//! Espejo en vivo vía simulator-server (puerto de `B_e`, `FR`, `aW` y `F_e`).
use crate::{bridge, paths, AppState};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use tauri::{AppHandle, Manager};

#[derive(Default)]
pub struct SimulatorState {
    child: Mutex<Option<Child>>,
    /// Dispositivo del espejo activo: (plataforma "ios"/"android", id). Lo usa la grabación de video.
    device: Mutex<Option<(String, String)>>,
    stdin: Mutex<Option<ChildStdin>>,
    generation: AtomicU64,
}

fn env_for(app: &AppHandle, cmd: &mut Command) {
    if let Some(sdk) = app.state::<AppState>().settings.android_sdk_path() {
        cmd.env("ANDROID_HOME", sdk);
    }
}

/// Verificación inicial (`simulator-server verify`), solo informativa como en el original.
pub fn preflight(app: &AppHandle) {
    let Some(bin) = paths::simulator_server(app) else {
        eprintln!("[SIM-SERVER] bundled simulator-server binary not found");
        return;
    };
    let mut cmd = Command::new(&bin);
    cmd.arg("verify").stdin(Stdio::null());
    env_for(app, &mut cmd);
    if let Ok(out) = cmd.output() {
        for line in String::from_utf8_lossy(&out.stdout).lines().filter(|l| !l.is_empty()) {
            eprintln!("[SIM-SERVER] verify: {}", line);
        }
    }
}

/// Últimas líneas de stderr para explicar por qué se cayó el stream.
struct Tail {
    last: String,
    error: String,
    lines: VecDeque<String>,
}

fn looks_like_error(line: &str) -> bool {
    let l = line.to_lowercase();
    let tagged_info = ["info", "debug", "trace", "warn"]
        .iter()
        .any(|t| l.starts_with('[') && l.split(']').next().map(|h| h.contains(t)).unwrap_or(false));
    !tagged_info
        && (["error", "panic", "panicked", "fatal"].iter().any(|w| l.contains(w))
            || l.contains("note: run with `rust_backtrace"))
}

fn truncate(s: &str) -> String {
    if s.chars().count() > 500 {
        format!("{}…", s.chars().take(500).collect::<String>())
    } else {
        s.to_string()
    }
}

pub fn start(app: &AppHandle, device_id: &str, target: &str) {
    stop(app, true);
    let Some(bin) = paths::simulator_server(app) else {
        bridge::send(app, "simulator-stream-error", vec![json!("No se encontró simulator-server dentro de Clean Studio")]);
        return;
    };

    let mut cmd = Command::new(&bin);
    cmd.args([target, "--id", device_id])
        .current_dir(bin.parent().unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    env_for(app, &mut cmd);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            bridge::send(app, "simulator-stream-error", vec![json!(format!("Simulator stream failed to start: {}", e))]);
            return;
        }
    };

    let state = app.state::<AppState>();
    let generation = state.simulator.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let started = Instant::now();
    let ready = Arc::new(Mutex::new(false));
    let tail = Arc::new(Mutex::new(Tail { last: String::new(), error: String::new(), lines: VecDeque::new() }));

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    *state.simulator.stdin.lock().unwrap() = child.stdin.take();
    *state.simulator.child.lock().unwrap() = Some(child);
    *state.simulator.device.lock().unwrap() = Some((target.to_string(), device_id.to_string()));

    if let Some(stdout) = stdout {
        let app = app.clone();
        let ready = ready.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let line = line.trim();
                if line.is_empty() || !is_current(&app, generation) {
                    continue;
                }
                if line.contains("stream_ready") {
                    if let Some(url) = line
                        .split_whitespace()
                        .find(|w| w.starts_with("http://") && w.ends_with("stream.mjpeg"))
                    {
                        *ready.lock().unwrap() = true;
                        eprintln!("[SIM-SERVER] Stream listo en {} ({} ms)", url, started.elapsed().as_millis());
                        bridge::send(&app, "simulator-stream-url", vec![json!(url)]);
                        device_command(&app, "setUpKeyboard");
                    }
                }
            }
        });
    }
    if let Some(stderr) = stderr {
        let tail = tail.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if line.is_empty() {
                    continue;
                }
                let mut t = tail.lock().unwrap();
                if looks_like_error(&line) {
                    t.error = line.clone();
                }
                t.last = line.clone();
                t.lines.push_back(line);
                if t.lines.len() > 20 {
                    t.lines.pop_front();
                }
            }
        });
    }

    // Cierre del proceso → url null + error, salvo que ya haya sido reemplazado
    let app = app.clone();
    thread::spawn(move || loop {
        thread::sleep(std::time::Duration::from_millis(400));
        if !is_current(&app, generation) {
            return;
        }
        let state = app.state::<AppState>();
        let exited = {
            let mut guard = state.simulator.child.lock().unwrap();
            match guard.as_mut().map(|c| c.try_wait()) {
                Some(Ok(Some(_))) | Some(Err(_)) => {
                    *guard = None;
                    true
                }
                Some(Ok(None)) => false,
                None => return,
            }
        };
        if !exited {
            continue;
        }
        *state.simulator.stdin.lock().unwrap() = None;
        bridge::send(&app, "simulator-stream-url", vec![Value::Null]);
        let t = tail.lock().unwrap();
        let fallback = if *ready.lock().unwrap() {
            "Device streaming stopped unexpectedly"
        } else {
            "Simulator stream failed to start"
        };
        let msg = [&t.error, &t.last].into_iter().find(|s| !s.is_empty()).map(|s| truncate(s));
        bridge::send(&app, "simulator-stream-error", vec![json!(msg.unwrap_or_else(|| fallback.to_string()))]);
        return;
    });
}

