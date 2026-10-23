//! "Revisar con Claude": Claude mira las pantallas de una ejecución y anota lo que una regla
//! automática no ve (textos mal escritos, datos que no coinciden entre pantallas, elementos
//! cortados o encimados…).
//!
//! 1. Se saca una captura PNG del video al terminar cada paso (con el extractor de cuadros.swift)
//!    en `<ejecución>/revision/`.
//! 2. Se arranca Claude Code sin ventana (`claude -p`) en la carpeta de la ejecución, con este mismo
//!    ejecutable como único servidor MCP y permiso solo para leer archivos y `guardar_revision`.
//! 3. Claude guarda sus hallazgos con `guardar_revision` (mcp.rs → `guardar`), que los añade a
//!    `ejecucion.json` con `tipo: "claude"` y vuelve a generar el reporte.
//!
//! Sin Claude Code, el pedido se copia y se abre Claude Desktop.
use crate::{bridge, reports};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::AppHandle;

const MAX_CAPTURAS: usize = 30;

fn leer(dir: &Path) -> Result<Value, String> {
    std::fs::read_to_string(dir.join("ejecucion.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or_else(|| "No se encontró la ejecución.".to_string())
}

/// Pasos de primer nivel con el segundo del video en que terminan, para sacar sus capturas.
/// Devuelve (flujo, nombre del paso, archivo PNG, segundo); como mucho `MAX_CAPTURAS`, repartidas.
fn momentos(record: &Value) -> Vec<(usize, String, String, String, f64)> {
    let mut out = Vec::new();
    for (fi, f) in record["flujos"].as_array().into_iter().flatten().enumerate() {
        let (Some(video), Some(rec)) = (f["video"].as_str(), f["grabacionInicioMs"].as_i64()) else { continue };
        for (pi, p) in f["pasos"].as_array().into_iter().flatten().filter(|p| p["nivel"].as_u64() == Some(0)).enumerate() {
            let Some(ini) = p["inicioMs"].as_i64() else { continue };
            let fin = ini + p["duracionMs"].as_i64().unwrap_or(0);
            out.push((
                fi,
                p["paso"].as_str().unwrap_or("Paso").to_string(),
                video.to_string(),
                format!("{:02}-{:02}.png", fi + 1, pi + 1),
                ((fin - rec) as f64 / 1000.0).max(0.0),
            ));
        }
    }
    if out.len() > MAX_CAPTURAS {
        let paso = out.len() as f64 / MAX_CAPTURAS as f64;
        out = (0..MAX_CAPTURAS).map(|i| out[(i as f64 * paso) as usize].clone()).collect();
    }
    out
}

/// Texto del pedido a Claude (sin efectos, para poder probarlo).
pub fn pedido(record: &Value, dir: &Path, capturas: &[(usize, String, String)]) -> String {
    let flujos = record["flujos"].as_array().cloned().unwrap_or_default();
    let mut t = format!(
        "Eres QA de apps móviles. Revisa esta ejecución de prueba de Clean Studio como lo haría una persona que usa la app.\n\n\
         Carpeta de la ejecución: {}\n",
        dir.display()
    );
    for (fi, f) in flujos.iter().enumerate() {
        t.push_str(&format!(
            "\nFlujo {}: {} ({})\n",
            fi + 1,
            f["nombre"].as_str().unwrap_or("Flujo"),
            if f["estado"] == "COMPLETED" { "pasó" } else { "falló" }
        ));
        for (cf, paso, archivo) in capturas.iter().filter(|c| c.0 == fi) {
            let _ = cf;
            t.push_str(&format!("- {} → revision/{}\n", paso, archivo));
        }
    }
    let extras: Vec<&str> = record["capturas"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !extras.is_empty() {
        t.push_str(&format!("\nCapturas tomadas por el flujo: {}\n", extras.join(", ")));
    }
    let ya: Vec<String> = record["calidad"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|h| h["tipo"] != "claude")
        .filter_map(|h| h["titulo"].as_str().map(|s| format!("- {}", s)))
        .collect();
    if !ya.is_empty() {
        t.push_str(&format!("\nYa detectado (no lo repitas):\n{}\n", ya.join("\n")));
    }
    t.push_str(&format!(
        "\nAbre cada imagen con la herramienta Read, en orden. Busca lo que vería un usuario:\n\
         - textos con faltas, en otro idioma o cortados;\n\
         - datos que no coinciden entre pantallas (nombres, precios, unidades como \"/mes\" en un plan anual, cantidades);\n\
         - elementos encimados, fuera de la pantalla o con poco contraste;\n\
         - botones sin texto o que no dejan claro qué hacen;\n\
         - pantallas vacías, en construcción o cargando más de la cuenta;\n\
         - mensajes de error poco claros.\n\
         No inventes: anota solo lo que se ve en las imágenes.\n\n\
         Al terminar, llama una sola vez a la herramienta guardar_revision con ejecucion=\"{}\" y la lista de hallazgos. \
         Cada hallazgo lleva gravedad (alta, media o baja), titulo (corto), detalle (qué está mal y cómo mejorarlo) y \
         captura (el archivo donde se ve, por ejemplo \"revision/01-03.png\"). Si no encuentras nada, llámala con la lista vacía. \
         Escribe en español.",
        dir.display()
    ));
    t
}

/// Saca las capturas y arma el pedido. Devuelve (pedido, cantidad de capturas).
pub fn preparar(dir: &Path) -> Result<(String, usize), String> {
    let record = leer(dir)?;
    let carpeta = dir.join("revision");
    let _ = std::fs::remove_dir_all(&carpeta);
    std::fs::create_dir_all(&carpeta).map_err(|e| format!("No se pudo preparar la revisión: {}", e))?;
    let lista = momentos(&record);
    if let Some(exe) = crate::parpadeos::extractor() {
        // Una llamada por video
        let mut videos: Vec<&str> = lista.iter().map(|m| m.2.as_str()).collect();
        videos.dedup();
        for v in videos {
            let pares: Vec<String> = lista.iter().filter(|m| m.2 == v).map(|m| format!("{}={:.2}", m.3, m.4)).collect();
            let _ = Command::new(&exe).arg("--png").arg(dir.join(v)).arg("540").arg(&carpeta).args(&pares).status();
        }
    }
    let capturas: Vec<(usize, String, String)> = lista
        .into_iter()
        .filter(|m| carpeta.join(&m.3).exists())
        .map(|m| (m.0, m.1, m.3))
        .collect();
    let extras = record["capturas"].as_array().map(|c| c.len()).unwrap_or(0);
    if capturas.is_empty() && extras == 0 {
        return Err("Esta ejecución no tiene video ni capturas para revisar.".to_string());
    }
    let n = capturas.len() + extras;
    Ok((pedido(&record, dir, &capturas), n))
}

/// Carpeta de la ejecución a partir de su `reporte.html`, solo dentro de las ejecuciones del workspace.
fn carpeta_de(app: &AppHandle, reporte: &str) -> Result<PathBuf, String> {
    let r = PathBuf::from(reporte);
    let dentro = crate::runs::runs_dir(app).is_some_and(|d| r.starts_with(d));
    match r.parent() {
        Some(dir) if dentro && dir.join("ejecucion.json").exists() => Ok(dir.to_path_buf()),
        _ => Err("No se encontró esa ejecución.".to_string()),
    }
}

/// Botón "Revisar con Claude". Devuelve "code" (revisa sin ventana y avisa al terminar) o
/// "desktop" (pedido copiado y Claude Desktop abierto).
pub fn iniciar(app: &AppHandle, reporte: &str) -> Result<Value, String> {
    let dir = carpeta_de(app, reporte)?;
    let (pedido, n) = preparar(&dir)?;
    let Some(cli) = crate::claude::claude_cli() else {
        if !crate::claude::copy(&pedido) {
            return Err("No se pudo copiar el pedido para Claude.".to_string());
        }
        crate::claude::open_claude_desktop();
        return Ok(json!("desktop"));
    };
    eprintln!("[REVISIÓN] Claude revisa {} capturas en {}", n, dir.display());
    let app = app.clone();
    std::thread::spawn(move || {
        let resultado = ejecutar_claude(&cli, &dir, &pedido);
        let record = leer(&dir).unwrap_or(Value::Null);
        let cantidad = record["calidad"].as_array().map(|h| h.iter().filter(|x| x["tipo"] == "claude").count()).unwrap_or(0);
        let (ok, mensaje) = match resultado {
            Ok(()) if record["revisionClaude"].is_object() => (
                true,
                match cantidad {
                    0 => "Claude revisó las pantallas y no encontró nada más.".to_string(),
                    1 => "Claude encontró 1 cosa más. Está en el reporte.".to_string(),
                    n => format!("Claude encontró {} cosas más. Están en el reporte.", n),
                },
            ),
            Ok(()) => (false, "Claude terminó sin guardar la revisión. Vuelve a intentarlo.".to_string()),
            Err(e) => (false, e),
        };
        eprintln!("[REVISIÓN] {}", mensaje);
        bridge::notify("Clean Studio", "Revisión con Claude", &mensaje);
        bridge::send(&app, "cs-revision-claude", vec![json!({ "ok": ok, "mensaje": mensaje, "reporte": dir.join("reporte.html") })]);
    });
    Ok(json!("code"))
}

fn ejecutar_claude(cli: &str, dir: &Path, pedido: &str) -> Result<(), String> {
    // El servidor MCP es este mismo ejecutable: la copia registrada en Claude Code puede ser otra
    // (por ejemplo una versión instalada más vieja, sin guardar_revision)
    let exe = std::env::current_exe().map_err(|e| format!("No se encontró Clean Studio: {}", e))?;
    let config = dir.join("revision").join("mcp.json");
    let servidores = json!({ "mcpServers": { "clean-studio": { "type": "stdio", "command": exe, "args": ["mcp"] } } });
    std::fs::write(&config, servidores.to_string()).map_err(|e| format!("No se pudo preparar la revisión: {}", e))?;
    let mut hijo = Command::new(cli)
        .arg("-p")
        .arg(pedido)
        .arg("--mcp-config")
        .arg(&config)
        .arg("--strict-mcp-config")
        .args(["--allowedTools", "Read", "mcp__clean-studio__guardar_revision"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("No se pudo abrir Claude Code: {}", e))?;
    let limite = Instant::now() + Duration::from_secs(600);
    loop {
        match hijo.try_wait() {
            Ok(Some(estado)) if estado.success() => return Ok(()),
            Ok(Some(_)) => {
                let mut err = String::new();
                if let Some(mut e) = hijo.stderr.take() {
                    let _ = std::io::Read::read_to_string(&mut e, &mut err);
                }
                eprintln!("[REVISIÓN] claude falló: {}", err.trim());
                return Err("Claude no pudo terminar la revisión. Revisa que Claude Code tenga la sesión iniciada.".to_string());
            }
            Ok(None) if Instant::now() > limite => {
                let _ = hijo.kill();
                return Err("La revisión tardó más de 10 minutos y se canceló.".to_string());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(500)),
            Err(e) => return Err(format!("No se pudo seguir la revisión: {}", e)),
        }
    }
}

/// Herramienta MCP `guardar_revision`: añade los hallazgos de Claude a la ejecución y vuelve a
/// generar el reporte. Reemplaza los de una revisión anterior.
pub fn guardar(dir: &Path, hallazgos: &[Value]) -> Result<usize, String> {
    let mut record = leer(dir)?;
    let mut lista: Vec<Value> = record["calidad"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|h| h["tipo"] != "claude")
        .collect();
    let mut n = 0;
    for h in hallazgos {
        let titulo = h["titulo"].as_str().unwrap_or("").trim();
        if titulo.is_empty() {
            continue;
        }
        let gravedad = match h["gravedad"].as_str() {
            Some("alta") => "alta",
            Some("baja") => "baja",
            _ => "media",
        };
        // Solo capturas que existan dentro de la carpeta de la ejecución
        let captura = h["captura"]
            .as_str()
            .map(|c| c.trim_start_matches("./").to_string())
            .filter(|c| !c.contains("..") && dir.join(c).is_file());
        lista.push(json!({
            "gravedad": gravedad,
            "tipo": "claude",
            "titulo": titulo,
            "detalle": h["detalle"].as_str().unwrap_or("").trim(),
            "captura": captura,
        }));
        n += 1;
    }
    let peso = |h: &Value| match h["gravedad"].as_str() {
        Some("alta") => 0,
        Some("media") => 1,
        _ => 2,
    };
    lista.sort_by_key(peso);
    record["calidad"] = json!(lista);
    record["revisionClaude"] = json!({ "fecha": chrono::Local::now().to_rfc3339(), "cantidad": n });
    std::fs::write(dir.join("ejecucion.json"), serde_json::to_string_pretty(&record).unwrap_or_default())
        .map_err(|e| format!("No se pudo guardar la revisión: {}", e))?;
    std::fs::write(dir.join("reporte.html"), reports::render(&record))
        .map_err(|e| format!("No se pudo actualizar el reporte: {}", e))?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registro() -> Value {
        json!({
            "flujos": [{
                "nombre": "Membresías", "estado": "COMPLETED", "video": "video.mp4", "grabacionInicioMs": 1000,
                "pasos": [
                    { "nivel": 0, "paso": "Abrir la app", "inicioMs": 1000, "duracionMs": 2000 },
                    { "nivel": 1, "paso": "Interno", "inicioMs": 1500, "duracionMs": 100 },
                    { "nivel": 0, "paso": "Tocar \"Membresía\"", "inicioMs": 4000, "duracionMs": 500 }
                ]
            }],
            "capturas": ["perfil.png"],
            "calidad": [{ "tipo": "lento", "titulo": "«Abrir la app» tardó 9 s" }]
        })
    }

    #[test]
    fn saca_una_captura_al_final_de_cada_paso_de_primer_nivel() {
        let m = momentos(&registro());
        assert_eq!(m.len(), 2);
        assert_eq!((m[0].3.as_str(), m[0].4), ("01-01.png", 2.0));
        assert_eq!((m[1].3.as_str(), m[1].4), ("01-02.png", 3.5));
    }

    #[test]
    fn el_pedido_lista_pasos_capturas_y_lo_ya_detectado() {
        let caps = vec![(0, "Abrir la app".to_string(), "01-01.png".to_string())];
        let p = pedido(&registro(), Path::new("/w/ej"), &caps);
        assert!(p.contains("- Abrir la app → revision/01-01.png"));
        assert!(p.contains("Capturas tomadas por el flujo: perfil.png"));
        assert!(p.contains("- «Abrir la app» tardó 9 s"));
        assert!(p.contains("ejecucion=\"/w/ej\""));
    }

    #[test]
    fn guardar_reemplaza_la_revision_anterior_y_valida_capturas() {
        let dir = std::env::temp_dir().join(format!("cs-revision-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("revision")).unwrap();
        std::fs::write(dir.join("revision/01-01.png"), b"png").unwrap();
        let mut r = registro();
        r["calidad"].as_array_mut().unwrap().push(json!({ "tipo": "claude", "titulo": "vieja" }));
        std::fs::write(dir.join("ejecucion.json"), r.to_string()).unwrap();
        let n = guardar(
            &dir,
            &[
                json!({ "gravedad": "alta", "titulo": "Plan anual dice /mes", "detalle": "Debe decir /año", "captura": "revision/01-01.png" }),
                json!({ "titulo": "Sin captura válida", "captura": "../../etc/passwd" }),
                json!({ "titulo": "  " }),
            ],
        )
        .unwrap();
        assert_eq!(n, 2);
        let g: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("ejecucion.json")).unwrap()).unwrap();
        let titulos: Vec<&str> = g["calidad"].as_array().unwrap().iter().filter_map(|h| h["titulo"].as_str()).collect();
        assert_eq!(titulos, vec!["Plan anual dice /mes", "Sin captura válida", "«Abrir la app» tardó 9 s"]);
        assert_eq!(g["calidad"][0]["captura"], "revision/01-01.png");
        assert!(g["calidad"][1]["captura"].is_null());
        assert!(dir.join("reporte.html").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
