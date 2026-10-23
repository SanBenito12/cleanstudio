//! Informe de calidad de una ejecución: lo que está mal o se puede mejorar, más allá de
//! "pasó / falló".
//!
//! - Fallos: cada paso fallido con su explicación en español.
//! - Cierres y bloqueos: en Android, `logcat` (FATAL EXCEPTION, ANR, tombstones nativos y errores
//!   de JavaScript de React Native); en iOS, los `.ips` nuevos de DiagnosticReports.
//! - Lentitud: pasos que tardan mucho más que en ejecuciones anteriores con el mismo paso.
//! - Inestabilidad: flujos que, con el mismo YAML, unas veces pasan y otras fallan.
//! - Parpadeos: destellos y zonas que cambian de ida y vuelta en el video (parpadeos.rs).
//!
//! `analizar` es puro (registro + historial + eventos del sistema → hallazgos) para poder
//! probarlo; la lectura de logcat y de los reportes de iOS va aparte.
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

/// Algo que pasó en el sistema durante la ejecución (cierre, bloqueo, error de JS).
#[derive(Debug, Clone, PartialEq)]
pub struct EventoSistema {
    pub tipo: &'static str, // "cierre" | "bloqueo" | "error-js"
    pub ms: i64,            // momento (epoch en ms)
    pub detalle: String,
}

/// appId de un flujo, leído de su YAML (`appId: com.empresa.app`).
pub fn app_id(yaml: &str) -> Option<String> {
    yaml.lines()
        .map(str::trim)
        .take_while(|l| *l != "---")
        .find_map(|l| l.strip_prefix("appId:"))
        .map(|v| v.trim().trim_matches(|c| c == '"' || c == '\'').to_string())
        .filter(|v| !v.is_empty())
}

// ---------------------------------------------------------------------------
// Eventos del sistema
// ---------------------------------------------------------------------------

/// Busca cierres, bloqueos y errores de JavaScript de las apps probadas en un volcado de
/// `adb logcat -d -v epoch` (líneas `<epoch> <pid> <tid> <nivel> <tag>: <mensaje>`).
pub fn eventos_logcat(texto: &str, apps: &[String], desde_ms: i64) -> Vec<EventoSistema> {
    let mut out: Vec<EventoSistema> = Vec::new();
    let lines: Vec<&str> = texto.lines().collect();
    let parse = |line: &str| -> Option<(i64, char, String, String)> {
        let mut parts = line.split_whitespace();
        let epoch: f64 = parts.next()?.parse().ok()?;
        let _pid = parts.next()?;
        let _tid = parts.next()?;
        let nivel = parts.next()?.chars().next()?;
        let resto = parts.collect::<Vec<_>>().join(" ");
        let (tag, msg) = resto.split_once(": ").unwrap_or((resto.as_str(), ""));
        Some(((epoch * 1000.0) as i64, nivel, tag.trim().to_string(), msg.trim().to_string()))
    };
    let es_app = |s: &str| apps.iter().any(|a| s.contains(a.as_str()));
    for (i, line) in lines.iter().enumerate() {
        let Some((ms, nivel, tag, msg)) = parse(line) else { continue };
        if ms < desde_ms {
            continue;
        }
        let siguiente = |n: usize| lines.get(i + n).and_then(|l| parse(l)).map(|p| p.3).unwrap_or_default();
        let evento = if tag == "AndroidRuntime" && msg.starts_with("FATAL EXCEPTION") {
            // La línea siguiente dice "Process: <paquete>, PID: …" y la otra la excepción
            let proceso = siguiente(1);
            es_app(&proceso).then(|| EventoSistema {
                tipo: "cierre",
                ms,
                detalle: siguiente(2),
            })
        } else if msg.starts_with("ANR in ") && es_app(&msg) {
            let razon = (1..6).map(siguiente).find(|l| l.starts_with("Reason:")).unwrap_or_default();
            Some(EventoSistema { tipo: "bloqueo", ms, detalle: razon })
        } else if tag == "DEBUG" && msg.starts_with("pid:") && msg.contains(">>>") && es_app(&msg) {
            // Tombstone de un cierre nativo: "pid: 123, tid: 123, name: …  >>> com.app <<<"
            let senal = (1..4).map(siguiente).find(|l| l.starts_with("signal")).unwrap_or_default();
            Some(EventoSistema { tipo: "cierre", ms, detalle: senal })
        } else if tag == "ReactNativeJS" && nivel == 'E' {
            Some(EventoSistema { tipo: "error-js", ms, detalle: msg.clone() })
        } else {
            None
        };
        if let Some(e) = evento {
            // El mismo error se repite a veces en varias líneas seguidas
            if !out.iter().any(|o| o.tipo == e.tipo && o.detalle == e.detalle && (o.ms - e.ms).abs() < 2000) {
                out.push(e);
            }
        }
    }
    out
}

