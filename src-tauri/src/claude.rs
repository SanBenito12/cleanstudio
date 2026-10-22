//! Conexión con Claude (ventana "Claude"): estado, alta en Claude Desktop / Claude Code y
//! una prueba real del servidor MCP (`clean-studio mcp`).
use crate::paths;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const SERVER_NAME: &str = "clean-studio";

fn exe() -> String {
    std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
}

fn desktop_dir() -> PathBuf {
    paths::home_dir().join("Library/Application Support/Claude")
}

fn desktop_config() -> PathBuf {
    desktop_dir().join("claude_desktop_config.json")
}

fn read_json(path: &PathBuf) -> Option<Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok())
}

/// Comando de Claude Code, a nivel de usuario para que funcione en cualquier carpeta.
pub fn code_command() -> String {
    format!("claude mcp add --scope user {} -- \"{}\" mcp", SERVER_NAME, exe())
}

/// `claude` (CLI de Claude Code): rutas habituales y, si no, el PATH del shell del usuario.
fn claude_cli() -> Option<String> {
    let home = paths::home_dir();
    for p in [
        home.join(".local/bin/claude"),
        home.join(".claude/local/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ] {
        if p.exists() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    let out = Command::new("zsh").args(["-lc", "command -v claude"]).output().ok()?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!path.is_empty()).then_some(path)
}

fn code_configured() -> bool {
    let Some(cfg) = read_json(&paths::home_dir().join(".claude.json")) else { return false };
    if cfg["mcpServers"].get(SERVER_NAME).is_some() {
        return true;
    }
    cfg["projects"]
        .as_object()
        .map(|p| p.values().any(|v| v["mcpServers"].get(SERVER_NAME).is_some()))
        .unwrap_or(false)
}

/// Arranca `clean-studio mcp` como lo haría Claude y pide la lista de herramientas.
fn self_test() -> Result<usize, String> {
    let mut child = Command::new(exe())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(
            stdin,
            "{}\n{}",
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"clean-studio-prueba","version":"1"}}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})
        );
    }
    let stdout = child.stdout.take().ok_or("sin salida")?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if v["id"] == 2 {
                    let _ = tx.send(v["result"]["tools"].as_array().map(|t| t.len()).unwrap_or(0));
                    break;
                }
            }
        }
    });
    let result = rx.recv_timeout(Duration::from_secs(10)).map_err(|_| "no respondió".to_string());
    let _ = child.kill();
    let _ = child.wait();
    result
}

/// Estado completo para la ventana.
pub fn status() -> Value {
    let desktop_installed = desktop_dir().exists() || PathBuf::from("/Applications/Claude.app").exists();
    let desktop_entry = read_json(&desktop_config()).and_then(|c| c["mcpServers"].get(SERVER_NAME).cloned());
    let desktop_ok = desktop_entry
        .as_ref()
        .and_then(|e| e["command"].as_str())
        .map(|c| c == exe())
        .unwrap_or(false);
    let cli = claude_cli();
    let test = self_test();
    json!({
        "exe": exe(),
        "desktop": {
            "instalado": desktop_installed,
            "conectado": desktop_ok,
            // Registrado, pero apuntando a otra copia de la app (p. ej. una versión de pruebas)
            "otraRuta": desktop_entry.is_some() && !desktop_ok,
        },
        "code": {
            "instalado": cli.is_some(),
            "conectado": code_configured(),
            "comando": code_command(),
        },
        "servidor": match test {
            Ok(n) => json!({ "ok": n > 0, "herramientas": n }),
            Err(e) => json!({ "ok": false, "error": e }),
        },
    })
}

pub fn add_desktop() -> Result<(), String> {
    if !desktop_dir().exists() {
        return Err("No encontré Claude Desktop en este Mac. Descárgalo en claude.ai/download.".into());
    }
    let file = desktop_config();
    let mut config = read_json(&file).unwrap_or_else(|| json!({}));
    if !config.is_object() {
        return Err("claude_desktop_config.json no tiene un formato válido; no lo modifiqué.".into());
    }
    if !config["mcpServers"].is_object() {
        config["mcpServers"] = json!({});
    }
    config["mcpServers"][SERVER_NAME] = json!({ "command": exe(), "args": ["mcp"] });
    let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&file, text).map_err(|e| format!("No se pudo guardar {}: {}", file.display(), e))
}

pub fn add_code() -> Result<(), String> {
    let cli = claude_cli().ok_or("No encontré Claude Code (el comando claude). Instálalo o copia el comando y ejecútalo tú.")?;
    // Se reemplaza la entrada anterior si apuntaba a otra ruta
    let _ = Command::new(&cli).args(["mcp", "remove", "--scope", "user", SERVER_NAME]).output();
    let out = Command::new(&cli)
        .args(["mcp", "add", "--scope", "user", SERVER_NAME, "--", &exe(), "mcp"])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { "Claude Code no aceptó el comando.".into() } else { err })
    }
}

pub fn copy(text: &str) -> bool {
    Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            if let Some(mut i) = c.stdin.take() {
                let _ = i.write_all(text.as_bytes());
            }
            c.wait()
        })
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn open_claude_desktop() {
    let _ = Command::new("open").args(["-a", "Claude"]).spawn();
}

/// "Arreglar con Claude" en un paso fallido: con Claude Code abre una terminal en el workspace y
/// arranca `claude` con el pedido ya escrito; si no está, copia el pedido y abre Claude Desktop.
/// Devuelve `"code"` o `"desktop"` para que la interfaz diga qué pasó.
pub fn fix_failed_step(workspace: Option<PathBuf>, archivo: &str, paso: &str, error: &str) -> Result<Value, String> {
    let nombre = PathBuf::from(archivo)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| archivo.to_string());
    let pedido = format!(
        "El flujo {nombre} ({archivo}) falló en el paso \"{paso}\" con este error: {error}\n\n\
         Usa las herramientas de Clean Studio: lee la última ejecución con leer_ejecucion, mira la pantalla \
         del dispositivo con ver_pantalla y explícame en español por qué falló. Luego corrige {nombre}, \
         valídalo con validar_flujo y vuelve a ejecutarlo con ejecutar_flujo para comprobar que pasa."
    );
    match claude_cli() {
        Some(cli) => {
            // Un .command se abre en Terminal sin pedir permisos de Automatización
            let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
            let dir = workspace.unwrap_or_else(|| PathBuf::from(archivo).parent().map(PathBuf::from).unwrap_or_default());
            let script = std::env::temp_dir().join("clean-studio-arreglar.command");
            let body = format!(
                "#!/bin/zsh -l\ncd {} && exec {} {}\n",
                quote(&dir.to_string_lossy()),
                quote(&cli),
                quote(&pedido)
            );
            std::fs::write(&script, body).map_err(|e| format!("No se pudo preparar la terminal: {}", e))?;
            let _ = Command::new("chmod").arg("+x").arg(&script).status();
            Command::new("open")
                .args(["-a", "Terminal"])
                .arg(&script)
                .spawn()
                .map_err(|e| format!("No se pudo abrir la Terminal: {}", e))?;
            Ok(json!("code"))
        }
        None => {
            if !copy(&pedido) {
                return Err("No se pudo copiar el pedido para Claude.".to_string());
            }
            open_claude_desktop();
            Ok(json!("desktop"))
        }
    }
}
