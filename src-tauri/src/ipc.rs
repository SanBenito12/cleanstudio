//! Emulación de `ipcMain`: cada canal del renderer original se despacha aquí.
//! `ipc_send` = `ipcRenderer.send` (sin respuesta), `ipc_invoke` = `ipcRenderer.invoke`.
use crate::{bridge, paths, server, simulator, windows, workspace, AppState};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, WebviewWindow};

/// Versión que el renderer original espera (novedades, compatibilidad con studio-server).
pub const STUDIO_VERSION: &str = "0.9.8";
const CLOUD_DISABLED: &str = "La nube está deshabilitada en Clean Studio: todo se ejecuta en tu Mac, sin cuentas.";
const UPDATES_DISABLED: &str = "Las actualizaciones automáticas están deshabilitadas en Clean Studio.";

fn arg(args: &[Value], i: usize) -> Value {
    args.get(i).cloned().unwrap_or(Value::Null)
}

fn arg_str(args: &[Value], i: usize) -> String {
    match args.get(i) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// Algunos manejadores bloquean (diálogos de osascript, procesos): fuera del hilo de IPC.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tauri::async_runtime::spawn_blocking(f).await.expect("tarea IPC abortada")
}

#[tauri::command]
pub async fn ipc_send(app: AppHandle, window: WebviewWindow, channel: String, args: Vec<Value>) {
    blocking(move || handle_send(&app, &window, &channel, &args)).await
}

fn handle_send(app: &AppHandle, window: &WebviewWindow, channel: &str, args: &[Value]) {
    let state = app.state::<AppState>();
    match channel {
        // Servidor
        "start-server" => {
            server::start(app);
        }
        "stop-server" => server::stop(app),
        "restart-server" => server::restart(app),
        "get-server-status" => bridge::send(app, "server-status", vec![server::status(app)]),
        "build-server" => {}

        // Ajustes
        "set-android-sdk-path" => state.settings.set_android_sdk_path(&arg_str(args, 0)),
        "set-allow-prerelease" => state.settings.set("allowPrerelease", arg(args, 0)),
        "set-web-browser-enabled" => state.settings.set("webBrowserEnabled", arg(args, 0)),
        "set-editor-settings" => state.settings.set("editorSettings", arg(args, 0)),
        "set-autosave-enabled" => {}

        // Archivos
        "open-file" => workspace::open_file(app, &arg(args, 0)),
        "save-file" => workspace::save_file(app, &arg(args, 0)),
        "setup-file-watcher" => {
            let expanded = args
                .get(1)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            workspace::setup_watcher(app, &arg_str(args, 0), expanded);
            crate::runs::send_flow_states(app);
        }
        "get-directory-contents" => workspace::directory_contents(app, &arg_str(args, 0)),
        "load-file-content" => workspace::load_file_content(app, &arg_str(args, 0)),

        // Ventanas
        "theme-changed" => bridge::send(app, "theme-update", vec![arg(args, 0)]),
        "window-minimize" => {
            let _ = window.minimize();
        }
        "window-maximize" => {
            if window.is_maximized().unwrap_or(false) {
                let _ = window.unmaximize();
            } else {
                let _ = window.maximize();
            }
        }
        "window-close" => {
            let _ = window.close();
        }
        "close-about-window" => windows::close_about(app),
        "app-control" => match arg(args, 0).get("action").and_then(Value::as_str) {
            Some("quit") => app.exit(0),
            Some("focus-main-window") => windows::focus_main(app),
            _ => {}
        },

        // Simulador
        "start-simulator-stream" => {
            let opts = arg(args, 0);
            let device = opts.get("deviceId").and_then(Value::as_str).unwrap_or_default().to_string();
            let target = opts.get("target").and_then(Value::as_str).unwrap_or_default().to_string();
            simulator::start(app, &device, &target);
        }
        "stop-simulator-stream" => simulator::stop(app, false),
        "simulator-device-command" => simulator::device_command(app, &arg_str(args, 0)),

        // Deshabilitados: actualizaciones y Cloud
        "check-update" => bridge::send(
            app,
            "update-check-result",
            vec![json!({ "update": false, "version": STUDIO_VERSION, "newVersion": STUDIO_VERSION })],
        ),
        "start-download" => bridge::send(app, "update-error", vec![json!({ "message": UPDATES_DISABLED })]),
        "quit-and-install" | "sync-user-context" => {}

        // Propio de Clean Studio: evidencia de ejecuciones (video + reporte)
        "cs-run-update" => crate::runs::update(app, &arg_str(args, 0), &arg_str(args, 1)),
        // Botón "Claude" de la cabecera: mismo diálogo que el menú "Conectar con Claude…"
        "cs-connect-claude" => {
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || windows::open_claude(&handle));
        }

        // Propio de Clean Studio: consola del renderer
        "renderer-log" => eprintln!("[RENDERER:{}] {}", arg_str(args, 0), arg_str(args, 1)),

        other => eprintln!("[IPC] canal send no soportado: {} {:?}", other, args),
    }
}