/// Lee logcat del emulador o teléfono Android usado en la ejecución.
pub fn leer_logcat(adb: &mut Command, serial: &str, apps: &[String], desde: SystemTime) -> Vec<EventoSistema> {
    let desde_ms = desde.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    adb.args(["-s", serial, "logcat", "-d", "-v", "epoch", "-b", "main,system,crash"]);
    let Some(out) = crate::bridge::output_with_timeout(adb, std::time::Duration::from_secs(10)) else {
        return Vec::new();
    };
    eventos_logcat(&String::from_utf8_lossy(&out), apps, desde_ms)
}

/// Cierres de apps en el simulador de iOS: reportes `.ips` nuevos cuyo `bundleID` es una app probada.
pub fn leer_cierres_ios(dir: &Path, apps: &[String], desde: SystemTime) -> Vec<EventoSistema> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ips") {
            continue;
        }
        let Ok(modificado) = entry.metadata().and_then(|m| m.modified()) else { continue };
        if modificado < desde {
            continue;
        }
        let Ok(texto) = std::fs::read_to_string(&path) else { continue };
        // La primera línea es un JSON con bundleID, app_name, timestamp…
        let Some(cabecera) = texto.lines().next().and_then(|l| serde_json::from_str::<Value>(l).ok()) else { continue };
        let bundle = cabecera["bundleID"].as_str().unwrap_or("");
        if !apps.iter().any(|a| a == bundle) {
            continue;
        }
        let ms = modificado.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        out.push(EventoSistema {
            tipo: "cierre",
            ms,
            detalle: format!("Reporte de cierre: {}", path.file_name().unwrap_or_default().to_string_lossy()),
        });
    }
    out
}

/// Hallazgos de una ejecución recién guardada en `dir`, con los eventos del dispositivo
/// (`plataforma` "android"/"ios", sin importar mayúsculas) y las 30 ejecuciones anteriores.
/// Lo usan la app (runs.rs) y el servidor MCP (mcp.rs).
pub fn informe(record: &Value, dir: &Path, plataforma: &str, id: &str, adb: Option<Command>, desde: SystemTime) -> Vec<Value> {
    let apps: Vec<String> = record["flujos"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| app_id(f["yaml"].as_str().unwrap_or("")))
        .collect();
    let eventos = match (plataforma.to_lowercase().as_str(), adb) {
        _ if apps.is_empty() || id.is_empty() => Vec::new(),
        ("android", Some(mut adb)) => leer_logcat(&mut adb, id, &apps, desde),
        ("ios", _) => leer_cierres_ios(&crate::paths::home_dir().join("Library/Logs/DiagnosticReports"), &apps, desde),
        _ => Vec::new(),
    };
    // Las carpetas empiezan con la fecha: las 30 más recientes, sin la actual
    let mut anteriores: Vec<std::path::PathBuf> = dir
        .parent()
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p != dir)
        .collect();
    anteriores.sort();
    let historial: Vec<Value> = anteriores
        .iter()
        .rev()
        .take(30)
        .filter_map(|d| std::fs::read_to_string(d.join("ejecucion.json")).ok())
        .filter_map(|t| serde_json::from_str(&t).ok())
        .collect();
    let mut hallazgos = analizar(record, &historial, &eventos);
    // Parpadeos: revisa el video de cada flujo
    for flujo in record["flujos"].as_array().into_iter().flatten() {
        let (Some(video), Some(rec)) = (flujo["video"].as_str(), flujo["grabacionInicioMs"].as_i64()) else { continue };
        let Some(cuadros) = crate::parpadeos::leer(&dir.join(video)) else { continue };
        let pasos: Vec<f64> = flujo["pasos"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["inicioMs"].as_i64())
            .map(|t| (t - rec) as f64 / 1000.0)
            .collect();
        hallazgos.extend(hallazgos_parpadeo(flujo, &crate::parpadeos::detectar(&cuadros, &pasos)));
    }
    ordenar(&mut hallazgos);
    hallazgos
}

