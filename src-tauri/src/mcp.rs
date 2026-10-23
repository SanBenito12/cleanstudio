//! Servidor MCP de Clean Studio (`clean-studio mcp`): Claude Desktop o Claude Code lo lanzan
//! por stdio y controlan el motor con herramientas en español (dispositivos, pantalla, flujos,
//! reportes). Habla con studio-server por HTTP en localhost:5050; si Clean Studio no está
//! abierto, arranca el motor por su cuenta (con la telemetría bloqueada) y lo cierra al salir.
use crate::{errors, paths, reports, runs, server};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

const BASE: &str = "http://localhost:5050";
const INSTRUCCIONES: &str = "Clean Studio ejecuta pruebas E2E móviles (iOS/Android) escritas en YAML, en el Mac del usuario y sin nube. Flujo de trabajo recomendado: listar_dispositivos → conectar_dispositivo → ver_pantalla para conocer los textos e ids → escribir el flujo YAML (consulta guia_comandos) → validar_flujo → ejecutar_flujo, que deja video y reporte HTML en <workspace>/.clean-studio/ejecuciones. Para probar comandos sueltos sin crear archivos usa ejecutar_comandos. Responde al usuario en español.";

pub fn run() {
    let mut own_server: Option<Child> = None;
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let method = msg["method"].as_str().unwrap_or("");
        let result: Result<Value, (i64, String)> = match method {
            "initialize" => Ok(json!({
                "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "clean-studio", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCCIONES,
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => Ok(call(&msg["params"], &mut own_server)),
            _ => Err((-32601, format!("Método no soportado: {}", method))),
        };
        // Las notificaciones (sin id) no llevan respuesta
        let Some(id) = msg.get("id").cloned() else { continue };
        let response = match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
        };
        let _ = writeln!(out, "{}", response);
        let _ = out.flush();
    }
    if let Some(mut child) = own_server {
        let _ = child.kill();
        let _ = child.wait();
    }
}

// ---------------------------------------------------------------------------
// Herramientas
// ---------------------------------------------------------------------------

fn tools() -> Value {
    let workspace = json!({ "type": "string", "description": "Carpeta del workspace (ruta absoluta). Por defecto, la carpeta del archivo." });
    json!([
        {
            "name": "listar_dispositivos",
            "description": "Lista los simuladores iOS, emuladores Android y navegadores disponibles, con su estado (available, connected…) y su id.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "conectar_dispositivo",
            "description": "Enciende (si hace falta) y conecta un dispositivo por su id. Hace falta un dispositivo conectado para ver la pantalla o ejecutar pruebas.",
            "inputSchema": { "type": "object", "properties": { "id": { "type": "string", "description": "id del dispositivo según listar_dispositivos" } }, "required": ["id"] }
        },
        {
            "name": "ver_pantalla",
            "description": "Captura la pantalla del dispositivo conectado y lista sus elementos (texto, id, posición) para escribir selectores de tapOn, assertVisible, etc.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "ejecutar_comandos",
            "description": "Ejecuta al instante uno o varios comandos YAML en el dispositivo conectado (por ejemplo '- tapOn: \"Entrar\"'), sin crear archivos. Útil para explorar la app.",
            "inputSchema": { "type": "object", "properties": {
                "yaml": { "type": "string", "description": "Comandos en YAML (lista de pasos)" },
                "appId": { "type": "string", "description": "App a usar en launchApp, stopApp o clearState (por ejemplo com.empresa.app)" },
                "workspace": workspace
            }, "required": ["yaml"] }
        },
        {
            "name": "validar_flujo",
            "description": "Comprueba la sintaxis de un flujo YAML sin ejecutarlo.",
            "inputSchema": { "type": "object", "properties": {
                "archivo": { "type": "string", "description": "Ruta absoluta del flujo .yaml" },
                "workspace": workspace
            }, "required": ["archivo"] }
        },
        {
            "name": "ejecutar_flujo",
            "description": "Ejecuta un flujo YAML en el dispositivo conectado y espera el resultado. Devuelve cada paso en español con la explicación de los errores y guarda video, capturas y reporte HTML.",
            "inputSchema": { "type": "object", "properties": {
                "archivo": { "type": "string", "description": "Ruta absoluta del flujo .yaml" },
                "workspace": workspace
            }, "required": ["archivo"] }
        },
        {
            "name": "ultimas_ejecuciones",
            "description": "Lista las ejecuciones guardadas de un workspace (más recientes primero) con su resultado y la ruta del reporte.",
            "inputSchema": { "type": "object", "properties": { "workspace": { "type": "string", "description": "Carpeta del workspace (ruta absoluta)" } }, "required": ["workspace"] }
        },
        {
            "name": "leer_ejecucion",
            "description": "Devuelve el detalle de una ejecución guardada: pasos, errores explicados, video y capturas.",
            "inputSchema": { "type": "object", "properties": { "carpeta": { "type": "string", "description": "Carpeta de la ejecución (de ultimas_ejecuciones)" } }, "required": ["carpeta"] }
        },
        {
            "name": "guardar_revision",
            "description": "Guarda los hallazgos de una revisión visual de una ejecución (los muestra en su reporte, en \"Qué revisar\"). Reemplaza la revisión anterior de esa ejecución.",
            "inputSchema": { "type": "object", "properties": {
                "ejecucion": { "type": "string", "description": "Carpeta de la ejecución (ruta absoluta)" },
                "hallazgos": { "type": "array", "description": "Lista vacía si no hay nada que revisar", "items": { "type": "object", "properties": {
                    "gravedad": { "type": "string", "enum": ["alta", "media", "baja"] },
                    "titulo": { "type": "string", "description": "Qué está mal, corto" },
                    "detalle": { "type": "string", "description": "Por qué importa y cómo mejorarlo" },
                    "captura": { "type": "string", "description": "Imagen donde se ve, relativa a la carpeta (por ejemplo revision/01-03.png)" }
                }, "required": ["titulo"] } }
            }, "required": ["ejecucion", "hallazgos"] }
        },
        {
            "name": "guia_comandos",
            "description": "Referencia rápida en español de los comandos YAML disponibles para escribir flujos.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

fn text(t: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": t.into() }] })
}

fn error(t: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": t.into() }], "isError": true })
}

fn call(params: &Value, own_server: &mut Option<Child>) -> Value {
    let name = params["name"].as_str().unwrap_or("");
    let args = &params["arguments"];
    if name == "guia_comandos" {
        return text(GUIA);
    }
    if matches!(name, "ultimas_ejecuciones" | "leer_ejecucion" | "guardar_revision") {
        return match name {
            "ultimas_ejecuciones" => ultimas_ejecuciones(args),
            "guardar_revision" => guardar_revision(args),
            _ => leer_ejecucion(args),
        };
    }
    if let Err(e) = ensure_server(own_server) {
        return error(e);
    }
    match name {
        "listar_dispositivos" => listar_dispositivos(),
        "conectar_dispositivo" => conectar_dispositivo(args),
        "ver_pantalla" => ver_pantalla(),
        "ejecutar_comandos" => ejecutar_comandos(args),
        "validar_flujo" => validar_flujo(args),
        "ejecutar_flujo" => ejecutar_flujo(args),
        other => error(format!("Herramienta desconocida: {}", other)),
    }
}

fn devices() -> Result<Vec<Value>, String> {
    sse_first("/api/devices/events", 15, |v| v.is_array())
        .and_then(|v| v.as_array().cloned())
        .ok_or_else(|| "No se pudo leer la lista de dispositivos del motor.".to_string())
}

fn connected_device() -> Result<Value, String> {
    devices()?
        .into_iter()
        .find(|d| d["state"] == "connected" && d["deviceType"] != "BROWSER")
        .or_else(|| devices().ok()?.into_iter().find(|d| d["state"] == "connected"))
        .ok_or_else(|| "No hay ningún dispositivo conectado. Usa listar_dispositivos y conectar_dispositivo.".to_string())
}

fn listar_dispositivos() -> Value {
    match devices() {
        Ok(list) => {
            let lines: Vec<String> = list
                .iter()
                .map(|d| {
                    format!(
                        "- {} · {} {} · estado: {} · id: {}",
                        d["description"].as_str().unwrap_or("?"),
                        d["platform"].as_str().unwrap_or(""),
                        d["deviceType"].as_str().unwrap_or(""),
                        d["state"].as_str().unwrap_or(""),
                        d["instanceId"].as_str().or(d["modelId"].as_str()).unwrap_or("")
                    )
                })
                .collect();
            text(format!("{} dispositivos:\n{}", lines.len(), lines.join("\n")))
        }
        Err(e) => error(e),
    }
}

fn conectar_dispositivo(args: &Value) -> Value {
    let Some(id) = args["id"].as_str() else { return error("Falta el id del dispositivo.") };
    crate::simulator::liberar_driver_ios(id);
    if let Err(e) = http("POST", &format!("/api/devices/{}/connect", encode(id)), Some(&json!({})), 60) {
        return error(format!("No se pudo conectar {}: {}", id, e));
    }
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline {
        if let Ok(list) = devices() {
            if let Some(d) = list.iter().find(|d| d["instanceId"] == id || d["modelId"] == id) {
                if d["state"] == "connected" {
                    return text(format!("Conectado: {}", d["description"].as_str().unwrap_or(id)));
                }
                if let Some(err) = d["error"].as_str() {
                    return error(format!("El dispositivo informó un error: {}", err));
                }
            }
        }
        thread::sleep(Duration::from_secs(2));
    }
    error("El dispositivo no terminó de conectarse en 3 minutos.")
}

fn ver_pantalla() -> Value {
    let device = match connected_device() {
        Ok(d) => d,
        Err(e) => return error(e),
    };
    let id = device["instanceId"].as_str().unwrap_or("");
    let Some(screen) = sse_first(&format!("/api/devices/deviceScreen/sse?instanceId={}", encode(id)), 30, |v| {
        v.get("screenshot").is_some()
    }) else {
        return error("No se pudo capturar la pantalla del dispositivo.");
    };
    let mut lines = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for e in screen["elements"].as_array().into_iter().flatten() {
        let texto = e["text"].as_str().or(e["accessibilityText"].as_str()).unwrap_or("");
        let id = e["resourceId"].as_str().unwrap_or("");
        let hint = e["hintText"].as_str().unwrap_or("");
        if texto.is_empty() && id.is_empty() && hint.is_empty() {
            continue;
        }
        let b = &e["bounds"];
        let mut desc = Vec::new();
        if !texto.is_empty() {
            desc.push(format!("texto \"{}\"", texto));
        }
        if !id.is_empty() && id != texto {
            desc.push(format!("id \"{}\"", id));
        }
        if !hint.is_empty() {
            desc.push(format!("pista \"{}\"", hint));
        }
        let line = format!("- {} · ({}, {}) {}×{}", desc.join(" · "), b["x"], b["y"], b["width"], b["height"]);
        // iOS reporta algunos elementos dos veces (contenedor y etiqueta)
        if !seen.insert(line.clone()) || texto.trim().is_empty() && id.is_empty() {
            continue;
        }
        lines.push(line);
        if lines.len() >= 150 {
            break;
        }
    }
    let resumen = format!(
        "{} · pantalla {}×{} · {} elementos con texto o id:\n{}",
        device["description"].as_str().unwrap_or(""),
        screen["width"],
        screen["height"],
        lines.len(),
        lines.join("\n")
    );
    let mut content = vec![json!({ "type": "text", "text": resumen })];
    if let Some(img) = screen["screenshot"].as_str() {
        content.push(json!({ "type": "image", "data": img, "mimeType": "image/jpeg" }));
    }
    json!({ "content": content })
}

fn workspace_for(args: &Value, archivo: Option<&Path>) -> PathBuf {
    args["workspace"]
        .as_str()
        .map(PathBuf::from)
        .or_else(|| archivo.and_then(|a| a.parent().map(Path::to_path_buf)))
        .unwrap_or_else(paths::home_dir)
}

fn icono(estado: &str) -> &'static str {
    match estado {
        "COMPLETED" => "✓",
        "FAILED" => "✗",
        "SKIPPED" => "–",
        "WARNED" => "!",
        _ => "·",
    }
}

/// Pasos (de `runs::build_record` o de commandStatuses) en texto legible.
fn describe_steps(pasos: &[Value]) -> String {
    let mut out = Vec::new();
    for p in pasos {
        let nivel = p["nivel"].as_u64().unwrap_or(0) as usize;
        out.push(format!(
            "{}{} {}",
            "  ".repeat(nivel),
            icono(p["estado"].as_str().unwrap_or("")),
            p["paso"].as_str().unwrap_or("")
        ));
        if let Some(err) = p["error"].as_str().filter(|e| !e.is_empty()) {
            let explicacion = p["explicacion"].as_str().unwrap_or("");
            let sugerencia = p["sugerencia"].as_str().unwrap_or("");
            if !explicacion.is_empty() {
                out.push(format!("{}    → {} {}", "  ".repeat(nivel), explicacion, sugerencia));
            }
            out.push(format!("{}    Error del motor: {}", "  ".repeat(nivel), err));
        }
    }
    out.join("\n")
}

fn guardar_revision(args: &Value) -> Value {
    let Some(dir) = args["ejecucion"].as_str().map(PathBuf::from) else { return error("Falta la carpeta de la ejecución.") };
    let hallazgos = args["hallazgos"].as_array().cloned().unwrap_or_default();
    match crate::revision::guardar(&dir, &hallazgos) {
        Ok(n) => text(format!("Revisión guardada con {} hallazgos. Reporte: {}", n, dir.join("reporte.html").display())),
        Err(e) => error(e),
    }
}

fn ejecutar_comandos(args: &Value) -> Value {
    let Some(yaml) = args["yaml"].as_str() else { return error("Falta el YAML de los comandos.") };
    // Sin encabezado, launchApp/stopApp/clearState no saben qué app usar: se añade el appId
    let con_app;
    let yaml = match args["appId"].as_str().filter(|a| !a.is_empty()) {
        Some(app) if !yaml.contains("appId:") => {
            con_app = format!("appId: {}\n---\n{}", app, yaml);
            con_app.as_str()
        }
        _ => yaml,
    };
    if let Err(e) = connected_device() {
        return error(e);
    }
    let workspace = workspace_for(args, None);
    let body = json!({ "yaml": yaml, "workspacePath": workspace, "env": {} });
    match http("POST", "/api/devices/runCommand", Some(&body), 300) {
        Ok(resp) => {
            let statuses = json!({ "commandStatuses": resp["commandStatuses"].clone(), "filepath": "", "flowStatus": "" });
            let record = runs::build_record("flow", &statuses, SystemTime::now(), None, &[], &[]);
            let pasos = record["flujos"][0]["pasos"].as_array().cloned().unwrap_or_default();
            let ok = resp["success"].as_bool().unwrap_or(false);
            let mut t = format!("{}\n{}", if ok { "Comandos ejecutados." } else { "Los comandos fallaron." }, describe_steps(&pasos));
            if let Some(e) = resp["error"].as_str().filter(|e| !e.is_empty()) {
                if let Some((explicacion, sugerencia)) = errors::explicar(e) {
                    t.push_str(&format!("\n→ {} {}", explicacion, sugerencia));
                }
                t.push_str(&format!("\nError del motor: {}", e));
            }
            if ok { text(t) } else { error(t) }
        }
        Err(e) => error(format!("No se pudieron ejecutar los comandos: {}", e)),
    }
}

fn flow_request(args: &Value) -> Result<(PathBuf, PathBuf, Value), String> {
    let archivo = PathBuf::from(args["archivo"].as_str().ok_or("Falta la ruta del flujo (archivo).")?);
    let workspace = workspace_for(args, Some(&archivo));
    let archivo = if archivo.is_absolute() { archivo } else { workspace.join(archivo) };
    let yaml = std::fs::read_to_string(&archivo).map_err(|e| format!("No se pudo leer {}: {}", archivo.display(), e))?;
    let rel = archivo
        .strip_prefix(&workspace)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| archivo.to_string_lossy().to_string());
    let body = json!({ "yaml": yaml, "workspacePath": workspace, "filePath": rel, "env": {} });
    Ok((archivo, workspace, body))
}

fn dry_run(body: &Value) -> Result<String, String> {
    let mut b = body.clone();
    b["dryRun"] = json!(true);
    let resp = http("POST", "/api/devices/runFlowFile", Some(&b), 60)?;
    if resp["success"].as_bool() != Some(true) {
        let detalle = resp["details"].as_str().or(resp["error"].as_str()).unwrap_or("el motor rechazó el flujo");
        let mut msg = format!("El flujo no es válido: {}", detalle);
        if let Some((explicacion, sugerencia)) = errors::explicar(detalle) {
            msg.push_str(&format!("\n→ {} {}", explicacion, sugerencia));
        }
        return Err(msg);
    }
    resp["flowId"].as_str().map(String::from).ok_or_else(|| "El motor no devolvió un id de ejecución.".to_string())
}

fn validar_flujo(args: &Value) -> Value {
    match flow_request(args).and_then(|(archivo, _, body)| dry_run(&body).map(|_| archivo)) {
        Ok(archivo) => text(format!("El flujo {} es válido.", archivo.display())),
        Err(e) => error(e),
    }
}

fn ejecutar_flujo(args: &Value) -> Value {
    let (archivo, workspace, body) = match flow_request(args) {
        Ok(r) => r,
        Err(e) => return error(e),
    };
    let device = match connected_device() {
        Ok(d) => d,
        Err(e) => return error(e),
    };
    let flow_id = match dry_run(&body) {
        Ok(id) => id,
        Err(e) => return error(e),
    };
    let started = SystemTime::now();
    let (mut sse, rx) = sse_stream(
        &format!(
            "/api/devices/flowStatus/sse?flowId={}&filepath={}",
            encode(&flow_id),
            encode(&archivo.to_string_lossy())
        ),
        900,
    );
    thread::sleep(Duration::from_millis(500));
    let mut run_body = body.clone();
    run_body["flowId"] = json!(flow_id);
    if let Err(e) = http("POST", "/api/devices/runFlowFile", Some(&run_body), 60) {
        let _ = sse.kill();
        return error(format!("No se pudo iniciar el flujo: {}", e));
    }
    let mut last = Value::Null;
    while let Ok(msg) = rx.recv_timeout(Duration::from_secs(900)) {
        let status = msg["flowStatus"].as_str().unwrap_or("").to_string();
        last = msg;
        if status == "COMPLETED" || status == "FAILED" {
            break;
        }
    }
    let _ = sse.kill();
    let _ = sse.wait();
    if last.is_null() {
        return error("No llegó ningún estado de la ejecución.");
    }

    // Evidencia, igual que desde la app
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dir = workspace
        .join(".clean-studio")
        .join("ejecuciones")
        .join(format!("{}-{}", stamp, runs::slug(&archivo.to_string_lossy())));
    let _ = std::fs::create_dir_all(&dir);
    let videos = runs::copy_videos("flow", &last, &dir);
    let capturas = runs::collect_screenshots(&dir, started);
    let device_json = json!({
        "plataforma": device["platform"].as_str().unwrap_or(""),
        "id": device["instanceId"].as_str().unwrap_or(""),
        "nombre": device["description"].as_str().unwrap_or(""),
    });
    let mut record = runs::build_record("flow", &last, started, Some(device_json), &videos, &capturas);
    let adb = paths::detect_android_sdk().map(|sdk| Command::new(PathBuf::from(sdk).join("platform-tools").join("adb")));
    record["calidad"] = json!(crate::calidad::informe(
        &record,
        &dir,
        device["platform"].as_str().unwrap_or(""),
        device["instanceId"].as_str().unwrap_or(""),
        adb,
        started,
    ));
    let _ = std::fs::write(dir.join("ejecucion.json"), serde_json::to_string_pretty(&record).unwrap_or_default());
    let reporte = dir.join("reporte.html");
    let _ = std::fs::write(&reporte, reports::render(&record));

    let ok = record["estado"] == "COMPLETED";
    let pasos = record["flujos"][0]["pasos"].as_array().cloned().unwrap_or_default();
    let mut t = format!(
        "{} — {} ({} ms)\n{}\n\nReporte: {}",
        if ok { "Prueba aprobada" } else { "La prueba falló" },
        archivo.display(),
        record["duracionMs"],
        describe_steps(&pasos),
        reporte.display()
    );
    // Lo que hay que revisar, para que Claude lo explique o lo corrija
    if let Some(h) = record["calidad"].as_array().filter(|h| !h.is_empty()) {
        t.push_str("\n\nQué revisar:");
        for x in h {
            t.push_str(&format!(
                "\n- [{}] {}: {}",
                x["gravedad"].as_str().unwrap_or(""),
                x["titulo"].as_str().unwrap_or(""),
                x["detalle"].as_str().unwrap_or("")
            ));
        }
    }
    for v in &videos {
        t.push_str(&format!("\nVideo: {}", dir.join(v).display()));
    }
    for c in &capturas {
        t.push_str(&format!("\nCaptura: {}", dir.join(c).display()));
    }
    if ok { text(t) } else { error(t) }
}

fn ultimas_ejecuciones(args: &Value) -> Value {
    let Some(ws) = args["workspace"].as_str() else { return error("Falta la carpeta del workspace.") };
    let base = Path::new(ws).join(".clean-studio").join("ejecuciones");
    let Ok(rd) = std::fs::read_dir(&base) else { return text("Este workspace todavía no tiene ejecuciones guardadas.") };
    let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs.reverse();
    let lines: Vec<String> = dirs
        .iter()
        .take(15)
        .map(|d| {
            let record: Value = std::fs::read_to_string(d.join("ejecucion.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            let estado = match record["estado"].as_str() {
                Some("COMPLETED") => "aprobada",
                Some("FAILED") => "falló",
                _ => "incompleta",
            };
            let nombres: Vec<&str> = record["flujos"].as_array().into_iter().flatten().filter_map(|f| f["nombre"].as_str()).collect();
            format!("- {} · {} · {}\n  carpeta: {}", record["inicio"].as_str().unwrap_or(""), estado, nombres.join(", "), d.display())
        })
        .collect();
    text(format!("{} ejecuciones (más recientes primero):\n{}", lines.len(), lines.join("\n")))
}

fn leer_ejecucion(args: &Value) -> Value {
    let Some(carpeta) = args["carpeta"].as_str() else { return error("Falta la carpeta de la ejecución.") };
    let mut dir = PathBuf::from(carpeta);
    if dir.is_file() {
        dir = dir.parent().map(Path::to_path_buf).unwrap_or(dir);
    }
    let Some(record) = std::fs::read_to_string(dir.join("ejecucion.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
        return error(format!("No hay ejecucion.json en {}", dir.display()));
    };
    let mut t = format!(
        "Ejecución {} · estado {} · inicio {} · duración {} ms\n",
        dir.display(),
        record["estado"].as_str().unwrap_or(""),
        record["inicio"].as_str().unwrap_or(""),
        record["duracionMs"]
    );
    for f in record["flujos"].as_array().into_iter().flatten() {
        t.push_str(&format!(
            "\n{} ({})\n{}\n",
            f["nombre"].as_str().unwrap_or(""),
            f["archivo"].as_str().unwrap_or(""),
            describe_steps(f["pasos"].as_array().map(Vec::as_slice).unwrap_or(&[]))
        ));
    }
    t.push_str(&format!("\nReporte: {}", dir.join("reporte.html").display()));
    for v in record["videos"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        t.push_str(&format!("\nVideo: {}", dir.join(v).display()));
    }
    text(t)
}

// ---------------------------------------------------------------------------
// Motor y HTTP (curl evita dependencias y maneja SSE y chunked)
// ---------------------------------------------------------------------------

fn server_up() -> bool {
    std::net::TcpStream::connect_timeout(&"127.0.0.1:5050".parse().unwrap(), Duration::from_millis(300)).is_ok()
}

fn ensure_server(own: &mut Option<Child>) -> Result<(), String> {
    if server_up() {
        return Ok(());
    }
    let java = paths::java_bin(None);
    let jar = paths::studio_jar(None);
    if !java.exists() || !jar.exists() {
        return Err("No se encontró el motor de Clean Studio (JVM o studio-server.jar). Abre Clean Studio o reinstálalo.".into());
    }
    let logs = paths::home_dir().join("Library/Application Support/com.cleanstudio.desktop/logs");
    let _ = std::fs::create_dir_all(&logs);
    let mut cmd = Command::new(&java);
    server::apply_no_telemetry(&mut cmd);
    cmd.arg(format!("-Dlog.path={}", logs.join("studio-server-mcp.log").display()))
        .arg("-jar")
        .arg(&jar)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut path_var = std::env::var("PATH").unwrap_or_default();
    if let Some(sdk) = paths::detect_android_sdk() {
        path_var = format!("{}:{}/platform-tools:{}/emulator", path_var, sdk, sdk);
        cmd.env("ANDROID_HOME", &sdk);
    }
    cmd.env("PATH", format!("{}:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin", path_var));
    let child = cmd.spawn().map_err(|e| format!("No se pudo iniciar el motor: {}", e))?;
    *own = Some(child);
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if server_up() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err("El motor no respondió en 60 segundos.".into())
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect()
}

fn http(method: &str, path: &str, body: Option<&Value>, timeout_secs: u64) -> Result<Value, String> {
    let mut cmd = Command::new("curl");
    cmd.args(["-s", "-m", &timeout_secs.to_string(), "-X", method, "-H", "Content-Type: application/json"]);
    if body.is_some() {
        cmd.args(["--data-binary", "@-"]);
    }
    cmd.arg(format!("{}{}", BASE, path)).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    if let (Some(b), Some(mut stdin)) = (body, child.stdin.take()) {
        let _ = stdin.write_all(b.to_string().as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("el motor no respondió (curl {})", out.status.code().unwrap_or(-1)));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|_| text.chars().take(300).collect())
}

/// Abre un SSE y entrega cada mensaje `data:` ya parseado.
fn sse_stream(path: &str, timeout_secs: u64) -> (Child, mpsc::Receiver<Value>) {
    let (tx, rx) = mpsc::channel();
    let mut child = Command::new("curl")
        .args(["-s", "-N", "-m", &timeout_secs.to_string()])
        .arg(format!("{}{}", BASE, path))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("curl no disponible");
    let stdout = child.stdout.take().expect("stdout");
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(data) = line.strip_prefix("data:") {
                if let Ok(v) = serde_json::from_str::<Value>(data.trim()) {
                    if tx.send(v).is_err() {
                        break;
                    }
                }
            }
        }
    });
    (child, rx)
}

fn sse_first(path: &str, timeout_secs: u64, accept: impl Fn(&Value) -> bool) -> Option<Value> {
    let (mut child, rx) = sse_stream(path, timeout_secs);
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut found = None;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(v) if accept(&v) => {
                found = Some(v);
                break;
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    found
}

const GUIA: &str = r#"Guía rápida de comandos YAML (Clean Studio)

Encabezado del flujo:
  appId: com.empresa.app        # bundle id (iOS) o paquete (Android); para web: url: https://…
  name: "Nombre del flujo"
  ---

Abrir y cerrar la app:
  - launchApp                    # o con opciones: launchApp: { clearState: true }
  - stopApp
  - clearState                   # no funciona con apps del sistema de iOS

Tocar y escribir:
  - tapOn: "Entrar"              # por texto visible
  - tapOn: { id: "btn_login" }   # por id (resourceId / accessibilityIdentifier)
  - tapOn: { point: "50%,90%" }  # por coordenadas relativas
  - doubleTapOn: "Foto"
  - longPressOn: "Mensaje"
  - inputText: "usuario@correo.com"
  - eraseText: 10
  - hideKeyboard
  - pressKey: Enter              # Home, Back, Enter, Backspace…

Verificar:
  - assertVisible: "Bienvenido"
  - assertNotVisible: "Error"
  - extendedWaitUntil: { visible: "Catálogo", timeout: 10000 }
  - assertTrue: ${output.total > 0}

Desplazar y gestos:
  - scroll
  - scrollUntilVisible: { element: "Agregar al carrito", direction: DOWN }
  - swipe: { direction: LEFT }
  - back                          # Android

Evidencia y control:
  - takeScreenshot: "nombre"     # se guarda en la carpeta de la ejecución
  - waitForAnimationToEnd
  - runFlow: otro-flujo.yaml
  - repeat: { times: 3, commands: [ { tapOn: "Siguiente" } ] }
  - openLink: https://ejemplo.com

Variables:
  env:
    CORREO: "qa@ejemplo.com"     # se usan como ${CORREO}
"#;
