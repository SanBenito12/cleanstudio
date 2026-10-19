//! Evidencia de cada ejecución: video, capturas, `ejecucion.json` y reporte HTML.
//!
//! El shim reenvía (`cs-run-update`) cada mensaje de los SSE de estado que ya usa la interfaz
//! (`flowStatus` y `workspaceStatus`). El primer mensaje de una ejecución nueva abre una carpeta
//! en `<workspace>/.clean-studio/ejecuciones/`; el estado final la cierra. El motor ya graba la
//! pantalla y guarda las capturas en `~/.maestro/tests/`: aquí solo se copian a la carpeta de la
//! ejecución (grabar en paralelo choca con la grabación del motor en el simulador).
use crate::{errors, paths, reports, simulator, workspace, AppState};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Manager};

#[derive(Default)]
pub struct RunsState {
    active: Mutex<Option<ActiveRun>>,
}

struct ActiveRun {
    key: String,
    kind: String,
    dir: PathBuf,
    started: SystemTime,
    device: Option<(String, String)>,
    last: Value,
}

fn is_final(status: &str) -> bool {
    status == "COMPLETED" || status == "FAILED"
}

pub(crate) fn slug(s: &str) -> String {
    let base = Path::new(s).file_stem().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| s.to_string());
    let clean: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    clean.trim_matches('-').chars().take(48).collect()
}

/// Un mensaje de estado de la interfaz (`kind` = "flow" | "workspace", `raw` = JSON del SSE).
pub fn update(app: &AppHandle, kind: &str, raw: &str) {
    let Ok(data) = serde_json::from_str::<Value>(raw) else { return };
    let (key, status) = if kind == "workspace" {
        (data["workspaceRunId"].as_str(), data["workspaceStatus"].as_str())
    } else {
        (data["flowId"].as_str(), data["flowStatus"].as_str())
    };
    let (Some(key), Some(status)) = (key.map(String::from), status.map(String::from)) else { return };
    let (key, status) = (key.as_str(), status.as_str());
    let state = app.state::<AppState>();
    let mut guard = state.runs.active.lock().unwrap();

    // Durante un workspace también llegan estados de flujos sueltos: el workspace manda
    if let Some(run) = guard.as_ref() {
        if run.kind == "workspace" && kind == "flow" {
            return;
        }
    }

    let is_same = guard.as_ref().map(|r| r.key == key).unwrap_or(false);
    if !is_same {
        if is_final(status) {
            return; // estado final de una ejecución que no vimos empezar
        }
        let finished = guard.take();
        if let Some(prev) = finished {
            let app = app.clone();
            thread::spawn(move || finish(&app, prev));
        }
        *guard = start(app, kind, key, &data);
    }
    if let Some(run) = guard.as_mut() {
        run.last = data;
    }
    if is_final(status) {
        if let Some(run) = guard.take() {
            drop(guard);
            finish(app, run);
        }
    }
}

fn start(app: &AppHandle, kind: &str, key: &str, data: &Value) -> Option<ActiveRun> {
    let filepath = data["filepath"].as_str().unwrap_or("");
    let workspace = workspace::current_root(app)
        .or_else(|| Path::new(filepath).parent().map(Path::to_path_buf))?;
    let name = if kind == "workspace" { "workspace".to_string() } else { slug(filepath) };
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dir = workspace.join(".clean-studio").join("ejecuciones").join(format!("{}-{}", stamp, name));
    std::fs::create_dir_all(&dir).ok()?;

    let device = simulator::current_device(app);
    eprintln!("[EJECUCIONES] Inicio {} → {}", key, dir.display());
    Some(ActiveRun {
        key: key.to_string(),
        kind: kind.to_string(),
        dir,
        started: SystemTime::now(),
        device,
        last: data.clone(),
    })
}

