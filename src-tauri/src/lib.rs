//! Clean Studio: el renderer original de Maestro Studio sobre Tauri v2.
//! Este crate reemplaza el proceso main de Electron (ver `ipc.rs` para los canales).
mod bridge;
mod ipc;
mod paths;
mod server;
mod settings;
mod simulator;
mod windows;
mod workspace;

use tauri::{Manager, RunEvent};

#[derive(Default)]
pub struct AppState {
    pub settings: settings::Settings,
    pub server: server::ServerState,
    pub simulator: simulator::SimulatorState,
    pub workspace: workspace::WorkspaceState,
}

fn shutdown(app: &tauri::AppHandle) {
    simulator::stop(app, true);
    server::stop(app);
}

static SIGNALED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    SIGNALED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// SIGINT/SIGTERM cierran la app limpiamente (como el original) para no dejar
/// studio-server ni simulator-server huérfanos.
fn handle_signals(app: tauri::AppHandle) {
    unsafe {
        libc::signal(libc::SIGINT, on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t);
    }
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        if SIGNALED.load(std::sync::atomic::Ordering::SeqCst) {
            shutdown(&app);
            app.exit(0);
            return;
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![ipc::ipc_send, ipc::ipc_invoke])
        .menu(windows::build_menu)
        .on_menu_event(|app, event| windows::on_menu_event(app, event.id().as_ref()))
        .setup(|app| {
            let handle = app.handle().clone();
            app.state::<AppState>().settings.load(&handle);
            windows::create_main(&handle)?;
            handle_signals(handle.clone());
            // Igual que el original: verificación del simulator-server y arranque del servidor
            std::thread::spawn(move || {
                simulator::preflight(&handle);
                server::start(&handle);
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Error al iniciar Clean Studio")
        .run(|app, event| match event {
            RunEvent::WindowEvent { label, event: tauri::WindowEvent::Destroyed, .. } if label == windows::MAIN => {
                shutdown(app);
                app.exit(0);
            }
            RunEvent::Exit => shutdown(app),
            _ => {}
        });
}
