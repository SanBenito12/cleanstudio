//! Ventanas: principal, "Acerca de" y menú nativo (puerto de `uW`, `cA`, `Mme` y `Lme`).
use crate::bridge;
use serde_json::json;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
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

/// Menú de la app, misma estructura que el original (traducido).
pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let about = MenuItem::with_id(app, MENU_ABOUT, format!("Acerca de {}", APP_NAME), true, None::<&str>)?;
    let app_menu = Submenu::with_items(
        app,
        APP_NAME,
        true,
        &[
            &about,
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
    Menu::with_items(app, &[&app_menu, &file, &edit, &view, &window, &help])
}

pub fn on_menu_event(app: &AppHandle, id: &str) {
    match id {
        MENU_ABOUT => open_about(app),
        MENU_LEARN_MORE => {
            let _ = bridge::open_external("https://docs.maestro.dev/maestro-studio/run-tests-with-maestro-studio");
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