/// Convierte lo que encontró el detector de parpadeos en avisos del informe.
pub fn hallazgos_parpadeo(flujo: &Value, parpadeos: &[crate::parpadeos::Parpadeo]) -> Vec<Value> {
    let Some(rec) = flujo["grabacionInicioMs"].as_i64() else { return Vec::new() };
    parpadeos
        .iter()
        .map(|p| {
            let ms = rec + (p.desde * 1000.0) as i64;
            let durante = paso_en(flujo, ms).map(|x| format!(" durante «{}»", x)).unwrap_or_default();
            let (titulo, detalle) = if p.tipo == "destello" {
                (
                    format!("La pantalla destelló{}", durante),
                    format!(
                        "Por un instante {} y volvió. Suele pasar cuando una pantalla se redibuja desde cero o su fondo carga tarde.",
                        p.zona
                    ),
                )
            } else {
                (
                    format!("Posible parpadeo {}{}", p.zona, durante),
                    format!(
                        "Esa zona cambió de ida y vuelta durante {} sin que la prueba tocara nada. Puede ser un parpadeo o una animación, como un indicador de carga: míralo en el video.",
                        segundos(((p.hasta - p.desde) * 1000.0) as i64)
                    ),
                )
            };
            hallazgo("media", "parpadeo", titulo, detalle, flujo, Some(ms))
        })
        .collect()
}

fn ordenar(hallazgos: &mut [Value]) {
    let peso = |h: &Value| match h["gravedad"].as_str() {
        Some("alta") => 0,
        Some("media") => 1,
        _ => 2,
    };
    hallazgos.sort_by_key(peso);
}

// ---------------------------------------------------------------------------
// Análisis
// ---------------------------------------------------------------------------

fn ventana(flujo: &Value) -> Option<(i64, i64)> {
    let pasos = flujo["pasos"].as_array()?;
    let ini = pasos.iter().filter_map(|p| p["inicioMs"].as_i64()).min()?;
    let fin = pasos
        .iter()
        .filter_map(|p| Some(p["inicioMs"].as_i64()? + p["duracionMs"].as_i64().unwrap_or(0)))
        .max()?;
    Some((ini, fin))
}

/// Paso que estaba en curso en un momento dado (el último que empezó antes).
fn paso_en(flujo: &Value, ms: i64) -> Option<String> {
    flujo["pasos"]
        .as_array()?
        .iter()
        .filter(|p| p["inicioMs"].as_i64().map(|t| t <= ms).unwrap_or(false))
        .max_by_key(|p| p["inicioMs"].as_i64())
        .and_then(|p| p["paso"].as_str())
        .map(String::from)
}

fn mediana(mut v: Vec<i64>) -> Option<i64> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    Some(v[v.len() / 2])
}

fn segundos(ms: i64) -> String {
    format!("{:.1} s", ms as f64 / 1000.0).replace('.', ",")
}