#[tauri::command]
pub async fn ipc_invoke(app: AppHandle, channel: String, args: Vec<Value>) -> Result<Value, String> {
    blocking(move || handle_invoke(&app, &channel, &args)).await
}

fn handle_invoke(app: &AppHandle, channel: &str, args: &[Value]) -> Result<Value, String> {
    let state = app.state::<AppState>();
    Ok(match channel {
        // Ajustes
        "get-android-sdk-path" => json!(state.settings.android_sdk_path()),
        "validate-android-sdk" => json!(paths::validate_android_sdk(&arg_str(args, 0))),
        "get-allow-prerelease" => json!(state.settings.bool_or("allowPrerelease", false)),
        "get-web-browser-enabled" => json!(state.settings.bool_or("webBrowserEnabled", false)),
        "get-editor-settings" => state.settings.editor_settings(),

        // Workspace y archivos
        "open-workspace-invoke" => workspace::open_workspace(app),
        "create-workspace" => workspace::create_workspace(app),
        "get-recent-workspaces" => workspace::recent_workspaces(app),
        "create-file" => workspace::create_file(app, &arg(args, 0)),
        "create-directory" => workspace::create_directory(app, &arg(args, 0)),
        "delete-file-or-directory" => workspace::delete(app, &arg(args, 0)),
        "duplicate-file" => workspace::duplicate(app, &arg(args, 0)),
        "move-files" => workspace::move_files(app, &arg(args, 0)),
        "rename-file-or-directory" => workspace::rename(app, &arg(args, 0)),
        "pick-app-file" => workspace::pick_app_file(),
        "read-file" => workspace::read_file(&arg_str(args, 0))?,
        "show-in-file-manager" => workspace::show_in_file_manager(&arg_str(args, 0)),

        // Ventanas y sistema
        "get-window-fullscreen-state" => json!(app
            .get_webview_window(windows::MAIN)
            .and_then(|w| w.is_fullscreen().ok())
            .unwrap_or(false)),
        "get-app-version" => json!(STUDIO_VERSION),
        "open-external" => {
            bridge::open_external(&arg_str(args, 0))?;
            Value::Null
        }
        "open-about" => {
            let handle = app.clone();
            app.run_on_main_thread(move || windows::open_about(&handle)).map_err(|e| e.to_string())?;
            Value::Null
        }
        "show-message-box" => bridge::message_box(&arg(args, 0)),

        // Ventana de Claude
        "cs-claude-status" => crate::claude::status(),
        "cs-claude-add-desktop" => {
            crate::claude::add_desktop()?;
            Value::Null
        }
        "cs-claude-add-code" => {
            crate::claude::add_code()?;
            Value::Null
        }
        "cs-claude-copy" => json!(crate::claude::copy(&arg_str(args, 0))),
        // Antes de conectar un dispositivo (parche de xye en build-studio-ui.mjs)
        "cs-antes-de-conectar" => {
            crate::simulator::liberar_driver_ios(&arg_str(args, 0));
            Value::Null
        }
        "cs-revisar-con-claude" => crate::revision::iniciar(app, &arg_str(args, 0))?,
        "cs-arreglar-con-claude" => {
            let opts = arg(args, 0);
            let s = |k: &str| opts.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            crate::claude::fix_failed_step(workspace::current_root(app), &s("archivo"), &s("paso"), &s("error"))?
        }
        "cs-claude-open-desktop" => {
            crate::claude::open_claude_desktop();
            Value::Null
        }

        // Árbol de archivos: último resultado de cada flujo y su reporte
        "cs-estados-flujos" => crate::runs::flow_states(app),
        "cs-abrir-reporte" => {
            crate::runs::open_report(app, &arg_str(args, 0))?;
            Value::Null
        }

        // Maestro Cloud deshabilitado: sin sesión
        "oauth-get-cached-token" | "oauth-validate-token" | "oauth-logout" => Value::Null,
        "oauth-sign-in" | "oauth-switch-org" => return Err(CLOUD_DISABLED.to_string()),

        other => return Err(format!("Canal IPC no soportado en Clean Studio: {}", other)),
    })
}
