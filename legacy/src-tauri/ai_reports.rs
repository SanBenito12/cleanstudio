use crate::engines::base_project_dir;
use crate::models::{AiFlowRequest, AiFlowResponse, AppState, ExportReportRequest, WorkspaceFile};
use chrono::Local;
use std::fs;
use std::path::PathBuf;
use tauri::State;

pub fn ensure_sample_flows_internal() -> PathBuf {
    let dir = base_project_dir().join("sample-flows");
    let _ = fs::create_dir_all(&dir);

    let login_flow = dir.join("01-inicio-sesion-compra.yaml");
    if !login_flow.exists() {
        let _ = fs::write(
            &login_flow,
            r#"appId: com.ejemplo.tienda
name: "Flujo 01 — Inicio de Sesión y Carrito de Compras"
tags:
  - humo
  - produccion
env:
  CORREO_DEMO: "usuario@cleanstudio.dev"
---
# 1. Lanzar la aplicación limpiando el estado previo
- launchApp:
    clearState: true

# 2. Verificar que la pantalla de bienvenida está visible
- assertVisible: "Iniciar sesión"
- tapOn:
    id: "input_correo"
- inputText: ${CORREO_DEMO}

# 3. Ingresar contraseña y entrar
- tapOn:
    id: "input_password"
- inputText: "ClaveSegura2026!"
- tapOn: "Entrar"

# 4. Validar acceso al catálogo y agregar producto
- extendedWaitUntil:
    visible: "Catálogo Principal"
    timeout: 8000
- scrollUntilVisible:
    element: "Agregar al carrito"
    direction: DOWN
- tapOn: "Agregar al carrito"
- assertVisible: "1 artículo en tu carrito"
- takeScreenshot: "compra-exitosa"
"#,
        );
    }

    let web_flow = dir.join("02-prueba-web-busqueda.yaml");
    if !web_flow.exists() {
        let _ = fs::write(
            &web_flow,
            r#"url: https://example.com
name: "Flujo 02 — Automatización Web E2E"
tags:
  - web
  - regresion
---
- launchApp
- assertVisible: "Example Domain"
- takeScreenshot: "portada-web"
"#,
        );
    }

    let onboarding_flow = dir.join("03-registro-y-permisos.yaml");
    if !onboarding_flow.exists() {
        let _ = fs::write(
            &onboarding_flow,
            r#"appId: com.ejemplo.appmovil
name: "Flujo 03 — Onboarding y Permisos Nativos"
tags:
  - onboarding
---
- launchApp:
    clearState: true
    permissions:
      notifications: allow
      location: allow
- swipe:
    direction: LEFT
- swipe:
    direction: LEFT
- tapOn: "Comenzar ahora"
- assertVisible: "Mi Perfil"
"#,
        );
    }

    dir
}

