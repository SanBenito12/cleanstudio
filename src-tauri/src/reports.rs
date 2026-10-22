//! Reporte HTML en español de una ejecución (a partir de `ejecucion.json`).
//!
//! Se ve como la propia pantalla de Clean Studio congelada: el dispositivo con el video a la
//! izquierda y el panel de ejecuciones a la derecha. Al tocar un paso el video salta a ese
//! momento y, mientras se reproduce, se marca el paso en curso. Es un solo archivo junto al
//! video y las capturas: la carpeta se puede comprimir y compartir.
use serde_json::Value;

/// DM Sans, la tipografía de la interfaz (licencia OFL), incrustada para verse igual sin red.
const DM_SANS: &[u8] = include_bytes!("dm-sans.woff2");

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub(crate) fn duracion(ms: Option<i64>) -> String {
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
        .map(|d| d.format("%d/%m/%Y, %H:%M").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

fn estado_general(estado: &str) -> (&'static str, &'static str) {
    match estado {
        "COMPLETED" => ("Aprobada", "ok"),
        "FAILED" => ("Falló", "fail"),
        _ => ("Incompleta", "warn"),
    }
}

/// Iconos de estado con la misma forma que el panel de ejecuciones de la app.
fn icono(estado: &str) -> (&'static str, &'static str) {
    match estado {
        "COMPLETED" => (
            "ok",
            r#"<svg viewBox="0 0 16 16" aria-label="Correcto"><circle cx="8" cy="8" r="7"/><path d="M5 8.2l2 2 4-4.4" fill="none"/></svg>"#,
        ),
        "FAILED" => (
            "fail",
            r#"<svg viewBox="0 0 16 16" aria-label="Falló"><circle cx="8" cy="8" r="7"/><path d="M5.6 5.6l4.8 4.8M10.4 5.6l-4.8 4.8" fill="none"/></svg>"#,
        ),
        "WARNED" => (
            "warn",
            r#"<svg viewBox="0 0 16 16" aria-label="Aviso"><circle cx="8" cy="8" r="7"/><path d="M8 4.6v4.2M8 11.2v.2" fill="none"/></svg>"#,
        ),
        "SKIPPED" => (
            "muted",
            r#"<svg viewBox="0 0 16 16" aria-label="Omitido"><circle cx="8" cy="8" r="7"/><path d="M5.2 8h5.6" fill="none"/></svg>"#,
        ),
        _ => (
            "pending",
            r#"<svg viewBox="0 0 16 16" aria-label="No se ejecutó"><circle cx="8" cy="8" r="7" stroke-dasharray="2.2 2.2"/></svg>"#,
        ),
    }
}

const LOGO: &str = r#"<svg class="logo" viewBox="0 25 92 92" aria-hidden="true"><path fill="currentColor" d="M11.04 29.6H80.96A11.04 11.04 0 0 1 80.96 51.68H11.04A11.04 11.04 0 0 1 11.04 29.6ZM11.04 59.96H57.04A11.04 11.04 0 0 1 57.04 82.04H11.04A11.04 11.04 0 0 1 11.04 59.96ZM11.04 90.32H33.12A11.04 11.04 0 0 1 33.12 112.4H11.04A11.04 11.04 0 0 1 11.04 90.32ZM69.92 101.36a11.04 11.04 0 1 0 22.08 0a11.04 11.04 0 1 0 -22.08 0Z"/></svg>"#;

const CSS: &str = r#"
/* Tokens de la interfaz de Clean Studio (zinc + esmeralda, radio .625rem) */
:root{
  --bg:oklch(100% 0 0);--panel:oklch(98.5% 0 0);--card:oklch(100% 0 0);--fg:oklch(14.1% .005 285.823);
  --muted:oklch(96.7% .001 286.375);--muted-fg:oklch(55.2% .016 285.938);--border:oklch(92% .004 286.32);
  --ok:oklch(59.6% .145 163.225);--ok-soft:oklch(59.6% .145 163.225/.1);
  --fail:oklch(57.7% .245 27.325);--fail-soft:oklch(57.7% .245 27.325/.08);
  --warn:oklch(66.6% .179 58.318);--warn-soft:oklch(66.6% .179 58.318/.1);
  --bezel:oklch(21% .006 285.885);--code:oklch(96.7% .001 286.375);--radius:.625rem;color-scheme:light
}
@media (prefers-color-scheme:dark){:root{
  --bg:oklch(14.1% .005 285.823);--panel:oklch(17.5% .005 285.823);--card:oklch(21% .006 285.885);--fg:oklch(98.5% 0 0);
  --muted:oklch(27.4% .006 286.033);--muted-fg:oklch(70.5% .015 286.067);--border:oklch(100% 0 0/.1);
  --ok:oklch(76.5% .177 163.223);--ok-soft:oklch(76.5% .177 163.223/.12);
  --fail:oklch(70.4% .191 22.216);--fail-soft:oklch(70.4% .191 22.216/.12);
  --warn:oklch(82.8% .189 84.429);--warn-soft:oklch(82.8% .189 84.429/.12);
  --bezel:oklch(8% 0 0);--code:oklch(21% .006 285.885);color-scheme:dark
}}
@font-face{font-family:"DM Sans";font-weight:100 1000;font-display:swap;src:url(data:font/woff2;base64,__FONT__) format("woff2")}
*{box-sizing:border-box}
html,body{margin:0;background:var(--bg);color:var(--fg)}
body{font:14px/1.5 "DM Sans",-apple-system,BlinkMacSystemFont,"Helvetica Neue",sans-serif;font-feature-settings:"ss01"}
.top{position:sticky;top:0;z-index:2;height:48px;display:grid;grid-template-columns:1fr auto 1fr;align-items:center;gap:12px;
  padding:0 max(16px,env(safe-area-inset-left));background:var(--bg);border-bottom:1px solid var(--border)}
.brand{display:flex;align-items:center;gap:8px;font-weight:600}
.logo{width:18px;height:18px}
.top h1{margin:0;font-size:14px;font-weight:500;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:52vw}
.badge{justify-self:end;display:inline-flex;align-items:center;gap:6px;font-weight:600;font-size:13px;padding:3px 10px;border-radius:999px}
.badge.ok{color:var(--ok);background:var(--ok-soft)}.badge.fail{color:var(--fail);background:var(--fail-soft)}.badge.warn{color:var(--warn);background:var(--warn-soft)}
.badge svg{width:14px;height:14px}
.badge svg circle,.badge svg path{stroke:currentColor;stroke-width:1.6;fill:none;stroke-linecap:round;stroke-linejoin:round}
.layout{display:grid;grid-template-columns:minmax(280px,360px) minmax(0,1fr);min-height:calc(100vh - 48px)}
.device{background:var(--panel);border-right:1px solid var(--border);padding:16px}
.device-inner{position:sticky;top:64px;display:grid;gap:16px}
.select{display:flex;align-items:center;gap:8px;height:36px;padding:0 12px;border:1px solid var(--border);border-radius:var(--radius);
  background:var(--card);font-weight:500;overflow:hidden;white-space:nowrap;text-overflow:ellipsis}
.select .dot{width:16px;height:16px;flex:none;color:var(--ok)}
.select .dot circle,.select .dot path{stroke:currentColor;stroke-width:1.6;fill:none;stroke-linecap:round}
.phone{position:relative;width:100%;max-width:300px;justify-self:center;aspect-ratio:402/874;border-radius:44px;
  border:10px solid var(--bezel);background:#000;overflow:hidden;box-shadow:0 0 0 1px var(--border)}
.phone video,.phone img{width:100%;height:100%;object-fit:cover;display:block}
.phone .empty{position:absolute;inset:0;display:grid;place-items:center;padding:24px;text-align:center;color:oklch(70.5% .015 286.067);font-size:13px}
.facts{display:grid;grid-template-columns:auto 1fr;gap:6px 14px;margin:0;font-size:13px}
.facts dt{color:var(--muted-fg)}.facts dd{margin:0;font-variant-numeric:tabular-nums;overflow-wrap:anywhere}
.runs{padding:16px clamp(16px,3vw,32px) 40px;display:grid;gap:24px;align-content:start;min-width:0}
.flow{border:1px solid var(--border);border-radius:var(--radius);background:var(--card);overflow:hidden}
.flow-head{display:flex;align-items:center;gap:10px;padding:12px 14px;border-bottom:1px solid var(--border)}
.flow-head h2{margin:0;font-size:14px;font-weight:600;flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.flow-head .when{color:var(--muted-fg);font-size:13px;white-space:nowrap}
.flow-head .total{font-variant-numeric:tabular-nums;color:var(--muted-fg);font-size:13px}
.steps{list-style:none;margin:0;padding:4px 0}
.step{display:grid;grid-template-columns:20px minmax(0,1fr) auto;gap:10px;align-items:start;width:100%;padding:8px 14px;
  border:0;background:none;color:inherit;font:inherit;text-align:left}
button.step{cursor:pointer}button.step:hover{background:var(--muted)}
button.step:focus-visible{outline:2px solid var(--fg);outline-offset:-2px}
.step.now{background:var(--muted)}
.step .ico{width:16px;height:16px;margin-top:2px}
.step .ico circle,.step .ico path{stroke-width:1.5;fill:none;stroke-linecap:round;stroke-linejoin:round}
.ok .ico circle,.ok .ico path{stroke:var(--ok)}.fail .ico circle,.fail .ico path{stroke:var(--fail)}
.warn .ico circle,.warn .ico path{stroke:var(--warn)}.muted .ico circle,.muted .ico path,.pending .ico circle{stroke:var(--muted-fg)}
.pending .label{color:var(--muted-fg)}
.step .orig{display:block;color:var(--muted-fg);font-size:12px;margin-top:1px}
.step .time{color:var(--muted-fg);font-size:13px;font-variant-numeric:tabular-nums;white-space:nowrap}
.step.group .label{font-weight:600}
.err{grid-column:2/-1;margin-top:6px;padding:10px 12px;border-radius:calc(var(--radius) - 2px);background:var(--fail-soft);display:grid;gap:4px}
.err strong{color:var(--fail);font-weight:600}
.err code{font:12px/1.45 ui-monospace,"SF Mono",Menlo,monospace;color:var(--muted-fg);white-space:pre-wrap;overflow-wrap:anywhere}
.section h3{margin:0 0 10px;font-size:14px;font-weight:600}
.review{border:1px solid var(--border);border-radius:var(--radius);background:var(--card);overflow:hidden}
.review-head{display:flex;align-items:baseline;gap:10px;padding:12px 14px;border-bottom:1px solid var(--border)}
.review-head h2{margin:0;font-size:14px;font-weight:600;flex:1}
.review-head span{color:var(--muted-fg);font-size:13px}
.review ol{list-style:none;margin:0;padding:0}
.review li{display:grid;grid-template-columns:10px minmax(0,1fr) auto;gap:4px 12px;align-items:baseline;padding:10px 14px}
.review li+li{border-top:1px solid var(--border)}
.review .sev{width:8px;height:8px;border-radius:50%;align-self:center}
.sev.alta{background:var(--fail)}.sev.media{background:var(--warn)}.sev.baja{background:var(--muted-fg)}
.review .what{font-weight:600}
.review .why{grid-column:2/-1;color:var(--muted-fg);font-size:13px;overflow-wrap:anywhere}
.review .where{grid-column:2/-1;color:var(--muted-fg);font-size:12px}
.review .seek{font:inherit;font-size:13px;font-weight:500;color:var(--fg);background:var(--muted);border:0;border-radius:999px;
  padding:2px 10px;cursor:pointer;white-space:nowrap}
.review .seek:hover{background:var(--border)}.review .seek:focus-visible{outline:2px solid var(--fg);outline-offset:2px}
.review .clean{padding:12px 14px;margin:0;color:var(--muted-fg);font-size:13px}
.shots{display:grid;grid-template-columns:repeat(auto-fill,minmax(140px,1fr));gap:12px}
.shots figure{margin:0}.shots img{width:100%;border-radius:var(--radius);border:1px solid var(--border);display:block}
.shots figcaption{font-size:12px;color:var(--muted-fg);margin-top:4px;overflow-wrap:anywhere}
details{border:1px solid var(--border);border-radius:var(--radius);background:var(--card)}
summary{cursor:pointer;padding:10px 14px;font-weight:500}
.yaml{margin:0;padding:10px 0;background:var(--code);border-top:1px solid var(--border);overflow-x:auto;
  font:12.5px/1.6 ui-monospace,"SF Mono",Menlo,monospace;counter-reset:l}
.yaml span{display:block;padding-right:16px;white-space:pre}
.yaml span::before{counter-increment:l;content:counter(l);display:inline-block;width:3.2em;padding-right:1.2em;text-align:right;color:var(--muted-fg);opacity:.7}
.hint{color:var(--muted-fg);font-size:12px;margin:0}
footer{color:var(--muted-fg);font-size:12px}
@media (max-width:860px){
  .layout{grid-template-columns:minmax(0,1fr)}
  .device{border-right:0;border-bottom:1px solid var(--border)}
  .device-inner{position:static}.phone{max-width:240px}
  .top{grid-template-columns:auto 1fr auto}.top h1{max-width:none}
}
@media (prefers-reduced-motion:reduce){*{scroll-behavior:auto!important}}
"#;

const JS: &str = r#"
(function(){
  var player=document.getElementById("player");
  if(!player)return;
  var steps=[].slice.call(document.querySelectorAll("button.step[data-t]"));
  // Mostrar el primer cuadro en vez de un recuadro vacío antes de reproducir
  player.addEventListener("loadedmetadata",function(){if(player.paused&&player.currentTime===0)player.currentTime=0.05;},{once:true});
  function load(src){if(src&&player.getAttribute("src")!==src){player.setAttribute("src",src);player.load();}}
  // Pasos y hallazgos del informe de calidad saltan al mismo punto del video
  [].slice.call(document.querySelectorAll("[data-t][data-video]")).forEach(function(b){
    b.addEventListener("click",function(){
      load(b.dataset.video);
      var t=parseFloat(b.dataset.t);
      var go=function(){player.currentTime=Math.max(0,t-0.3);player.play().catch(function(){});};
      if(player.readyState>=1)go();else player.addEventListener("loadedmetadata",go,{once:true});
    });
  });
  player.addEventListener("timeupdate",function(){
    var src=player.getAttribute("src"),t=player.currentTime,cur=null;
    steps.forEach(function(b){if(b.dataset.video===src&&parseFloat(b.dataset.t)<=t+0.05)cur=b;});
    steps.forEach(function(b){b.classList.toggle("now",b===cur);});
  });
})();
"#;

fn flujo_html(flujo: &Value, out: &mut String) {
    let (texto, clase) = estado_general(flujo["estado"].as_str().unwrap_or(""));
    let video = flujo["video"].as_str();
    let rec_start = flujo["grabacionInicioMs"].as_i64();
    let pasos = flujo["pasos"].as_array().cloned().unwrap_or_default();
    let total: i64 = pasos.iter().filter(|p| p["nivel"].as_u64() == Some(0)).filter_map(|p| p["duracionMs"].as_i64()).sum();
    out.push_str(&format!(
        "<section class=\"flow\"><div class=\"flow-head\"><h2 title=\"{arch}\">{nombre}</h2><span class=\"total\">{total}</span><span class=\"badge {clase}\">{texto}</span></div><ol class=\"steps\">",
        arch = esc(flujo["archivo"].as_str().unwrap_or("")),
        nombre = esc(flujo["nombre"].as_str().unwrap_or("Flujo")),
        total = duracion(Some(total)),
        clase = clase,
        texto = texto,
    ));
    for paso in &pasos {
        let nivel = paso["nivel"].as_u64().unwrap_or(0) as f32;
        let (sclass, svg) = icono(paso["estado"].as_str().unwrap_or(""));
        let nombre = paso["paso"].as_str().unwrap_or("");
        let original = paso["pasoOriginal"].as_str().unwrap_or("");
        let grupo = if paso["grupo"].as_bool().unwrap_or(false) { " group" } else { "" };
        // Momento del paso dentro del video, si hay grabación
        let offset = match (video, rec_start, paso["inicioMs"].as_i64()) {
            (Some(_), Some(r), Some(t)) if t >= r => Some((t - r) as f64 / 1000.0),
            _ => None,
        };
        let (tag, attrs) = match offset {
            Some(s) => (
                "button",
                format!(" type=\"button\" data-t=\"{:.2}\" data-video=\"{}\" title=\"Ver este paso en el video\"", s, esc(video.unwrap_or(""))),
            ),
            None => ("div", String::new()),
        };
        let mut label = format!("<span class=\"label\">{}", esc(nombre));
        if original != nombre {
            label.push_str(&format!("<span class=\"orig\">{}</span>", esc(original)));
        }
        label.push_str("</span>");
        let mut err = String::new();
        let error = paso["error"].as_str().unwrap_or("");
        if !error.is_empty() {
            err.push_str("<div class=\"err\">");
            let explicacion = paso["explicacion"].as_str().unwrap_or("");
            let sugerencia = paso["sugerencia"].as_str().unwrap_or("");
            if !explicacion.is_empty() {
                err.push_str(&format!("<strong>{}</strong>", esc(explicacion)));
            }
            if !sugerencia.is_empty() {
                err.push_str(&format!("<span>{}</span>", esc(sugerencia)));
            }
            err.push_str(&format!("<code>{}</code></div>", esc(error)));
        }
        out.push_str(&format!(
            "<li><{tag} class=\"step {sclass}{grupo}\"{attrs} style=\"padding-left:calc(14px + {ind}em)\">{svg}{label}<span class=\"time\">{time}</span>{err}</{tag}></li>",
            tag = tag,
            sclass = sclass,
            grupo = grupo,
            attrs = attrs,
            ind = nivel * 1.25,
            svg = svg.replace("<svg ", "<svg class=\"ico\" "),
            label = label,
            time = duracion(paso["duracionMs"].as_i64()),
            err = err,
        ));
    }
    out.push_str("</ol></section>");
    if let Some(yaml) = flujo["yaml"].as_str().filter(|y| !y.is_empty()) {
        out.push_str("<details><summary>Flujo YAML ejecutado</summary><pre class=\"yaml\">");
        for line in yaml.lines() {
            out.push_str(&format!("<span>{}</span>", esc(line)));
        }
        out.push_str("</pre></details>");
    }
}

/// "Qué revisar": hallazgos de calidad.rs (fallos, cierres, lentitud, inestabilidad).
/// Los registros anteriores a esta función no traen `calidad` y no muestran la sección.
fn calidad_html(record: &Value, out: &mut String) {
    let Some(hallazgos) = record["calidad"].as_array() else { return };
    let multiples = record["flujos"].as_array().map(|f| f.len() > 1).unwrap_or(false);
    let resumen = match hallazgos.len() {
        0 => String::new(),
        1 => "1 cosa".to_string(),
        n => format!("{} cosas", n),
    };
    out.push_str(&format!(
        "<section class=\"review\"><div class=\"review-head\"><h2>Qué revisar</h2><span>{}</span></div>",
        resumen
    ));
    if hallazgos.is_empty() {
        out.push_str("<p class=\"clean\">Nada que revisar: sin fallos, sin cierres ni bloqueos de la app, sin pasos más lentos de lo normal y sin resultados inestables.</p></section>");
        return;
    }
    out.push_str("<ol>");
    for h in hallazgos {
        let gravedad = h["gravedad"].as_str().unwrap_or("baja");
        let etiqueta = match gravedad {
            "alta" => "Grave",
            "media" => "Importante",
            _ => "Menor",
        };
        let seek = match (h["segundo"].as_f64(), h["video"].as_str()) {
            (Some(t), Some(v)) => format!(
                "<button type=\"button\" class=\"seek\" data-t=\"{:.2}\" data-video=\"{}\">Ver en el video · {}:{:02}</button>",
                t,
                esc(v),
                (t as i64) / 60,
                (t as i64) % 60
            ),
            _ => "<span></span>".to_string(),
        };
        let donde = if multiples {
            format!("<span class=\"where\">{}</span>", esc(h["flujo"].as_str().unwrap_or("")))
        } else {
            String::new()
        };
        out.push_str(&format!(
            "<li><span class=\"sev {g}\" role=\"img\" aria-label=\"{e}\" title=\"{e}\"></span><span class=\"what\">{t}</span>{seek}<span class=\"why\">{d}</span>{donde}</li>",
            g = gravedad,
            e = etiqueta,
            t = esc(h["titulo"].as_str().unwrap_or("")),
            seek = seek,
            d = esc(h["detalle"].as_str().unwrap_or("")),
            donde = donde,
        ));
    }
    out.push_str("</ol></section>");
}

pub fn render(record: &Value) -> String {
    let (estado, clase) = estado_general(record["estado"].as_str().unwrap_or(""));
    let (_, estado_svg) = icono(record["estado"].as_str().unwrap_or(""));
    let flujos = record["flujos"].as_array().cloned().unwrap_or_default();
    let titulo = if record["tipo"] == "workspace" {
        format!("Workspace con {} flujos", flujos.len())
    } else {
        flujos.first().and_then(|f| f["nombre"].as_str()).unwrap_or("Ejecución").to_string()
    };
    let dispositivo = record["dispositivo"].as_object();
    let device_name = dispositivo
        .map(|d| {
            let nombre = d.get("nombre").and_then(Value::as_str).unwrap_or("");
            // "iPhone 17 - iOS 27.0 - UDID" → "iPhone 17 · iOS 27.0" como el selector de la app
            let partes: Vec<&str> = nombre.split(" - ").collect();
            if partes.len() >= 2 { format!("{} • {}", partes[0], partes[1]) } else { nombre.to_string() }
        })
        .unwrap_or_else(|| "Sin dispositivo conectado".to_string());
    let total_pasos: usize = flujos.iter().map(|f| f["pasos"].as_array().map(|p| p.len()).unwrap_or(0)).sum();
    let fallidos: usize = flujos
        .iter()
        .flat_map(|f| f["pasos"].as_array().cloned().unwrap_or_default())
        .filter(|p| p["estado"] == "FAILED")
        .count();
    let first_video = flujos.iter().find_map(|f| f["video"].as_str());
    let captura = record["capturas"].as_array().and_then(|c| c.first()).and_then(Value::as_str);

    let mut html = String::new();
    html.push_str(&format!(
        "<!doctype html><html lang=\"es\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1,viewport-fit=cover\"><title>{} · Clean Studio</title><style>{}</style></head><body>",
        esc(&titulo),
        CSS.replace("__FONT__", &base64(DM_SANS))
    ));
    html.push_str(&format!(
        "<header class=\"top\"><div class=\"brand\">{}<span>Clean Studio</span></div><h1>{}</h1><span class=\"badge {}\">{}{}</span></header>",
        LOGO,
        esc(&titulo),
        clase,
        estado_svg,
        estado
    ));

    // Panel del dispositivo: video (o la primera captura) dentro del marco
    html.push_str("<div class=\"layout\"><aside class=\"device\"><div class=\"device-inner\">");
    html.push_str(&format!(
        "<div class=\"select\"><svg class=\"dot\" viewBox=\"0 0 16 16\" aria-hidden=\"true\"><circle cx=\"8\" cy=\"8\" r=\"7\"/><path d=\"M5 8.2l2 2 4-4.4\"/></svg>{}</div>",
        esc(&device_name)
    ));
    html.push_str("<div class=\"phone\">");
    match (first_video, captura) {
        (Some(v), _) => html.push_str(&format!(
            "<video id=\"player\" src=\"{}\" controls playsinline preload=\"metadata\"></video>",
            esc(v)
        )),
        (None, Some(c)) => html.push_str(&format!("<img src=\"{}\" alt=\"Captura de la ejecución\">", esc(c))),
        _ => html.push_str("<div class=\"empty\">Esta ejecución se guardó sin video.</div>"),
    }
    html.push_str("</div>");
    if first_video.is_some() {
        html.push_str("<p class=\"hint\">Toca un paso para ver ese momento en el video.</p>");
    }
    html.push_str(&format!(
        "<dl class=\"facts\"><dt>Inicio</dt><dd>{}</dd><dt>Duración</dt><dd>{}</dd><dt>Pasos</dt><dd>{} en total, {} con error</dd><dt>Plataforma</dt><dd>{}</dd></dl>",
        fecha(record["inicio"].as_str().unwrap_or("")),
        duracion(record["duracionMs"].as_i64()),
        total_pasos,
        fallidos,
        esc(dispositivo.and_then(|d| d.get("plataforma")).and_then(Value::as_str).unwrap_or("—"))
    ));
    html.push_str("</div></aside><main class=\"runs\">");
    calidad_html(record, &mut html);

    for flujo in &flujos {
        flujo_html(flujo, &mut html);
    }

    let capturas: Vec<&str> = record["capturas"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !capturas.is_empty() {
        html.push_str("<div class=\"section\"><h3>Capturas</h3><div class=\"shots\">");
        for c in capturas {
            html.push_str(&format!(
                "<figure><a href=\"{0}\"><img src=\"{0}\" alt=\"{0}\" loading=\"lazy\"></a><figcaption>{0}</figcaption></figure>",
                esc(c)
            ));
        }
        html.push_str("</div></div>");
    }
    html.push_str("<footer>Generado por Clean Studio. La prueba se ejecutó en tu Mac.</footer></main></div>");
    html.push_str(&format!("<script>{}</script></body></html>", JS));
    html
}

#[cfg(test)]
mod tests {
    /// Regenera el reporte de una ejecución guardada con el diseño actual.
    /// `CS_RECORD=<carpeta de la ejecución> cargo test --lib -- --ignored regenerar`
    #[test]
    #[ignore]
    fn regenerar_reporte() {
        let dir = std::path::PathBuf::from(std::env::var("CS_RECORD").expect("CS_RECORD"));
        let record: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("ejecucion.json")).unwrap()).unwrap();
        std::fs::write(dir.join("reporte.html"), super::render(&record)).unwrap();
    }

    #[test]
    fn base64_coincide_con_el_estandar() {
        assert_eq!(super::base64(b"Clean"), "Q2xlYW4=");
        assert_eq!(super::base64(b"Cl"), "Q2w=");
        assert_eq!(super::base64(b"Cle"), "Q2xl");
    }

    /// Regenera `reporte.html` de una ejecución guardada con el diseño actual.
    /// `CS_EJECUCION=<carpeta de la ejecución> cargo test --lib -- --ignored regenera_reporte`
    #[test]
    #[ignore]
    fn regenera_reporte() {
        let dir = std::path::PathBuf::from(std::env::var("CS_EJECUCION").expect("falta CS_EJECUCION"));
        let record: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("ejecucion.json")).unwrap()).unwrap();
        std::fs::write(dir.join("reporte.html"), super::render(&record)).unwrap();
    }
}
