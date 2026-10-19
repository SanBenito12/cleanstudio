//! Ventanas: principal, "Acerca de" y menú nativo (puerto de `uW`, `cA`, `Mme` y `Lme`).
use crate::bridge;
use serde_json::json;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, LogicalPosition, Manager, TitleBarStyle, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};

pub const MAIN: &str = "main";
pub const ABOUT: &str = "about";
const APP_NAME: &str = "Clean Studio";

/// `clean-studio <carpeta>` abre esa carpeta como workspace: el renderer la lee de
/// `localStorage["maestro-workspace-path"]` al arrancar.
fn workspace_from_args() -> Option<String> {
    let arg = std::env::args().skip(1).find(|a| !a.starts_with('-'))?;
    let path = std::fs::canonicalize(arg).ok()?;
    path.is_dir().then(|| path.to_string_lossy().to_string())
}

pub fn create_main(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let mut builder = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()));
    if let Some(dir) = workspace_from_args() {
        app.state::<crate::AppState>().settings.add_recent_workspace(&dir);
        let value = serde_json::to_string(&serde_json::to_string(&dir).unwrap_or_default()).unwrap_or_default();
        builder = builder.initialization_script(format!(
            "try{{localStorage.setItem(\"maestro-workspace-path\",{})}}catch(e){{}}",
            value
        ));
    }
    let window = builder
        .title(APP_NAME)
        .inner_size(1200.0, 800.0)
        .min_inner_size(900.0, 600.0)
        .center()
        .title_bar_style(TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(LogicalPosition::new(12.0, 14.0))
        .on_page_load(|w, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                let now = chrono::Local::now().format("%d/%m/%Y, %H:%M:%S").to_string();
                bridge::send(w.app_handle(), "main-process-message", vec![json!(now)]);
            }
        })
        .build()?;
    track_fullscreen(&window);
    Ok(window)
}

/// Emite `window-fullscreen-change` cuando cambia el estado de pantalla completa.
fn track_fullscreen(window: &WebviewWindow) {
    let w = window.clone();
    let last = std::sync::Mutex::new(false);
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Resized(_) = event {
            let fs = w.is_fullscreen().unwrap_or(false);
            let mut prev = last.lock().unwrap();
            if *prev != fs {
                *prev = fs;
                bridge::send(w.app_handle(), "window-fullscreen-change", vec![json!(fs)]);
            }
        }
    });
}