#[tauri::command]
pub fn list_workspace_files(workspace_dir: Option<String>) -> Result<Vec<WorkspaceFile>, String> {
    let dir = match workspace_dir {
        Some(ref p) if !p.trim().is_empty() && PathBuf::from(p).exists() => PathBuf::from(p),
        _ => ensure_sample_flows_internal(),
    };

    let mut files = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("Error leyendo directorio: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let is_yaml = name.ends_with(".yaml") || name.ends_with(".yml") || name.ends_with(".js");
        if is_dir || is_yaml {
            let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            files.push(WorkspaceFile {
                name,
                path: path.to_string_lossy().to_string(),
                is_dir,
                size_bytes,
                modified: Local::now().format("%Y-%m-%d %H:%M").to_string(),
            });
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

#[tauri::command]
pub fn read_flow_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("No se pudo leer el archivo {}: {}", path, e))
}

#[tauri::command]
pub fn save_flow_file(path: String, content: String) -> Result<String, String> {
    if let Some(parent) = PathBuf::from(&path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&path, content).map_err(|e| format!("No se pudo guardar el archivo {}: {}", path, e))?;
    Ok(format!("Archivo guardado correctamente en {}", path))
}

#[tauri::command]
pub fn create_flow_file(workspace_dir: Option<String>, file_name: String, content: String) -> Result<String, String> {
    let dir = match workspace_dir {
        Some(ref p) if !p.trim().is_empty() && PathBuf::from(p).exists() => PathBuf::from(p),
        _ => ensure_sample_flows_internal(),
    };
    let clean_name = if file_name.ends_with(".yaml") || file_name.ends_with(".yml") {
        file_name
    } else {
        format!("{}.yaml", file_name)
    };
    let target = dir.join(clean_name);
    fs::write(&target, content).map_err(|e| format!("Error al crear archivo: {}", e))?;
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
pub fn delete_flow_file(path: String) -> Result<String, String> {
    fs::remove_file(&path).map_err(|e| format!("No se pudo eliminar {}: {}", path, e))?;
    Ok("Archivo eliminado.".to_string())
}

#[tauri::command]
pub fn get_server_logs(state: State<'_, AppState>) -> Vec<String> {
    state
        .server_logs
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default()
}

#[tauri::command]
pub async fn generate_flow_with_ai(req: AiFlowRequest) -> Result<AiFlowResponse, String> {
    let app_id = req.app_id.unwrap_or_else(|| "com.ejemplo.app".to_string());
    let system_prompt = format!(
        "Eres el Asistente IA experto de Clean Studio para automatización móvil y web con Maestro YAML. \
         Genera SIEMPRE código YAML válido de Maestro comentado en español para appId: {}. \
         Devuelve únicamente el bloque YAML limpio sin texto extra fuera de comentarios YAML.",
        app_id
    );
    let user_msg = format!(
        "Instrucción del usuario: {}\nContexto YAML actual:\n{}\nJerarquía de elementos:\n{}",
        req.prompt,
        req.current_yaml.unwrap_or_default(),
        req.view_hierarchy_summary.unwrap_or_default()
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    if req.provider == "ollama" {
        let body = serde_json::json!({
            "model": if req.model.is_empty() { "qwen2.5-coder:7b" } else { &req.model },
            "prompt": format!("{}\n\n{}", system_prompt, user_msg),
            "stream": false
        });
        if let Ok(resp) = client.post("http://127.0.0.1:11434/api/generate").json(&body).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(text) = json.get("response").and_then(|v| v.as_str()) {
                    return Ok(AiFlowResponse {
                        yaml: text.trim().trim_matches('`').replace("yaml\n", ""),
                        explanation: "Flujo generado localmente con Ollama (100% privado y sin costo).".to_string(),
                        provider_used: format!("Ollama ({})", req.model),
                    });
                }
            }
        }
    } else if req.provider == "openai" {
        if let Some(ref key) = req.api_key {
            if !key.trim().is_empty() {
                let body = serde_json::json!({
                    "model": if req.model.is_empty() { "gpt-4o-mini" } else { &req.model },
                    "messages": [
                        { "role": "system", "content": system_prompt },
                        { "role": "user", "content": user_msg }
                    ]
                });
                if let Ok(resp) = client
                    .post("https://api.openai.com/v1/chat/completions")
                    .bearer_auth(key.trim())
                    .json(&body)
                    .send()
                    .await
                {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                            return Ok(AiFlowResponse {
                                yaml: content.trim().trim_matches('`').replace("yaml\n", ""),
                                explanation: "Flujo generado mediante tu API Key de OpenAI.".to_string(),
                                provider_used: "OpenAI API".to_string(),
                            });
                        }
                    }
                }
            }
        }
    } else if req.provider == "gemini" {
        if let Some(ref key) = req.api_key {
            if !key.trim().is_empty() {
                let model_name = if req.model.is_empty() { "gemini-2.5-flash" } else { &req.model };
                let url = format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                    model_name,
                    key.trim()
                );
                let body = serde_json::json!({
                    "contents": [{
                        "parts": [{ "text": format!("{}\n\n{}", system_prompt, user_msg) }]
                    }]
                });
                if let Ok(resp) = client.post(&url).json(&body).send().await {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(content) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                            return Ok(AiFlowResponse {
                                yaml: content.trim().trim_matches('`').replace("yaml\n", ""),
                                explanation: "Flujo generado mediante Google Gemini API.".to_string(),
                                provider_used: "Google Gemini".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Motor heurístico instantáneo en español (sin conexión ni API keys requeridas)
    let prompt_lower = req.prompt.to_lowercase();
    let mut steps = Vec::new();
    steps.push(format!("appId: {}", app_id));
    steps.push(format!("name: \"Flujo IA — {}\"", req.prompt.chars().take(48).collect::<String>()));
    steps.push("---".to_string());
    steps.push("# Generado por el Motor Inteligente Local de Clean Studio".to_string());
    steps.push("- launchApp:".to_string());
    steps.push("    clearState: true".to_string());

    if prompt_lower.contains("login") || prompt_lower.contains("sesión") || prompt_lower.contains("sesion") || prompt_lower.contains("entrar") {
        steps.push("- assertVisible: \"Iniciar sesión\"".to_string());
        steps.push("- tapOn:".to_string());
        steps.push("    id: \"input_email\"".to_string());
        steps.push("- inputText: \"usuario@cleanstudio.dev\"".to_string());
        steps.push("- tapOn:".to_string());
        steps.push("    id: \"input_password\"".to_string());
        steps.push("- inputText: \"ClaveSegura123\"".to_string());
        steps.push("- tapOn: \"Entrar\"".to_string());
    }
    if prompt_lower.contains("carrito") || prompt_lower.contains("compra") || prompt_lower.contains("producto") {
        steps.push("- scrollUntilVisible:".to_string());
        steps.push("    element: \"Agregar al carrito\"".to_string());
        steps.push("    direction: DOWN".to_string());
        steps.push("- tapOn: \"Agregar al carrito\"".to_string());
        steps.push("- assertVisible: \"Carrito (1)\"".to_string());
    }
    if prompt_lower.contains("buscar") || prompt_lower.contains("búsqueda") || prompt_lower.contains("busqueda") {
        steps.push("- tapOn: \"Buscar\"".to_string());
        steps.push("- inputText: \"consulta de prueba\"".to_string());
        steps.push("- pressKey: Enter".to_string());
    }
    steps.push("- takeScreenshot: \"evidencia-clean-studio\"".to_string());

    Ok(AiFlowResponse {
        yaml: steps.join("\n"),
        explanation: "Flujo sintetizado al instante por el Motor Inteligente Offline en Español de Clean Studio.".to_string(),
        provider_used: "Clean Studio Offline AI (Rust)".to_string(),
    })
}

#[tauri::command]
pub fn export_html_report(req: ExportReportRequest) -> Result<String, String> {
    let reports_dir = base_project_dir().join("reports");
    let _ = fs::create_dir_all(&reports_dir);
    let ts = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let filename = format!("reporte-{}-{}.html", req.flow_name.replace(' ', "-").to_lowercase(), ts);
    let report_path = reports_dir.join(filename);

    let passed = req.steps.iter().filter(|s| s.status == "COMPLETED" || s.status == "PASSED").count();
    let failed = req.steps.iter().filter(|s| s.status == "FAILED").count();

    let mut rows_html = String::new();
    for (idx, step) in req.steps.iter().enumerate() {
        let badge_color = if step.status == "COMPLETED" || step.status == "PASSED" {
            "#10b981"
        } else if step.status == "FAILED" {
            "#ef4444"
        } else {
            "#f59e0b"
        };
        let status_label = if step.status == "COMPLETED" || step.status == "PASSED" {
            "EXITOSO"
        } else if step.status == "FAILED" {
            "FALLIDO"
        } else {
            "PENDIENTE"
        };
        let err_block = step
            .error_es
            .as_ref()
            .map(|e| format!("<div style='margin-top:6px;color:#fca5a5;font-size:13px;'>Diagnóstico en Español: {}</div>", e))
            .unwrap_or_default();

        rows_html.push_str(&format!(
            "<tr>
                <td style='padding:12px;border-bottom:1px solid #1e293b;'>#{}</td>
                <td style='padding:12px;border-bottom:1px solid #1e293b;font-family:monospace;color:#e2e8f0;'>{}{}</td>
                <td style='padding:12px;border-bottom:1px solid #1e293b;'><span style='background:{};color:#020617;padding:4px 10px;border-radius:999px;font-weight:700;font-size:12px;'>{}</span></td>
                <td style='padding:12px;border-bottom:1px solid #1e293b;color:#94a3b8;'>{} ms</td>
            </tr>",
            idx + 1,
            step.command,
            err_block,
            badge_color,
            status_label,
            step.duration_ms
        ));
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="es">
<head>
<meta charset="UTF-8" />
<title>Reporte Clean Studio — {flow}</title>
<style>
  body {{ background:#020617; color:#f8fafc; font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif; padding:40px; margin:0; }}
  .card {{ background:#0f172a; border:1px solid #1e293b; border-radius:16px; padding:24px; margin-bottom:24px; }}
  h1 {{ margin:0 0 8px 0; color:#34d399; }}
  table {{ width:100%; border-collapse:collapse; }}
  th {{ text-align:left; padding:12px; border-bottom:2px solid #334155; color:#94a3b8; font-size:13px; text-transform:uppercase; }}
  pre {{ background:#020617; padding:16px; border-radius:12px; border:1px solid #1e293b; color:#38bdf8; overflow-x:auto; }}
</style>
</head>
<body>
  <div class="card">
    <h1>Clean Studio — Reporte de Ejecución E2E</h1>
    <p style="color:#94a3b8;margin:0;">Generado localmente en Español • Cero Telemetría • {date}</p>
    <div style="display:flex;gap:24px;margin-top:20px;flex-wrap:wrap;">
      <div><strong>Flujo:</strong> {flow}</div>
      <div><strong>App ID:</strong> <code>{app}</code></div>
      <div><strong>Dispositivo:</strong> {device}</div>
      <div><strong>Duración Total:</strong> {dur} ms</div>
      <div style="color:#34d399;"><strong>Pasos Exitosos:</strong> {passed}</div>
      <div style="color:#f87171;"><strong>Pasos Fallidos:</strong> {failed}</div>
    </div>
  </div>
  <div class="card">
    <h2 style="margin-top:0;">Desglose Paso a Paso</h2>
    <table>
      <thead><tr><th>Paso</th><th>Comando Maestro</th><th>Estado</th><th>Duración</th></tr></thead>
      <tbody>{rows}</tbody>
    </table>
  </div>
  <div class="card">
    <h2 style="margin-top:0;">Código YAML Ejecutado</h2>
    <pre>{yaml}</pre>
  </div>
</body>
</html>"#,
        flow = req.flow_name,
        date = Local::now().format("%d/%m/%Y %H:%M:%S"),
        app = req.app_id,
        device = req.device_name,
        dur = req.total_duration_ms,
        passed = passed,
        failed = failed,
        rows = rows_html,
        yaml = req.yaml_content
    );

    fs::write(&report_path, html).map_err(|e| format!("Error al escribir reporte HTML: {}", e))?;
    let _ = open::that(&report_path);
    Ok(report_path.to_string_lossy().to_string())
}
