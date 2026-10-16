//! Ciclo de vida de studio-server.jar en el puerto 5050 (puerto de `lA`, `Us` y `C_e`).
use crate::{bridge, paths, AppState};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use tauri::{AppHandle, Manager};

pub const PORT: u16 = 5050;

#[derive(Default)]
pub struct ServerState {
    child: Mutex<Option<Child>>,
    /// Identifica el proceso vigente: un proceso viejo que termina no afecta al nuevo.
    generation: AtomicU64,
    stopping: AtomicBool,
}

fn server_error(app: &AppHandle, payload: Value) {
    bridge::send(app, "server-error", vec![payload]);
}

pub fn status(app: &AppHandle) -> Value {
    let state = app.state::<AppState>();
    let running = state.server.child.lock().unwrap().is_some();
    if running {
        json!({ "running": true, "port": PORT })
    } else {
        json!({ "running": false })
    }
}

pub fn start(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    if state.server.child.lock().unwrap().is_some() {
        return true;
    }

    let jar = paths::studio_jar(app);
    let java = paths::java_bin(app);
    let java_home = paths::java_home(app);
    let logs_dir = app
        .path()
        .app_data_dir()
        .map(|d| d.join("logs"))
        .unwrap_or_else(|_| paths::project_dir().join("logs"));
    let _ = std::fs::create_dir_all(&logs_dir);
    let log_file = logs_dir.join("studio-server.log");

    let diagnostics = json!({
        "javaPath": java,
        "javaPathExists": java.exists(),
        "javaHomePath": java_home,
        "javaHomePathExists": java_home.exists(),
        "serverPath": jar,
        "serverPathExists": jar.exists(),
        "isWindows": false,
        "processEnvPath": std::env::var("PATH").unwrap_or_default(),
    });

    if !jar.exists() {
        let msg = "studio-server.jar not found. Please build the studio server before launching the app.";
        server_error(app, json!({ "type": "STARTUP_FAILURE", "message": msg, "diagnostics": diagnostics }));
        bridge::error_box("No se encontró Studio Server", msg);
        return false;
    }
    if !java.exists() {
        let msg = "Java runtime not found or not executable. Please install Java 17 or later to run Maestro Studio.";
        server_error(
            app,
            json!({
                "type": "JAVA_MISSING",
                "message": msg,
                "diagnostics": diagnostics,
                "suggestion": "Please install Java 17 or later to run Maestro Studio"
            }),
        );
        bridge::error_box("Error del runtime de Java", msg);
        return false;
    }

    let mut cmd = Command::new(&java);
    cmd.arg(format!("-Dlog.path={}", log_file.display()))
        .arg("-jar")
        .arg(&jar)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut path_var = std::env::var("PATH").unwrap_or_default();
    if let Some(sdk) = state.settings.android_sdk_path() {
        for sub in ["platform-tools", "emulator", "cmdline-tools/latest/bin"] {
            path_var.push(':');
            path_var.push_str(&format!("{}/{}", sdk, sub));
        }
        cmd.env("ANDROID_HOME", &sdk);
    }
    // Las apps GUI de macOS heredan un PATH mínimo: xcrun/adb de Homebrew deben ser visibles
    for extra in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"] {
        if !path_var.split(':').any(|p| p == extra) {
            path_var.push(':');
            path_var.push_str(extra);
        }
    }
    cmd.env("PATH", path_var);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            server_error(
                app,
                json!({ "type": "STARTUP_FAILURE", "message": format!("Failed to start server: {}", e) }),
            );
            return false;
        }
    };

    let generation = state.server.generation.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some(stdout) = child.stdout.take() {
        let app = app.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let line = line.trim().to_string();
                if !line.is_empty() {
                    bridge::send(&app, "server-output", vec![json!(line)]);
                }
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let app = app.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let line = line.trim().to_string();
                if !line.is_empty() {
                    bridge::send(&app, "server-error", vec![json!(line)]);
                }
            }
        });
    }
    *state.server.child.lock().unwrap() = Some(child);
    spawn_exit_watcher(app.clone(), generation);

    bridge::send(app, "server-status", vec![json!({ "running": true, "port": PORT })]);
    true
}

/// Espera a que el proceso termine (sin bloquear el Mutex) y notifica como `Wr.on("close")`.
fn spawn_exit_watcher(app: AppHandle, generation: u64) {
    thread::spawn(move || loop {
        thread::sleep(std::time::Duration::from_millis(500));
        let state = app.state::<AppState>();
        if state.server.generation.load(Ordering::SeqCst) != generation {
            return;
        }
        let code = {
            let mut guard = state.server.child.lock().unwrap();
            let Some(child) = guard.as_mut() else { return };
            match child.try_wait() {
                Ok(Some(status)) => {
                    *guard = None;
                    status.code()
                }
                Ok(None) => continue,
                Err(_) => {
                    *guard = None;
                    None
                }
            }
        };
        if !state.server.stopping.load(Ordering::SeqCst) {
            if let Some(c) = code.filter(|c| *c != 0) {
                let mut payload = json!({
                    "type": "PORT_IN_USE",
                    "code": c,
                    "message": format!("Server process exited with code {}", c),
                });
                if c == 1 {
                    payload["suggestion"] =
                        json!(format!("Port {} may be in use. Try running: lsof -i :{}", PORT, PORT));
                }
                server_error(&app, payload);
            }
        }
        bridge::send(&app, "server-status", vec![json!({ "running": false, "exitCode": code })]);
        return;
    });
}

pub fn stop(app: &AppHandle) {
    let state = app.state::<AppState>();
    let child = state.server.child.lock().unwrap().take();
    if let Some(mut child) = child {
        state.server.stopping.store(true, Ordering::SeqCst);
        let _ = child.kill();
        let _ = child.wait();
        state.server.stopping.store(false, Ordering::SeqCst);
        // El watcher ya no verá el hijo: se notifica aquí el cierre
        bridge::send(app, "server-status", vec![json!({ "running": false, "exitCode": Value::Null })]);
    }
}

pub fn restart(app: &AppHandle) {
    bridge::send(app, "server-status", vec![json!({ "restarting": true })]);
    stop(app);
    start(app);
    bridge::send(app, "server-status", vec![json!({ "restarting": false })]);
}
