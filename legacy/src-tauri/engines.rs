use crate::models::{AppState, EngineDiagnostics, LocalDevice};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

pub fn base_project_dir() -> PathBuf {
    let preferred = PathBuf::from("/Users/ilove/Downloads/clean-studio");
    if preferred.exists() {
        return preferred;
    }
    std::env::current_dir().unwrap_or(preferred)
}

pub fn studio_jar_path() -> PathBuf {
    let local = base_project_dir().join("resources/engines/dist-server/studio-server.jar");
    if local.exists() {
        return local;
    }
    PathBuf::from("/Applications/Maestro Studio.app/Contents/Resources/app.asar.unpacked/dist-server/studio-server.jar")
}

pub fn java_bin_path() -> PathBuf {
    let local = base_project_dir().join("resources/engines/bundled-jvm/mac-arm64/Contents/Home/bin/java");
    if local.exists() {
        return local;
    }
    PathBuf::from("/Applications/Maestro Studio.app/Contents/Resources/app.asar.unpacked/bundled-jvm/mac-arm64/Contents/Home/bin/java")
}

pub fn simulator_bin_path() -> PathBuf {
    let local = base_project_dir().join("resources/engines/bundled-simulator-server/mac-universal/simulator-server");
    if local.exists() {
        return local;
    }
    PathBuf::from("/Applications/Maestro Studio.app/Contents/Resources/app.asar.unpacked/bundled-simulator-server/mac-universal/simulator-server")
}

pub fn detect_android_sdk_internal() -> Option<String> {
    if let Ok(env_sdk) = std::env::var("ANDROID_HOME") {
        if PathBuf::from(&env_sdk).exists() {
            return Some(env_sdk);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/ilove".to_string());
    let default_sdk = format!("{}/Library/Android/sdk", home);
    if PathBuf::from(&default_sdk).exists() {
        return Some(default_sdk);
    }
    None
}

pub fn is_port_5050_open() -> bool {
    TcpStream::connect_timeout(
        &"127.0.0.1:5050".parse().unwrap(),
        Duration::from_millis(250),
    )
    .is_ok()
}

pub fn push_log(logs: &std::sync::Arc<std::sync::Mutex<Vec<String>>>, line: String) {
    if let Ok(mut guard) = logs.lock() {
        guard.push(line);
        if guard.len() > 250 {
            let excess = guard.len() - 250;
            guard.drain(0..excess);
        }
    }
}

#[tauri::command]
pub fn get_engine_diagnostics(state: State<'_, AppState>) -> EngineDiagnostics {
    let jar = studio_jar_path();
    let java = java_bin_path();
    let sim = simulator_bin_path();
    let sdk = detect_android_sdk_internal();

    let mut verify_lines = Vec::new();
    if sim.exists() {
        let mut cmd = Command::new(&sim);
        cmd.arg("verify");
        if let Some(ref s) = sdk {
            cmd.env("ANDROID_HOME", s);
        }
        if let Some(parent) = sim.parent() {
            let res_dir = parent.join("resources/android");
            if res_dir.exists() {
                cmd.env("SIMULATOR_RESOURCES_DIR", res_dir);
            }
        }
        if let Ok(out) = cmd.output() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for l in stdout.lines() {
                if !l.trim().is_empty() {
                    verify_lines.push(l.trim().to_string());
                }
            }
        }
    }

    let active_stream = state
        .active_stream_url
        .lock()
        .ok()
        .and_then(|g| g.clone());

    EngineDiagnostics {
        studio_server_running: is_port_5050_open(),
        studio_server_port: 5050,
        studio_jar_exists: jar.exists(),
        java_bin_exists: java.exists(),
        simulator_bin_exists: sim.exists(),
        simulator_verify_output: verify_lines,
        android_sdk_path: sdk,
        active_stream_url: active_stream,
        maestro_oss_path: base_project_dir()
            .join("packages/maestro-oss")
            .to_string_lossy()
            .to_string(),
    }
}

#[tauri::command]
pub fn start_studio_server(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    if is_port_5050_open() {
        return Ok("El servidor Maestro Core ya está activo en el puerto 5050.".to_string());
    }

    let jar = studio_jar_path();
    let java = java_bin_path();
    if !jar.exists() {
        return Err(format!("No se encontró studio-server.jar en {:?}", jar));
    }
    if !java.exists() {
        return Err(format!("No se encontró el runtime Java en {:?}", java));
    }

    let log_dir = base_project_dir().join("logs");
    let _ = fs::create_dir_all(&log_dir);
    let log_file = log_dir.join("studio-server.log");

    let mut cmd = Command::new(&java);
    cmd.arg(format!("-Dlog.path={}", log_file.to_string_lossy()))
        .arg("-jar")
        .arg(&jar)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(sdk) = detect_android_sdk_internal() {
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!(
            "{}:{}/platform-tools:{}/emulator:{}/cmdline-tools/latest/bin",
            current_path, sdk, sdk, sdk
        );
        cmd.env("ANDROID_HOME", &sdk);
        cmd.env("PATH", new_path);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Error al iniciar studio-server.jar desde Rust: {}", e))?;

    let logs_arc = state.server_logs.clone();
    if let Some(stdout) = child.stdout.take() {
        let logs_clone = logs_arc.clone();
        let app_clone = app.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                push_log(&logs_clone, format!("[CORE] {}", line));
                let _ = app_clone.emit("server-log", format!("[CORE] {}", line));
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let logs_clone = logs_arc.clone();
        let app_clone = app.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                push_log(&logs_clone, format!("[CORE-WARN] {}", line));
                let _ = app_clone.emit("server-log", format!("[CORE-WARN] {}", line));
            }
        });
    }

    if let Ok(mut guard) = state.studio_server.lock() {
        *guard = Some(child);
    }

    Ok("Servidor Maestro Core iniciado en http://localhost:5050".to_string())
}