fn finish(app: &AppHandle, run: ActiveRun) {
    let keep_video = app.state::<AppState>().settings.bool_or("grabarVideo", true);
    let videos = if keep_video { copy_videos(&run.kind, &run.last, &run.dir) } else { Vec::new() };
    let capturas = collect_screenshots(&run.dir, run.started);
    let device = run.device.as_ref().map(|(p, id)| {
        json!({
            "plataforma": if p == "ios" { "iOS" } else { "Android" },
            "id": id,
            "nombre": device_name(app, p, id),
        })
    });
    let record = build_record(&run.kind, &run.last, run.started, device, &videos, &capturas);
    let _ = std::fs::write(
        run.dir.join("ejecucion.json"),
        serde_json::to_string_pretty(&record).unwrap_or_default(),
    );
    let report = run.dir.join("reporte.html");
    if std::fs::write(&report, reports::render(&record)).is_ok() {
        let state = app.state::<AppState>();
        state.settings.set("ultimoReporte", json!(report.to_string_lossy()));
        if state.settings.bool_or("abrirReporteAlTerminar", false) {
            let _ = open::that(&report);
        }
    }
    eprintln!("[EJECUCIONES] Fin {} ({} videos, {} capturas)", run.key, videos.len(), capturas.len());
}

pub(crate) fn maestro_tests_dir() -> PathBuf {
    paths::home_dir().join(".maestro").join("tests")
}

/// Copia el video que grabó el motor (`videoUrl`, relativo a `~/.maestro/tests`).
/// En un workspace hay un video por flujo.
pub(crate) fn copy_videos(kind: &str, last: &Value, dir: &Path) -> Vec<String> {
    let flows: Vec<&Value> = if kind == "workspace" {
        last["flows"].as_object().map(|m| m.values().collect()).unwrap_or_default()
    } else {
        vec![last]
    };
    let mut out = Vec::new();
    for (i, flow) in flows.iter().enumerate() {
        let Some(rel) = flow["videoUrl"].as_str().filter(|s| !s.is_empty()) else { continue };
        let src = if Path::new(rel).is_absolute() { PathBuf::from(rel) } else { maestro_tests_dir().join(rel) };
        // El motor termina de escribir el MP4 justo al cerrar el flujo
        for _ in 0..20 {
            if std::fs::metadata(&src).map(|m| m.len() > 0).unwrap_or(false) {
                break;
            }
            thread::sleep(Duration::from_millis(250));
        }
        let name = if flows.len() == 1 {
            "video.mp4".to_string()
        } else {
            format!("video-{}-{}.mp4", i + 1, slug(flow["filepath"].as_str().unwrap_or("flujo")))
        };
        if std::fs::copy(&src, dir.join(&name)).is_ok() {
            out.push(name);
        }
    }
    out
}

/// Si la app se cierra a mitad de una ejecución, se guarda lo que haya.
pub fn abort(app: &AppHandle) {
    let run = app.state::<AppState>().runs.active.lock().unwrap().take();
    if let Some(run) = run {
        finish(app, run);
    }
}

// ---------------------------------------------------------------------------
// Registro de la ejecución
// ---------------------------------------------------------------------------

fn ms(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))
}

fn flatten(commands: &Value, level: usize, out: &mut Vec<Value>) {
    let Some(list) = commands.as_array() else { return };
    for c in list {
        let original = c["description"].as_str().or(c["command"].as_str()).unwrap_or("Paso").to_string();
        let error = c["error"].as_str().unwrap_or("").to_string();
        let (explicacion, sugerencia) = if error.is_empty() {
            (String::new(), String::new())
        } else {
            errors::explicar(&error).unwrap_or_default()
        };
        let duracion = match (ms(&c["timestamp"]), ms(&c["endTimestamp"])) {
            (Some(a), Some(b)) if b >= a => Some(b - a),
            _ => None,
        };
        let nested = ["subOnStartCommands", "subCommands", "subOnCompleteCommands"]
            .iter()
            .any(|k| c[*k].as_array().map(|a| !a.is_empty()).unwrap_or(false));
        out.push(json!({
            "nivel": level,
            "paso": errors::paso(&original),
            "pasoOriginal": original,
            "estado": c["status"].as_str().unwrap_or("PENDING"),
            "duracionMs": duracion,
            "inicioMs": ms(&c["timestamp"]),
            "error": error,
            "explicacion": explicacion,
            "sugerencia": sugerencia,
            "grupo": nested,
        }));
        for key in ["subOnStartCommands", "subCommands", "subOnCompleteCommands"] {
            flatten(&c[key], level + 1, out);
        }
    }
}

