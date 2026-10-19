//! Reporte HTML en español de una ejecución (a partir de `ejecucion.json`).
//! Es un solo archivo junto al video y las capturas: la carpeta se puede comprimir y compartir.
use serde_json::Value;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn duracion(ms: Option<i64>) -> String {
    let Some(ms) = ms else { return String::new() };
    if ms < 1000 {
        return format!("{} ms", ms);
    }
    let s = ms as f64 / 1000.0;
    if s < 60.0 {
        return format!("{:.1} s", s).replace('.', ",");
    }
    let total = s.round() as i64;
    format!("{} min {:02} s", total / 60, total % 60)
}

fn fecha(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|d| d.format("%d/%m/%Y %H:%M:%S").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

fn estado_general(estado: &str) -> (&'static str, &'static str) {
    match estado {
        "COMPLETED" => ("Aprobada", "ok"),
        "FAILED" => ("Falló", "fail"),
        _ => ("Incompleta", "warn"),
    }
}

fn estado_paso(estado: &str) -> (&'static str, &'static str) {
    match estado {
        "COMPLETED" => ("OK", "ok"),
        "FAILED" => ("Falló", "fail"),
        "WARNED" => ("Aviso", "warn"),
        "SKIPPED" => ("Omitido", "muted"),
        "RUNNING" => ("En curso", "warn"),
        _ => ("No se ejecutó", "muted"),
    }
}

const LOGO: &str = r#"<svg viewBox="0 25 92 92" aria-hidden="true"><path fill="currentColor" d="M11.04 29.6H80.96A11.04 11.04 0 0 1 80.96 51.68H11.04A11.04 11.04 0 0 1 11.04 29.6ZM11.04 59.96H57.04A11.04 11.04 0 0 1 57.04 82.04H11.04A11.04 11.04 0 0 1 11.04 59.96ZM11.04 90.32H33.12A11.04 11.04 0 0 1 33.12 112.4H11.04A11.04 11.04 0 0 1 11.04 90.32ZM69.92 101.36a11.04 11.04 0 1 0 22.08 0a11.04 11.04 0 1 0 -22.08 0Z"/></svg>"#;

const CSS: &str = r#"
:root{--bg:#f4f4f2;--card:#fff;--fg:#151515;--muted:#64645f;--line:#deded9;--ok:#1d7a46;--ok-bg:#e3f3e9;--fail:#b42318;--fail-bg:#fde8e6;--warn:#8a5a00;--warn-bg:#fdf1d8;--code:#f0f0ec;color-scheme:light}
@media (prefers-color-scheme:dark){:root{--bg:#111;--card:#1a1a1a;--fg:#ededea;--muted:#a3a39e;--line:#2e2e2c;--ok:#5cc98a;--ok-bg:#16301f;--fail:#ff8a7a;--fail-bg:#3a1712;--warn:#f0c060;--warn-bg:#352a10;--code:#232322;color-scheme:dark}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.55 -apple-system,BlinkMacSystemFont,"SF Pro Text","Helvetica Neue",sans-serif}
main{max-width:1000px;margin:0 auto;padding:32px max(16px,env(safe-area-inset-left)) 56px;display:grid;gap:22px}
header{display:flex;align-items:center;gap:14px;flex-wrap:wrap}header svg{width:34px;height:34px;flex:none}
h1{font-size:26px;line-height:1.15;margin:0;letter-spacing:-.01em;text-wrap:balance}h2{font-size:17px;margin:0 0 12px}
.sub{color:var(--muted);font-size:13px;margin-top:2px}
.pill{display:inline-flex;align-items:center;gap:6px;font-weight:600;font-size:13px;padding:3px 10px;border-radius:999px}
.pill::before{content:"";width:8px;height:8px;border-radius:50%;background:currentColor}
.ok{color:var(--ok);background:var(--ok-bg)}.fail{color:var(--fail);background:var(--fail-bg)}.warn{color:var(--warn);background:var(--warn-bg)}.muted{color:var(--muted);background:var(--code)}
.card{background:var(--card);border:1px solid var(--line);border-radius:14px;padding:18px 20px;min-width:0}
.meta{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:14px}
.meta dt{color:var(--muted);font-size:12px;text-transform:uppercase;letter-spacing:.06em}.meta dd{margin:2px 0 0;font-weight:500;font-variant-numeric:tabular-nums;overflow-wrap:anywhere}
video{width:100%;max-height:560px;border-radius:10px;background:#000}
.videos{display:grid;gap:12px}
table{width:100%;border-collapse:collapse;font-size:14px}th{text-align:left;color:var(--muted);font-weight:500;font-size:12px;text-transform:uppercase;letter-spacing:.06em;padding:0 8px 8px}
td{border-top:1px solid var(--line);padding:9px 8px;vertical-align:top}td.n{color:var(--muted);font-variant-numeric:tabular-nums;width:2.5em}td.t{white-space:nowrap;text-align:right;font-variant-numeric:tabular-nums;color:var(--muted)}
td .pill{font-size:12px;padding:1px 8px}
.grupo{font-weight:600}.orig{color:var(--muted);font-size:12px}
.err{margin-top:8px;border-left:3px solid var(--fail);padding:8px 12px;background:var(--fail-bg);border-radius:0 8px 8px 0}
.err p{margin:0}.err .tip{margin-top:4px}.err code{display:block;margin-top:6px;font-size:12px;color:var(--muted);white-space:pre-wrap;overflow-wrap:anywhere}
.shots{display:grid;grid-template-columns:repeat(auto-fill,minmax(160px,1fr));gap:12px}
.shots figure{margin:0}.shots img{width:100%;border-radius:8px;border:1px solid var(--line)}.shots figcaption{font-size:12px;color:var(--muted);margin-top:4px;overflow-wrap:anywhere}
details summary{cursor:pointer;color:var(--muted)}pre{background:var(--code);padding:14px;border-radius:10px;overflow-x:auto;font-size:13px}
.tabla{overflow-x:auto}footer{color:var(--muted);font-size:12px;text-align:center}
"#;

fn flujo_html(flujo: &Value, out: &mut String) {
    let (texto, clase) = estado_general(flujo["estado"].as_str().unwrap_or(""));
    out.push_str(&format!(
        "<section class=\"card\"><h2>{} <span class=\"pill {}\">{}</span></h2><div class=\"sub\">{}</div><div class=\"tabla\"><table><thead><tr><th>#</th><th>Paso</th><th>Estado</th><th style=\"text-align:right\">Tiempo</th></tr></thead><tbody>",
        esc(flujo["nombre"].as_str().unwrap_or("Flujo")),
        clase,
        texto,
        esc(flujo["archivo"].as_str().unwrap_or("")),
    ));
    let mut n = 0;
    for paso in flujo["pasos"].as_array().into_iter().flatten() {
        let nivel = paso["nivel"].as_u64().unwrap_or(0) as usize;
        let grupo = paso["grupo"].as_bool().unwrap_or(false);
        let (et, ec) = estado_paso(paso["estado"].as_str().unwrap_or(""));
        n += 1;
        let nombre = paso["paso"].as_str().unwrap_or("");
        let original = paso["pasoOriginal"].as_str().unwrap_or("");
        let mut celda = format!(
            "<div class=\"{}\" style=\"padding-left:{}em\">{}</div>",
            if grupo { "grupo" } else { "" },
            nivel as f32 * 1.2,
            esc(nombre)
        );
        if original != nombre {
            celda.push_str(&format!("<div class=\"orig\" style=\"padding-left:{}em\">{}</div>", nivel as f32 * 1.2, esc(original)));
        }
        let error = paso["error"].as_str().unwrap_or("");
        if !error.is_empty() {
            let explicacion = paso["explicacion"].as_str().unwrap_or("");
            let sugerencia = paso["sugerencia"].as_str().unwrap_or("");
            celda.push_str("<div class=\"err\">");
            if !explicacion.is_empty() {
                celda.push_str(&format!("<p><strong>{}</strong></p>", esc(explicacion)));
            }
            if !sugerencia.is_empty() {
                celda.push_str(&format!("<p class=\"tip\">{}</p>", esc(sugerencia)));
            }
            celda.push_str(&format!("<code>{}</code></div>", esc(error)));
        }
        out.push_str(&format!(
            "<tr><td class=\"n\">{}</td><td>{}</td><td><span class=\"pill {}\">{}</span></td><td class=\"t\">{}</td></tr>",
            n,
            celda,
            ec,
            et,
            duracion(paso["duracionMs"].as_i64())
        ));
    }
    out.push_str("</tbody></table></div>");
    if let Some(yaml) = flujo["yaml"].as_str().filter(|y| !y.is_empty()) {
        out.push_str(&format!("<details><summary>Ver el YAML ejecutado</summary><pre>{}</pre></details>", esc(yaml)));
    }
    out.push_str("</section>");
}

pub fn render(record: &Value) -> String {
    let (estado, clase) = estado_general(record["estado"].as_str().unwrap_or(""));
    let flujos = record["flujos"].as_array().cloned().unwrap_or_default();
    let titulo = if record["tipo"] == "workspace" {
        format!("Workspace · {} flujos", flujos.len())
    } else {
        flujos.first().and_then(|f| f["nombre"].as_str()).unwrap_or("Ejecución").to_string()
    };
    let dispositivo = record["dispositivo"]
        .as_object()
        .map(|d| {
            format!(
                "{} ({})",
                d.get("nombre").and_then(Value::as_str).unwrap_or(""),
                d.get("plataforma").and_then(Value::as_str).unwrap_or("")
            )
        })
        .unwrap_or_else(|| "Sin espejo activo".to_string());
    let total_pasos: usize = flujos.iter().map(|f| f["pasos"].as_array().map(|p| p.len()).unwrap_or(0)).sum();
    let fallidos: usize = flujos
        .iter()
        .flat_map(|f| f["pasos"].as_array().cloned().unwrap_or_default())
        .filter(|p| p["estado"] == "FAILED")
        .count();

    let mut html = String::new();
    html.push_str(&format!(
        "<!doctype html><html lang=\"es\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1,viewport-fit=cover\"><title>Reporte · {}</title><style>{}</style></head><body><main>",
        esc(&titulo),
        CSS
    ));
    html.push_str(&format!(
        "<header>{}<div><h1>{}</h1><div class=\"sub\">Reporte de ejecución de Clean Studio</div></div><span class=\"pill {}\" style=\"margin-left:auto\">{}</span></header>",
        LOGO,
        esc(&titulo),
        clase,
        estado
    ));
    html.push_str(&format!(
        "<section class=\"card\"><dl class=\"meta\"><div><dt>Inicio</dt><dd>{}</dd></div><div><dt>Duración</dt><dd>{}</dd></div><div><dt>Dispositivo</dt><dd>{}</dd></div><div><dt>Pasos</dt><dd>{} en total · {} con error</dd></div></dl></section>",
        fecha(record["inicio"].as_str().unwrap_or("")),
        duracion(record["duracionMs"].as_i64()),
        esc(&dispositivo),
        total_pasos,
        fallidos
    ));

    let videos: Vec<&str> = record["videos"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !videos.is_empty() {
        html.push_str("<section class=\"card\"><h2>Video de la ejecución</h2><div class=\"videos\">");
        for v in videos {
            html.push_str(&format!("<video controls preload=\"metadata\" src=\"{}\"></video>", esc(v)));
        }
        html.push_str("</div></section>");
    }

    for flujo in &flujos {
        flujo_html(flujo, &mut html);
    }

    let capturas: Vec<&str> = record["capturas"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !capturas.is_empty() {
        html.push_str("<section class=\"card\"><h2>Capturas</h2><div class=\"shots\">");
        for c in capturas {
            html.push_str(&format!(
                "<figure><a href=\"{0}\"><img src=\"{0}\" alt=\"{0}\" loading=\"lazy\"></a><figcaption>{0}</figcaption></figure>",
                esc(c)
            ));
        }
        html.push_str("</div></section>");
    }

    html.push_str("<footer>Generado por Clean Studio · todo se ejecutó en tu Mac</footer></main></body></html>");
    html
}