#[tauri::command]
pub fn stop_studio_server(state: State<'_, AppState>) -> Result<String, String> {
    if let Ok(mut guard) = state.studio_server.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            return Ok("Servidor Maestro Core detenido correctamente.".to_string());
        }
    }
    Ok("El servidor no estaba ejecutándose bajo esta sesión.".to_string())
}

#[tauri::command]
pub fn start_simulator_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    device_id: String,
    target: String,
) -> Result<String, String> {
    if let Ok(mut guard) = state.simulator_server.lock() {
        if let Some(mut old) = guard.take() {
            let _ = old.kill();
        }
    }

    let sim = simulator_bin_path();
    if !sim.exists() {
        return Err(format!("No se encontró simulator-server en {:?}", sim));
    }

    let mut cmd = Command::new(&sim);
    cmd.arg(&target)
        .arg("--id")
        .arg(&device_id)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(parent) = sim.parent() {
        cmd.current_dir(parent);
        let res_dir = parent.join("resources/android");
        if res_dir.exists() {
            cmd.env("SIMULATOR_RESOURCES_DIR", res_dir);
        }
    }
    if let Some(sdk) = detect_android_sdk_internal() {
        let current_path = std::env::var("PATH").unwrap_or_default();
        cmd.env("ANDROID_HOME", &sdk);
        cmd.env("PATH", format!("{}:{}/platform-tools", current_path, sdk));
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Error al lanzar simulator-server en Rust: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(stdin, "setUpKeyboard");
    }

    let stream_arc = state.active_stream_url.clone();
    let logs_arc = state.server_logs.clone();
    if let Some(stdout) = child.stdout.take() {
        let app_clone = app.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                push_log(&logs_arc, format!("[STREAM] {}", line));
                if line.contains("stream_ready") || line.contains("stream.mjpeg") {
                    for token in line.split_whitespace() {
                        if token.starts_with("http://") && token.contains("stream.mjpeg") {
                            if let Ok(mut g) = stream_arc.lock() {
                                *g = Some(token.to_string());
                            }
                            let _ = app_clone.emit("simulator-stream-url", token.to_string());
                        }
                    }
                }
            }
        });
    }

    if let Ok(mut guard) = state.simulator_server.lock() {
        *guard = Some(child);
    }

    Ok(format!("Iniciando stream de pantalla para {} ({})...", device_id, target))
}

#[tauri::command]
pub fn stop_simulator_stream(state: State<'_, AppState>) -> Result<String, String> {
    if let Ok(mut guard) = state.simulator_server.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
        }
    }
    if let Ok(mut g) = state.active_stream_url.lock() {
        *g = None;
    }
    Ok("Stream de pantalla detenido.".to_string())
}

#[tauri::command]
pub fn list_local_devices() -> Vec<LocalDevice> {
    let mut devices = Vec::new();

    if let Ok(out) = Command::new("xcrun")
        .args(["simctl", "list", "devices", "--json"])
        .output()
    {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(dev_map) = val.get("devices").and_then(|d| d.as_object()) {
                for (runtime_key, list) in dev_map {
                    let runtime_clean = runtime_key
                        .split('.')
                        .next_back()
                        .unwrap_or(runtime_key)
                        .replace('-', " ");
                    if let Some(arr) = list.as_array() {
                        for item in arr {
                            let is_avail = item.get("isAvailable").and_then(|v| v.as_bool()).unwrap_or(false);
                            let state = item.get("state").and_then(|v| v.as_str()).unwrap_or("Shutdown");
                            if is_avail && (state == "Booted" || runtime_clean.contains("iOS")) {
                                devices.push(LocalDevice {
                                    id: item.get("udid").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    name: item.get("name").and_then(|v| v.as_str()).unwrap_or("iPhone").to_string(),
                                    platform: "iOS".to_string(),
                                    state: if state == "Booted" { "Conectado (Booted)".to_string() } else { "Apagado".to_string() },
                                    runtime: runtime_clean.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    let adb_cmd = detect_android_sdk_internal()
        .map(|s| format!("{}/platform-tools/adb", s))
        .unwrap_or_else(|| "adb".to_string());
    if let Ok(out) = Command::new(&adb_cmd).args(["devices", "-l"]).output() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines().skip(1) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let id = parts[0].to_string();
                let status = parts[1];
                let model = parts
                    .iter()
                    .find(|p| p.starts_with("model:"))
                    .map(|p| p.replace("model:", ""))
                    .unwrap_or_else(|| id.clone());
                devices.push(LocalDevice {
                    id,
                    name: model,
                    platform: "Android".to_string(),
                    state: if status == "device" { "Conectado (ADB)".to_string() } else { status.to_string() },
                    runtime: "Android ADB".to_string(),
                });
            }
        }
    }

    devices.sort_by(|a, b| {
        let a_booted = a.state.contains("Conectado");
        let b_booted = b.state.contains("Conectado");
        b_booted.cmp(&a_booted).then_with(|| a.name.cmp(&b.name))
    });
    devices.truncate(25);
    devices
}

#[tauri::command]
pub fn boot_ios_simulator(udid: String) -> Result<String, String> {
    let _ = Command::new("xcrun").args(["simctl", "boot", &udid]).output();
    let _ = Command::new("open").args(["-a", "Simulator"]).spawn();
    Ok(format!("Simulador {} iniciado.", udid))
}