fn flow_record(flow: &Value) -> Value {
    let mut pasos = Vec::new();
    for key in ["onFlowStartCommandsStatuses", "commandStatuses", "onFlowCompleteCommandsStatuses"] {
        flatten(&flow[key], 0, &mut pasos);
    }
    let archivo = flow["filepath"].as_str().unwrap_or("").to_string();
    let yaml = std::fs::read_to_string(&archivo).unwrap_or_default();
    json!({
        "nombre": flow["flowName"].as_str().filter(|s| !s.is_empty()).unwrap_or(&archivo),
        "archivo": archivo,
        "estado": flow["flowStatus"].as_str().unwrap_or(""),
        "pasos": pasos,
        "yaml": yaml,
        // Para sincronizar los pasos con el video en el reporte
        "grabacionInicioMs": ms(&flow["screenRecordingStartTimestamp"]),
    })
}

fn device_name(app: &AppHandle, platform: &str, id: &str) -> String {
    if platform == "ios" {
        if let Ok(out) = Command::new("xcrun").args(["simctl", "list", "devices", "-j"]).output() {
            if let Ok(v) = serde_json::from_slice::<Value>(&out.stdout) {
                for list in v["devices"].as_object().into_iter().flat_map(|m| m.values()) {
                    for d in list.as_array().into_iter().flatten() {
                        if d["udid"].as_str() == Some(id) {
                            return d["name"].as_str().unwrap_or(id).to_string();
                        }
                    }
                }
            }
        }
    } else if let Ok(out) = adb(app).args(["-s", id, "shell", "getprop", "ro.product.model"]).output() {
        let model = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !model.is_empty() {
            return model;
        }
    }
    id.to_string()
}

pub(crate) fn build_record(
    kind: &str,
    last: &Value,
    started: SystemTime,
    device: Option<Value>,
    videos: &[String],
    capturas: &[String],
) -> Value {
    let mut flujos: Vec<Value> = if kind == "workspace" {
        last["flows"].as_object().map(|m| m.values().map(flow_record).collect()).unwrap_or_default()
    } else {
        vec![flow_record(last)]
    };
    // Video de cada flujo (copy_videos usa el mismo orden de flujos)
    let total = flujos.len();
    for (i, flujo) in flujos.iter_mut().enumerate() {
        let video = if total == 1 {
            videos.iter().find(|v| v.as_str() == "video.mp4")
        } else {
            videos.iter().find(|v| v.starts_with(&format!("video-{}-", i + 1)))
        };
        flujo["video"] = json!(video);
    }
    let estado = if kind == "workspace" { &last["workspaceStatus"] } else { &last["flowStatus"] };
    // Inicio y fin según los tiempos del motor (primer paso → último paso); si faltan, los nuestros
    let all_flows: Vec<&Value> = if kind == "workspace" {
        last["flows"].as_object().map(|m| m.values().collect()).unwrap_or_default()
    } else {
        vec![last]
    };
    let stamps = |field: &str| -> Vec<i64> {
        all_flows
            .iter()
            .flat_map(|f| f["commandStatuses"].as_array().cloned().unwrap_or_default())
            .filter_map(|c| ms(&c[field]))
            .collect()
    };
    let to_local = |ms: i64| chrono::DateTime::from_timestamp_millis(ms).map(|d| d.with_timezone(&chrono::Local));
    let inicio = stamps("timestamp").into_iter().min().and_then(to_local).unwrap_or_else(|| started.into());
    let fin = stamps("endTimestamp").into_iter().max().and_then(to_local).unwrap_or_else(chrono::Local::now);
    json!({
        "tipo": if kind == "workspace" { "workspace" } else { "flujo" },
        "estado": estado.as_str().unwrap_or(""),
        "inicio": inicio.to_rfc3339(),
        "fin": fin.to_rfc3339(),
        "duracionMs": (fin - inicio).num_milliseconds(),
        "dispositivo": device,
        "flujos": flujos,
        "videos": videos,
        "capturas": capturas,
        "motor": last,
    })
}