/// Dispositivo conectado al espejo, si hay uno.
pub fn current_device(app: &AppHandle) -> Option<(String, String)> {
    app.state::<AppState>().simulator.device.lock().unwrap().clone()
}

fn is_current(app: &AppHandle, generation: u64) -> bool {
    app.state::<AppState>().simulator.generation.load(Ordering::SeqCst) == generation
}

pub fn stop(app: &AppHandle, silent: bool) {
    let state = app.state::<AppState>();
    // Invalida los hilos del proceso anterior antes de matarlo
    state.simulator.generation.fetch_add(1, Ordering::SeqCst);
    *state.simulator.stdin.lock().unwrap() = None;
    let child = state.simulator.child.lock().unwrap().take();
    if !silent {
        *state.simulator.device.lock().unwrap() = None;
    }
    if let Some(mut child) = child {
        let _ = child.kill();
        let _ = child.wait();
        if !silent {
            bridge::send(app, "simulator-stream-url", vec![Value::Null]);
        }
    }
}

pub fn device_command(app: &AppHandle, command: &str) {
    let state = app.state::<AppState>();
    let mut guard = state.simulator.stdin.lock().unwrap();
    if let Some(stdin) = guard.as_mut() {
        let _ = writeln!(stdin, "{}", command);
        let _ = stdin.flush();
    }
}

/// Bundle del driver XCUITest de Maestro que se instala en cada simulador de iOS.
const DRIVER_IOS: &str = "dev.mobile.maestro-driver-iosUITests.xctrunner";

/// ¿Es el id de un simulador de iOS? (UDID: 8-4-4-4-12 en hexadecimal)
fn es_udid(id: &str) -> bool {
    let partes: Vec<&str> = id.split('-').collect();
    partes.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12]) && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Antes de conectar un simulador de iOS, cierra el driver de Maestro en los demás simuladores
/// encendidos. Todos usan el mismo puerto del Mac (22087): si otro sigue vivo, el motor habla con
/// ese y "ve" la pantalla de otro iPhone aunque el conectado sea este.
pub fn liberar_driver_ios(conectar: &str) {
    if !es_udid(conectar) {
        return;
    }
    let Ok(out) = Command::new("xcrun").args(["simctl", "list", "devices", "booted", "-j"]).output() else { return };
    let Ok(lista) = serde_json::from_slice::<Value>(&out.stdout) else { return };
    for d in lista["devices"].as_object().into_iter().flat_map(|m| m.values()).flat_map(|l| l.as_array().cloned().unwrap_or_default()) {
        let Some(udid) = d["udid"].as_str() else { continue };
        if udid != conectar {
            let cerrado = Command::new("xcrun").args(["simctl", "terminate", udid, DRIVER_IOS]).output();
            if cerrado.map(|o| o.status.success()).unwrap_or(false) {
                eprintln!("[SIMULADOR] Driver de Maestro cerrado en {} para conectar {}", udid, conectar);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reconoce_udids_de_ios() {
        assert!(super::es_udid("C7D925A8-EECC-4F10-A60E-10FF16858502"));
        assert!(!super::es_udid("emulator-5554"));
        assert!(!super::es_udid("chromium"));
        assert!(!super::es_udid("C7D925A8-EECC-4F10-A60E-10FF1685850Z"));
    }
}