pub fn open_about(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(ABOUT) {
        let _ = w.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, ABOUT, WebviewUrl::App("index.html#/about".into()))
        .title(format!("Acerca de {}", APP_NAME))
        .inner_size(400.0, 500.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .title_bar_style(TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(LogicalPosition::new(12.0, 14.0))
        .build();
    if let Ok(w) = built {
        track_fullscreen(&w);
    }
}

pub fn close_about(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(ABOUT) {
        let _ = w.close();
    }
}

pub fn focus_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

const MENU_ABOUT: &str = "cs-about";
const MENU_EXPORT_LOGS: &str = "cs-export-logs";
const MENU_LEARN_MORE: &str = "cs-learn-more";
const MENU_LAST_REPORT: &str = "cs-last-report";
const MENU_CLAUDE: &str = "cs-claude";
const MENU_RUNS_FOLDER: &str = "cs-runs-folder";
const MENU_TOGGLE_VIDEO: &str = "cs-toggle-video";
const MENU_TOGGLE_OPEN_REPORT: &str = "cs-toggle-open-report";

/// Menú de la app, misma estructura que el original (traducido).
pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let about = MenuItem::with_id(app, MENU_ABOUT, format!("Acerca de {}", APP_NAME), true, None::<&str>)?;
    let app_menu = Submenu::with_items(
        app,
        APP_NAME,
        true,
        &[
            &about,
            &MenuItem::with_id(app, MENU_CLAUDE, "Conectar con Claude…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, Some("Servicios"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some(&format!("Ocultar {}", APP_NAME)))?,
            &PredefinedMenuItem::hide_others(app, Some("Ocultar otros"))?,
            &PredefinedMenuItem::show_all(app, Some("Mostrar todo"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, Some(&format!("Salir de {}", APP_NAME)))?,
        ],
    )?;
    let file = Submenu::with_items(app, "Archivo", true, &[&PredefinedMenuItem::close_window(app, Some("Cerrar ventana"))?])?;
    let edit = Submenu::with_items(
        app,
        "Editar",
        true,
        &[
            &PredefinedMenuItem::undo(app, Some("Deshacer"))?,
            &PredefinedMenuItem::redo(app, Some("Rehacer"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some("Cortar"))?,
            &PredefinedMenuItem::copy(app, Some("Copiar"))?,
            &PredefinedMenuItem::paste(app, Some("Pegar"))?,
            &PredefinedMenuItem::select_all(app, Some("Seleccionar todo"))?,
        ],
    )?;
    let view = Submenu::with_items(app, "Ver", true, &[&PredefinedMenuItem::fullscreen(app, Some("Pantalla completa"))?])?;
    let window = Submenu::with_items(
        app,
        "Ventana",
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some("Minimizar"))?,
            &PredefinedMenuItem::maximize(app, Some("Zoom"))?,
        ],
    )?;
    // Los ajustes se leen aquí porque el menú se construye antes de `setup`
    let settings = &app.state::<crate::AppState>().settings;
    settings.load(app);
    let runs = Submenu::with_items(
        app,
        "Ejecuciones",
        true,
        &[
            &MenuItem::with_id(app, MENU_LAST_REPORT, "Abrir el último reporte", true, Some("CmdOrCtrl+Shift+R"))?,
            &MenuItem::with_id(app, MENU_RUNS_FOLDER, "Abrir la carpeta de ejecuciones", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &CheckMenuItem::with_id(
                app,
                MENU_TOGGLE_VIDEO,
                "Guardar el video de cada ejecución",
                true,
                settings.bool_or("grabarVideo", true),
                None::<&str>,
            )?,
            &CheckMenuItem::with_id(
                app,
                MENU_TOGGLE_OPEN_REPORT,
                "Abrir el reporte al terminar",
                true,
                settings.bool_or("abrirReporteAlTerminar", false),
                None::<&str>,
            )?,
        ],
    )?;
    let help = Submenu::with_items(
        app,
        "Ayuda",
        true,
        &[
            &MenuItem::with_id(app, MENU_EXPORT_LOGS, "Exportar registros", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, MENU_LEARN_MORE, "Más información", true, None::<&str>)?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &file, &edit, &view, &runs, &window, &help])
}

pub fn on_menu_event(app: &AppHandle, id: &str) {
    match id {
        MENU_ABOUT => open_about(app),
        // Los diálogos de osascript bloquean: fuera del hilo principal
        MENU_CLAUDE => {
            std::thread::spawn(connect_claude);
        }
        MENU_LEARN_MORE => {
            let _ = bridge::open_external("https://github.com/SanBenito12/cleanstudio#readme");
        }
        MENU_LAST_REPORT => {
            let last = app.state::<crate::AppState>().settings.get("ultimoReporte");
            match last.and_then(|v| v.as_str().map(String::from)).filter(|p| std::path::Path::new(p).exists()) {
                Some(path) => {
                    let _ = open::that(path);
                }
                None => bridge::error_box(
                    "Sin reportes todavía",
                    "Ejecuta una prueba: al terminar, Clean Studio guarda el video y el reporte en la carpeta de ejecuciones del workspace.",
                ),
            }
        }
        MENU_RUNS_FOLDER => match crate::runs::runs_dir(app) {
            Some(dir) => {
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::process::Command::new("open").arg(&dir).spawn();
            }
            None => bridge::error_box("Sin workspace", "Abre un espacio de trabajo para ver sus ejecuciones."),
        },
        MENU_TOGGLE_VIDEO | MENU_TOGGLE_OPEN_REPORT => {
            let (key, default) =
                if id == MENU_TOGGLE_VIDEO { ("grabarVideo", true) } else { ("abrirReporteAlTerminar", false) };
            let settings = &app.state::<crate::AppState>().settings;
            settings.set(key, serde_json::json!(!settings.bool_or(key, default)));
        }
        MENU_EXPORT_LOGS => {
            // Los registros viven en <app_data_dir>/logs: se muestran en Finder
            if let Ok(dir) = app.path().app_data_dir() {
                let logs = dir.join("logs");
                if logs.exists() {
                    let _ = std::process::Command::new("open").arg(&logs).spawn();
                } else {
                    bridge::error_box(
                        "Exportar registros",
                        "Todavía no hay registros. Se crearán cuando arranque el servidor.",
                    );
                }
            }
        }
        _ => {}
    }
}

/// "Conectar con Claude…": registra `clean-studio mcp` en Claude Desktop o copia el comando
/// para Claude Code. Así Claude (con la suscripción del usuario) controla Clean Studio.
fn connect_claude() {
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let code_cmd = format!("claude mcp add clean-studio -- \"{}\" mcp", exe);
    let choice = bridge::message_box(&serde_json::json!({
        "title": "Conectar con Claude",
        "message": "Conecta Clean Studio con Claude",
        "detail": "Claude podrá ver tus dispositivos y la pantalla, escribir flujos, ejecutarlos y leer los reportes, usando tu suscripción de Claude. Todo sigue ejecutándose en tu Mac.",
        "buttons": ["Agregar a Claude Desktop", "Copiar comando para Claude Code", "Cancelar"],
        "defaultId": 0,
        "cancelId": 2
    }));
    match choice["response"].as_u64() {
        Some(0) => match add_to_claude_desktop(&exe) {
            Ok(path) => {
                bridge::message_box(&serde_json::json!({
                    "title": "Conectar con Claude",
                    "message": "Listo: Clean Studio quedó agregado a Claude Desktop.",
                    "detail": format!("Cierra y vuelve a abrir Claude Desktop para ver las herramientas de Clean Studio.\n\nConfiguración: {}", path),
                    "buttons": ["OK"]
                }));
            }
            Err(e) => bridge::error_box("Conectar con Claude", &e),
        },
        Some(1) => {
            let copied = std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut c| {
                    use std::io::Write;
                    c.stdin.take().map(|mut i| i.write_all(code_cmd.as_bytes()));
                    c.wait()
                })
                .is_ok();
            bridge::message_box(&serde_json::json!({
                "title": "Conectar con Claude",
                "message": if copied { "Comando copiado." } else { "Copia este comando:" },
                "detail": format!("Pégalo en la terminal donde usas Claude Code:\n\n{}", code_cmd),
                "buttons": ["OK"]
            }));
        }
        _ => {}
    }
}

fn add_to_claude_desktop(exe: &str) -> Result<String, String> {
    let dir = crate::paths::home_dir().join("Library/Application Support/Claude");
    if !dir.exists() {
        return Err("No encontré Claude Desktop en este Mac. Instálalo desde claude.ai/download o usa la opción de Claude Code.".into());
    }
    let file = dir.join("claude_desktop_config.json");
    let mut config: serde_json::Value = std::fs::read_to_string(&file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !config.is_object() {
        return Err("claude_desktop_config.json no tiene un formato válido; no lo modifiqué.".into());
    }
    if !config["mcpServers"].is_object() {
        config["mcpServers"] = serde_json::json!({});
    }
    config["mcpServers"]["clean-studio"] = serde_json::json!({ "command": exe, "args": ["mcp"] });
    let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&file, text).map_err(|e| format!("No se pudo guardar {}: {}", file.display(), e))?;
    Ok(file.to_string_lossy().to_string())
}
