//! Utilidades compartidas: envío de mensajes main→renderer y diálogos nativos de macOS.
use serde_json::{json, Value};
use std::process::Command;
use tauri::{AppHandle, Emitter};

/// Equivalente a `webContents.send(channel, ...args)`: el shim lo despacha a los
/// listeners de `ipcRenderer.on(channel)`.
pub fn send(app: &AppHandle, channel: &str, args: Vec<Value>) {
    let _ = app.emit("cs-ipc", json!({ "channel": channel, "args": args }));
}

fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Ejecuta AppleScript y devuelve stdout; `None` si el usuario canceló.
fn osascript(script: &str) -> Option<String> {
    let out = Command::new("osascript").arg("-e").arg(script).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string();
    (!s.is_empty()).then_some(s)
}

/// El diálogo lo muestra `osascript`; `activate` lo trae al frente sin pedir permisos de Automatización.
fn front(script: &str) -> String {
    format!("activate\n{}", script)
}

pub fn choose_folder(prompt: &str) -> Option<String> {
    osascript(&front(&format!(
        "POSIX path of (choose folder with prompt \"{}\")",
        escape_applescript(prompt)
    )))
    .map(|p| p.trim_end_matches('/').to_string())
}

pub fn choose_file(prompt: &str, extensions: &[&str]) -> Option<String> {
    let types = if extensions.is_empty() {
        String::new()
    } else {
        format!(
            " of type {{{}}}",
            extensions.iter().map(|e| format!("\"{}\"", e)).collect::<Vec<_>>().join(", ")
        )
    };
    osascript(&front(&format!(
        "POSIX path of (choose file with prompt \"{}\"{})",
        escape_applescript(prompt),
        types
    )))
    .map(|p| p.trim_end_matches('/').to_string())
}

pub fn choose_file_name(prompt: &str, default_name: &str) -> Option<String> {
    osascript(&front(&format!(
        "POSIX path of (choose file name with prompt \"{}\" default name \"{}\")",
        escape_applescript(prompt),
        escape_applescript(default_name)
    )))
}

/// `dialog.showMessageBox`: devuelve `{ response, checkboxChecked }`.
pub fn message_box(opts: &Value) -> Value {
    let title = opts.get("title").and_then(Value::as_str).unwrap_or("Clean Studio");
    let message = opts.get("message").and_then(Value::as_str).unwrap_or("");
    let detail = opts.get("detail").and_then(Value::as_str).unwrap_or("");
    let mut buttons: Vec<String> = opts
        .get("buttons")
        .and_then(Value::as_array)
        .map(|b| b.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    if buttons.is_empty() {
        buttons.push("OK".into());
    }
    // AppleScript admite como máximo 3 botones
    buttons.truncate(3);
    let default_id = opts.get("defaultId").and_then(Value::as_u64).unwrap_or(0) as usize;
    let cancel_id = opts.get("cancelId").and_then(Value::as_u64).map(|v| v as usize);
    let text = if detail.is_empty() { message.to_string() } else { format!("{}\n\n{}", message, detail) };
    let icon = match opts.get("type").and_then(Value::as_str) {
        Some("error") => " with icon stop",
        Some("warning") => " with icon caution",
        _ => "",
    };
    let list = buttons.iter().map(|b| format!("\"{}\"", escape_applescript(b))).collect::<Vec<_>>().join(", ");
    let script = front(&format!(
        "button returned of (display dialog \"{}\" with title \"{}\" buttons {{{}}} default button {}{})",
        escape_applescript(&text),
        escape_applescript(title),
        list,
        default_id.min(buttons.len() - 1) + 1,
        icon
    ));
    let response = match osascript(&script) {
        Some(pressed) => buttons.iter().position(|b| *b == pressed).unwrap_or(0),
        None => cancel_id.unwrap_or(buttons.len() - 1),
    };
    json!({ "response": response, "checkboxChecked": false })
}

pub fn error_box(title: &str, message: &str) {
    let _ = osascript(&front(&format!(
        "display alert \"{}\" message \"{}\" as critical",
        escape_applescript(title),
        escape_applescript(message)
    )));
}

pub fn open_external(url: &str) -> Result<(), String> {
    open::that(url).map_err(|e| e.to_string())
}

pub fn reveal_in_finder(path: &str) {
    let _ = Command::new("open").arg("-R").arg(path).spawn();
}