/// Copia las capturas (takeScreenshot) que el motor guardó durante la ejecución en
/// `~/.maestro/tests/<sesión>/artifacts/takeScreenshot/`.
pub(crate) fn collect_screenshots(dir: &Path, since: SystemTime) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(sessions) = std::fs::read_dir(maestro_tests_dir()) else { return found };
    for session in sessions.flatten() {
        let shots = session.path().join("artifacts").join("takeScreenshot");
        let Ok(rd) = std::fs::read_dir(&shots) else { continue };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let recent = entry.metadata().and_then(|m| m.modified()).map(|m| m >= since).unwrap_or(false);
            // Si dos sesiones tienen una captura con el mismo nombre, queda una sola (la más reciente)
            if recent
                && name.to_lowercase().ends_with(".png")
                && !found.contains(&name)
                && std::fs::copy(entry.path(), dir.join(&name)).is_ok()
            {
                found.push(name);
            }
        }
    }
    found.sort();
    found
}

fn adb(app: &AppHandle) -> Command {
    let sdk_adb = app
        .state::<AppState>()
        .settings
        .android_sdk_path()
        .map(|sdk| PathBuf::from(sdk).join("platform-tools").join("adb"))
        .filter(|p| p.exists());
    Command::new(sdk_adb.unwrap_or_else(|| PathBuf::from("adb")))
}

/// Carpeta de ejecuciones del workspace abierto.
pub fn runs_dir(app: &AppHandle) -> Option<PathBuf> {
    workspace::current_root(app).map(|w| w.join(".clean-studio").join("ejecuciones"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Genera la evidencia completa a partir de un mensaje SSE real guardado en disco.
    /// `CS_SSE_SAMPLE=<archivo con líneas data:> CS_OUT=<carpeta> cargo test --lib -- --ignored`
    #[test]
    #[ignore]
    fn evidencia_desde_sse_real() {
        let sample = std::env::var("CS_SSE_SAMPLE").expect("CS_SSE_SAMPLE");
        let out = PathBuf::from(std::env::var("CS_OUT").expect("CS_OUT"));
        let text = std::fs::read_to_string(sample).unwrap();
        let last = text.lines().filter_map(|l| l.strip_prefix("data:")).last().unwrap().trim();
        let last: Value = serde_json::from_str(last).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        let started = SystemTime::now() - Duration::from_secs(86_400);
        let videos = copy_videos("flow", &last, &out);
        let capturas = collect_screenshots(&out, started);
        let device = Some(json!({ "plataforma": "iOS", "id": "C7D925A8", "nombre": "iPhone 17" }));
        let record = build_record("flow", &last, started, device, &videos, &capturas);
        std::fs::write(out.join("ejecucion.json"), serde_json::to_string_pretty(&record).unwrap()).unwrap();
        std::fs::write(out.join("reporte.html"), reports::render(&record)).unwrap();
        assert_eq!(videos, vec!["video.mp4"]);
        assert!(!capturas.is_empty());
        assert_eq!(record["estado"], "COMPLETED");
        assert_eq!(record["flujos"][0]["pasos"][1]["paso"], "Verificar que \"General\" está visible");
    }
}