fn hallazgo(gravedad: &str, tipo: &str, titulo: String, detalle: String, flujo: &Value, ms: Option<i64>) -> Value {
    // Momento dentro del video, para que el reporte salte ahí
    let video = flujo["video"].as_str();
    let t = match (video, flujo["grabacionInicioMs"].as_i64(), ms) {
        (Some(_), Some(r), Some(m)) if m >= r => Some((m - r) as f64 / 1000.0),
        _ => None,
    };
    json!({
        "gravedad": gravedad,
        "tipo": tipo,
        "titulo": titulo,
        "detalle": detalle,
        "flujo": flujo["nombre"],
        "archivo": flujo["archivo"],
        "video": video,
        "segundo": t,
    })
}

/// Hallazgos de una ejecución, del más grave al más leve.
/// `historial` son los `ejecucion.json` anteriores del mismo workspace.
pub fn analizar(record: &Value, historial: &[Value], eventos: &[EventoSistema]) -> Vec<Value> {
    let flujos = record["flujos"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();

    // Cierres, bloqueos y errores de JS, asignados al flujo y al paso en curso
    for e in eventos {
        let flujo = flujos
            .iter()
            .find(|f| ventana(f).map(|(a, b)| e.ms >= a - 2000 && e.ms <= b + 2000).unwrap_or(false))
            .or(flujos.first())
            .cloned()
            .unwrap_or(Value::Null);
        let durante = paso_en(&flujo, e.ms).map(|p| format!(" durante «{}»", p)).unwrap_or_default();
        let (gravedad, titulo) = match e.tipo {
            "cierre" => ("alta", format!("La app se cerró sola{}", durante)),
            "bloqueo" => ("alta", format!("La app dejó de responder{}", durante)),
            _ => ("media", format!("Error de JavaScript{}", durante)),
        };
        out.push(hallazgo(gravedad, e.tipo, titulo, e.detalle.clone(), &flujo, Some(e.ms)));
    }

    for flujo in &flujos {
        let pasos = flujo["pasos"].as_array().cloned().unwrap_or_default();

        // Fallos: solo el paso que falló de verdad, no los runFlow/repeat que lo contienen
        for p in pasos
            .iter()
            .filter(|p| p["estado"] == "FAILED" && p["grupo"] != true && !p["error"].as_str().unwrap_or("").is_empty())
        {
            let explicacion = p["explicacion"].as_str().filter(|s| !s.is_empty()).unwrap_or(p["error"].as_str().unwrap_or(""));
            let sugerencia = p["sugerencia"].as_str().unwrap_or("");
            out.push(hallazgo(
                "alta",
                "fallo",
                format!("Falló «{}»", p["paso"].as_str().unwrap_or("Paso")),
                [explicacion, sugerencia].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" "),
                flujo,
                p["inicioMs"].as_i64(),
            ));
        }

        // Lentitud: comparado con el mismo paso del mismo flujo en ejecuciones anteriores
        let archivo = flujo["archivo"].as_str().unwrap_or("");
        for p in pasos.iter().filter(|p| p["estado"] == "COMPLETED" && p["grupo"] != true) {
            let (Some(dur), Some(original)) = (p["duracionMs"].as_i64(), p["pasoOriginal"].as_str()) else { continue };
            // Las esperas explícitas tardan a propósito
            if original.to_lowercase().contains("wait") {
                continue;
            }
            let previos: Vec<i64> = historial
                .iter()
                .flat_map(|r| r["flujos"].as_array().cloned().unwrap_or_default())
                .filter(|f| f["archivo"].as_str() == Some(archivo))
                .flat_map(|f| f["pasos"].as_array().cloned().unwrap_or_default())
                .filter(|q| q["estado"] == "COMPLETED" && q["pasoOriginal"].as_str() == Some(original))
                .filter_map(|q| q["duracionMs"].as_i64())
                .collect();
            let lento = match mediana(previos.clone()) {
                Some(m) if previos.len() >= 2 => dur >= 3000 && dur > (m * 2).max(m + 2000),
                _ => dur >= 15000,
            };
            if lento {
                let comparado = mediana(previos.clone())
                    .filter(|_| previos.len() >= 2)
                    .map(|m| format!(" Normalmente tarda {}.", segundos(m)))
                    .unwrap_or_default();
                out.push(hallazgo(
                    "media",
                    "lento",
                    format!("«{}» tardó {}", p["paso"].as_str().unwrap_or("Paso"), segundos(dur)),
                    format!("Revisa si la pantalla carga lento o si el elemento tarda en aparecer.{}", comparado),
                    flujo,
                    p["inicioMs"].as_i64(),
                ));
            }
        }

        // Inestabilidad: con el mismo YAML, unas veces pasa y otras falla
        let yaml = flujo["yaml"].as_str().unwrap_or("");
        if !yaml.is_empty() {
            let mismos: Vec<&str> = historial
                .iter()
                .flat_map(|r| r["flujos"].as_array().cloned().unwrap_or_default())
                .filter(|f| f["archivo"].as_str() == Some(archivo) && f["yaml"].as_str() == Some(yaml))
                .filter_map(|f| f["estado"].as_str().map(|s| if s == "COMPLETED" { "ok" } else { "mal" }))
                .collect::<Vec<_>>()
                .into_iter()
                .chain(std::iter::once(if flujo["estado"] == "COMPLETED" { "ok" } else { "mal" }))
                .collect();
            let pasaron = mismos.iter().filter(|s| **s == "ok").count();
            if pasaron > 0 && pasaron < mismos.len() {
                out.push(hallazgo(
                    "media",
                    "inestable",
                    "Prueba inestable".to_string(),
                    format!(
                        "Con el mismo YAML pasó {} de {} veces. Suele ser una espera corta o una pantalla que no siempre carga igual.",
                        pasaron,
                        mismos.len()
                    ),
                    flujo,
                    None,
                ));
            }
        }
    }

    ordenar(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paso(nombre: &str, estado: &str, inicio: i64, dur: i64) -> Value {
        json!({ "paso": nombre, "pasoOriginal": nombre, "estado": estado, "inicioMs": inicio, "duracionMs": dur })
    }

    fn registro(estado: &str, pasos: Vec<Value>) -> Value {
        json!({ "flujos": [{
            "nombre": "Login", "archivo": "/w/login.yaml", "estado": estado, "yaml": "appId: com.app\n---\n- launchApp",
            "video": "video.mp4", "grabacionInicioMs": 1000, "pasos": pasos
        }] })
    }

    #[test]
    fn lee_el_app_id_del_encabezado() {
        assert_eq!(app_id("appId: \"com.ejemplo.app\"\nname: x\n---\n- launchApp"), Some("com.ejemplo.app".into()));
        assert_eq!(app_id("name: x\n---\n- appId: no"), None);
    }

    #[test]
    fn detecta_cierres_bloqueos_y_errores_de_js_de_la_app() {
        let log = "\
1000.000  10  10 E AndroidRuntime: FATAL EXCEPTION: main
1000.001  10  10 E AndroidRuntime: Process: com.app, PID: 10
1000.002  10  10 E AndroidRuntime: java.lang.NullPointerException: boom
1001.000  20  20 E AndroidRuntime: FATAL EXCEPTION: main
1001.001  20  20 E AndroidRuntime: Process: com.otra, PID: 20
1002.000  30  30 E ActivityManager: ANR in com.app (com.app/.MainActivity)
1002.001  30  30 E ActivityManager: PID: 30
1002.002  30  30 E ActivityManager: Reason: Input dispatching timed out
1003.000  40  40 E ReactNativeJS: TypeError: undefined is not an object
1003.500  40  40 E ReactNativeJS: TypeError: undefined is not an object
0900.000  50  50 E AndroidRuntime: FATAL EXCEPTION: viejo";
        let ev = eventos_logcat(log, &["com.app".to_string()], 950_000);
        assert_eq!(ev.len(), 3, "{:?}", ev);
        assert_eq!(ev[0].tipo, "cierre");
        assert!(ev[0].detalle.contains("NullPointerException"));
        assert_eq!(ev[1].tipo, "bloqueo");
        assert!(ev[1].detalle.contains("Input dispatching"));
        assert_eq!(ev[2].tipo, "error-js");
    }

    #[test]
    fn marca_pasos_lentos_respecto_al_historial() {
        let previos: Vec<Value> = (0..3).map(|_| registro("COMPLETED", vec![paso("Tocar Entrar", "COMPLETED", 0, 1000)])).collect();
        let actual = registro("COMPLETED", vec![paso("Tocar Entrar", "COMPLETED", 5000, 9000)]);
        let h = analizar(&actual, &previos, &[]);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0]["tipo"], "lento");
        assert_eq!(h[0]["segundo"], 4.0);
        // Sin historial, 9 s no basta para marcarlo
        assert!(analizar(&actual, &[], &[]).is_empty());
    }

    #[test]
    fn marca_flujos_inestables_y_fallos() {
        let previos = vec![registro("COMPLETED", vec![paso("Abrir", "COMPLETED", 0, 100)])];
        let mut fallido = paso("Tocar Entrar", "FAILED", 2000, 100);
        fallido["error"] = json!("Element not found");
        fallido["explicacion"] = json!("No se encontró el elemento.");
        let actual = registro("FAILED", vec![fallido]);
        let h = analizar(&actual, &previos, &[]);
        let tipos: Vec<&str> = h.iter().filter_map(|x| x["tipo"].as_str()).collect();
        assert_eq!(tipos, vec!["fallo", "inestable"]);
        assert!(h[1]["detalle"].as_str().unwrap().contains("1 de 2"));
    }

    #[test]
    fn convierte_parpadeos_en_avisos_con_su_momento() {
        let actual = registro("COMPLETED", vec![paso("Abrir la app", "COMPLETED", 1000, 500), paso("Ver mapa", "COMPLETED", 2000, 4000)]);
        let p = crate::parpadeos::Parpadeo { tipo: "zona", desde: 2.5, hasta: 4.5, zona: "arriba de la pantalla".into(), veces: 3 };
        let h = hallazgos_parpadeo(&actual["flujos"][0], &[p]);
        assert_eq!(h[0]["titulo"], "Posible parpadeo arriba de la pantalla durante «Ver mapa»");
        assert_eq!(h[0]["segundo"], 2.5);
        assert!(h[0]["detalle"].as_str().unwrap().contains("2,0 s"));
    }

    #[test]
    fn ubica_el_cierre_en_el_paso_en_curso() {
        let actual = registro("FAILED", vec![paso("Abrir la app", "COMPLETED", 1000, 500), paso("Tocar Entrar", "FAILED", 2000, 800)]);
        let ev = [EventoSistema { tipo: "cierre", ms: 2300, detalle: "boom".into() }];
        let h = analizar(&actual, &[], &ev);
        assert_eq!(h[0]["titulo"], "La app se cerró sola durante «Tocar Entrar»");
    }

    /// Analiza una ejecución guardada contra las demás de su carpeta y escribe el reporte en CS_OUT.
    /// `CS_EJECUCION=<carpeta> CS_OUT=<archivo.html> cargo test --lib -- --ignored reanaliza`
    #[test]
    #[ignore]
    fn reanaliza_ejecucion() {
        let dir = std::path::PathBuf::from(std::env::var("CS_EJECUCION").expect("CS_EJECUCION"));
        let leer = |d: &Path| -> Option<Value> { serde_json::from_str(&std::fs::read_to_string(d.join("ejecucion.json")).ok()?).ok() };
        let mut record = leer(&dir).expect("ejecucion.json");
        let historial: Vec<Value> = std::fs::read_dir(dir.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| *p != dir)
            .filter_map(|p| leer(&p))
            .collect();
        record["calidad"] = json!(analizar(&record, &historial, &[]));
        println!("{}", serde_json::to_string_pretty(&record["calidad"]).unwrap());
        std::fs::write(std::env::var("CS_OUT").expect("CS_OUT"), crate::reports::render(&record)).unwrap();
    }
}
