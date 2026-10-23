//! Detector de parpadeos en el video de una ejecución (para el informe de calidad).
//!
//! El video se reduce a miniaturas en gris (36 px de ancho, 10 cuadros por segundo) con el
//! extractor de src/cuadros.swift. Se buscan dos cosas:
//! - Destellos: toda la pantalla se pone mucho más clara u oscura durante 0,1–0,3 s y vuelve.
//! - Zonas que parpadean: un área de la pantalla que cambia de ida y vuelta varias veces sin que la
//!   prueba haya tocado nada (se ignoran los 1,5 s que siguen a cada paso, donde las transiciones
//!   son normales).
//!
//! Una animación de carga también cambia de ida y vuelta: por eso el informe lo llama "posible
//! parpadeo" y enlaza el momento del video para que lo juzgue una persona.
use std::path::{Path, PathBuf};
use std::process::Command;

/// Extractor compilado por build.rs (vacío si no había Xcode al compilar).
const EXTRACTOR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cuadros"));
const FPS: f64 = 10.0;
const ANCHO: usize = 36;
const CELDA: usize = 4;

pub struct Cuadros {
    pub ancho: usize,
    pub alto: usize,
    pub fps: f64,
    pub cuadros: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parpadeo {
    pub tipo: &'static str, // "destello" | "zona"
    pub desde: f64,         // segundos dentro del video
    pub hasta: f64,
    pub zona: String,
    pub veces: usize,
}

/// Salida del extractor: "ancho alto cantidad fps\n" + cuadros de ancho*alto bytes.
pub fn interpretar(bytes: &[u8]) -> Option<Cuadros> {
    let fin = bytes.iter().position(|b| *b == b'\n')?;
    let cab: Vec<&str> = std::str::from_utf8(&bytes[..fin]).ok()?.split_whitespace().collect();
    let (ancho, alto, n): (usize, usize, usize) = (cab.first()?.parse().ok()?, cab.get(1)?.parse().ok()?, cab.get(2)?.parse().ok()?);
    let fps: f64 = cab.get(3)?.parse().ok()?;
    let datos = &bytes[fin + 1..];
    let tam = ancho * alto;
    if tam == 0 || datos.len() < tam * n {
        return None;
    }
    let cuadros = (0..n).map(|i| datos[i * tam..(i + 1) * tam].to_vec()).collect();
    Some(Cuadros { ancho, alto, fps, cuadros })
}

/// Ruta del extractor (src/cuadros.swift) listo para ejecutar; `None` si se compiló sin Xcode.
/// También lo usa revision.rs para sacar capturas PNG.
pub fn extractor() -> Option<PathBuf> {
    if EXTRACTOR.is_empty() {
        return None;
    }
    let exe = std::env::temp_dir().join(format!("clean-studio-cuadros-{}", EXTRACTOR.len()));
    if !exe.exists() {
        std::fs::write(&exe, EXTRACTOR).ok()?;
        let _ = Command::new("chmod").arg("755").arg(&exe).status();
    }
    Some(exe)
}

/// Extrae los cuadros de un video. `None` si no hay extractor o el video no se puede leer.
pub fn leer(video: &Path) -> Option<Cuadros> {
    let exe = extractor()?;
    let out = Command::new(&exe).arg(video).arg(FPS.to_string()).arg(ANCHO.to_string()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    interpretar(&out.stdout)
}

fn zona(x0: usize, y0: usize, x1: usize, y1: usize, gw: usize, gh: usize) -> String {
    let cx = (x0 + x1) as f64 / 2.0 / gw as f64;
    let cy = (y0 + y1) as f64 / 2.0 / gh as f64;
    let v = if cy < 0.33 { "arriba" } else if cy < 0.66 { "en medio" } else { "abajo" };
    let h = if cx < 0.33 { " a la izquierda" } else if cx < 0.66 { "" } else { " a la derecha" };
    if v == "en medio" && h.is_empty() {
        "en el centro de la pantalla".to_string()
    } else {
        format!("{}{} de la pantalla", if v == "en medio" { "a media altura" } else { v }, h)
    }
}

/// `pasos`: segundo del video en que empezó cada paso.
pub fn detectar(c: &Cuadros, pasos: &[f64]) -> Vec<Parpadeo> {
    let n = c.cuadros.len();
    if n < 4 {
        return Vec::new();
    }
    let (gw, gh) = (c.ancho / CELDA, c.alto / CELDA);
    // Brillo medio de cada celda y de toda la pantalla, por cuadro
    let celdas: Vec<Vec<f32>> = c
        .cuadros
        .iter()
        .map(|f| {
            let mut v = vec![0f32; gw * gh];
            for cy in 0..gh {
                for cx in 0..gw {
                    let mut s = 0u32;
                    for y in 0..CELDA {
                        for x in 0..CELDA {
                            s += f[(cy * CELDA + y) * c.ancho + cx * CELDA + x] as u32;
                        }
                    }
                    v[cy * gw + cx] = s as f32 / (CELDA * CELDA) as f32;
                }
            }
            v
        })
        .collect();
    let global: Vec<f32> = c.cuadros.iter().map(|f| f.iter().map(|b| *b as f32).sum::<f32>() / f.len() as f32).collect();
    let seg = |i: usize| i as f64 / c.fps;
    let mut out = Vec::new();

    // 1. Destellos de toda la pantalla
    let mut i = 1;
    while i + 1 < n {
        let mut hallado = false;
        for largo in 1..=3usize {
            if i + largo >= n {
                break;
            }
            let (antes, despues) = (global[i - 1], global[i + largo]);
            let tramo = &global[i..i + largo];
            let claro = tramo.iter().all(|g| *g - antes.max(despues) > 35.0);
            let oscuro = tramo.iter().all(|g| antes.min(despues) - *g > 35.0);
            if claro || oscuro {
                out.push(Parpadeo {
                    tipo: "destello",
                    desde: seg(i),
                    hasta: seg(i + largo),
                    zona: if claro { "toda la pantalla se puso clara".into() } else { "toda la pantalla se puso oscura".into() },
                    veces: 1,
                });
                i += largo;
                hallado = true;
                break;
            }
        }
        if !hallado {
            i += 1;
        }
    }

    // 2. Zonas que cambian de ida y vuelta en momentos tranquilos (lejos de los pasos)
    let tranquilo = |i: usize| !pasos.iter().any(|p| seg(i) >= p - 0.3 && seg(i) <= p + 1.5);
    let ventana = (2.0 * c.fps) as usize;
    let mut i0 = 0;
    let mut actual: Option<Parpadeo> = None;
    while i0 + ventana < n {
        let fin = i0 + ventana;
        let activas: Vec<bool> = if (i0..=fin).all(tranquilo) {
            (0..gw * gh)
                .map(|k| {
                    // Cambios fuertes que alternan de sentido (aclara, oscurece, aclara, oscurece).
                    // Una transición, como un logo que crece y se desvanece, va en un solo sentido
                    // o cambia de sentido una vez.
                    let mut idas_y_vueltas = 0;
                    let mut ultimo = 0f32;
                    for j in i0 + 1..=fin {
                        let d = celdas[j][k] - celdas[j - 1][k];
                        if d.abs() > 30.0 {
                            if ultimo != 0.0 && d.signum() != ultimo.signum() {
                                idas_y_vueltas += 1;
                            }
                            ultimo = d;
                        }
                    }
                    idas_y_vueltas >= 3 && (celdas[fin][k] - celdas[i0][k]).abs() < 15.0
                })
                .collect()
        } else {
            vec![false; gw * gh]
        };
        // La zona activa más grande (celdas vecinas); 3 celdas o más para no contar puntos sueltos
        let mut visto = vec![false; gw * gh];
        let mut mejor: Option<(usize, usize, usize, usize, usize)> = None;
        for k in 0..gw * gh {
            if !activas[k] || visto[k] {
                continue;
            }
            let (mut pila, mut tam) = (vec![k], 0);
            let (mut x0, mut y0, mut x1, mut y1) = (gw, gh, 0, 0);
            visto[k] = true;
            while let Some(q) = pila.pop() {
                tam += 1;
                let (x, y) = (q % gw, q / gw);
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                let vecinos = [(x > 0).then(|| q - 1), (x + 1 < gw).then(|| q + 1), (y > 0).then(|| q - gw), (y + 1 < gh).then(|| q + gw)];
                for v in vecinos.into_iter().flatten() {
                    if activas[v] && !visto[v] {
                        visto[v] = true;
                        pila.push(v);
                    }
                }
            }
            if tam >= 3 && mejor.map(|m| tam > m.4).unwrap_or(true) {
                mejor = Some((x0, y0, x1, y1, tam));
            }
        }
        match (mejor, actual.as_mut()) {
            // Ventanas seguidas con parpadeo forman un solo aviso
            (Some(_), Some(p)) if seg(i0) <= p.hasta + 0.5 => {
                p.hasta = seg(fin);
                p.veces += 1;
            }
            (Some((x0, y0, x1, y1, _)), _) => {
                if let Some(p) = actual.take() {
                    out.push(p);
                }
                actual = Some(Parpadeo { tipo: "zona", desde: seg(i0), hasta: seg(fin), zona: zona(x0, y0, x1, y1, gw, gh), veces: 1 });
            }
            (None, _) => {}
        }
        i0 += (c.fps / 2.0) as usize; // ventanas de 2 s cada 0,5 s
    }
    if let Some(p) = actual {
        out.push(p);
    }
    out.sort_by(|a, b| a.desde.partial_cmp(&b.desde).unwrap());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(n: usize, f: impl Fn(usize, usize, usize) -> u8) -> Cuadros {
        let (ancho, alto) = (36, 64);
        let cuadros = (0..n)
            .map(|i| (0..ancho * alto).map(|p| f(i, p % ancho, p / ancho)).collect())
            .collect();
        Cuadros { ancho, alto, fps: 10.0, cuadros }
    }

    #[test]
    fn interpreta_la_salida_del_extractor() {
        let mut b = b"2 1 2 10\n".to_vec();
        b.extend([1, 2, 3, 4]);
        let c = interpretar(&b).unwrap();
        assert_eq!((c.ancho, c.alto, c.cuadros.len()), (2, 1, 2));
        assert_eq!(c.cuadros[1], vec![3, 4]);
        assert!(interpretar(b"2 1 3 10\n\x01\x02").is_none());
    }

    #[test]
    fn detecta_un_destello_blanco() {
        let c = video(30, |i, _, _| if i == 12 { 250 } else { 60 });
        let p = detectar(&c, &[]);
        assert_eq!(p.len(), 1, "{:?}", p);
        assert_eq!(p[0].tipo, "destello");
        assert!((p[0].desde - 1.2).abs() < 1e-9);
    }

    #[test]
    fn un_cambio_de_pantalla_no_es_destello() {
        // De una pantalla oscura a una clara, sin volver
        let c = video(30, |i, _, _| if i < 15 { 40 } else { 220 });
        assert!(detectar(&c, &[]).is_empty());
    }

    #[test]
    fn detecta_una_zona_que_parpadea_y_dice_donde() {
        // Un recuadro arriba a la derecha alterna claro/oscuro cada 0,2 s entre 1 y 4 s
        let c = video(60, |i, x, y| {
            let zona = x >= 24 && y < 16;
            if zona && (10..40).contains(&i) && (i / 2) % 2 == 0 { 230 } else { 50 }
        });
        let p = detectar(&c, &[]);
        let zona: Vec<&Parpadeo> = p.iter().filter(|p| p.tipo == "zona").collect();
        assert_eq!(zona.len(), 1, "{:?}", p);
        assert_eq!(zona[0].zona, "arriba a la derecha de la pantalla");
        assert!(zona[0].desde >= 0.5 && zona[0].desde <= 1.5, "{:?}", zona[0]);
    }

    #[test]
    fn una_animacion_de_entrada_no_es_parpadeo() {
        // Como el arranque de la app: blanco → logo oscuro que crece → gris → pantalla blanca
        let c = video(50, |i, _, y| match i {
            0..=9 => 250,
            10..=14 => if y < 32 { 60 } else { 250 },
            15..=19 => 60,
            20..=24 => 140,
            _ => 245,
        });
        assert!(detectar(&c, &[]).iter().all(|p| p.tipo != "zona"), "{:?}", detectar(&c, &[]));
    }

    #[test]
    fn ignora_lo_que_cambia_justo_despues_de_un_paso() {
        let c = video(40, |i, x, y| if x >= 24 && y < 16 && (5..20).contains(&i) && (i / 2) % 2 == 0 { 230 } else { 50 });
        // Pasos cada segundo: nunca hay 2 s tranquilos seguidos
        let pasos: Vec<f64> = (0..4).map(|s| s as f64).collect();
        assert!(detectar(&c, &pasos).iter().all(|p| p.tipo != "zona"));
    }

    /// Corre el detector sobre las ejecuciones guardadas de una carpeta e imprime lo que encuentra.
    /// `CS_EJECUCIONES=<…/.clean-studio/ejecuciones> cargo test --lib -- --ignored --nocapture videos_reales`
    #[test]
    #[ignore]
    fn videos_reales() {
        let raiz = std::path::PathBuf::from(std::env::var("CS_EJECUCIONES").expect("CS_EJECUCIONES"));
        let mut dirs: Vec<_> = std::fs::read_dir(&raiz).unwrap().flatten().map(|e| e.path()).collect();
        dirs.sort();
        for d in dirs {
            let Ok(t) = std::fs::read_to_string(d.join("ejecucion.json")) else { continue };
            let r: serde_json::Value = serde_json::from_str(&t).unwrap();
            for f in r["flujos"].as_array().into_iter().flatten() {
                let (Some(v), Some(rec)) = (f["video"].as_str(), f["grabacionInicioMs"].as_i64()) else { continue };
                let pasos: Vec<f64> = f["pasos"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| p["inicioMs"].as_i64())
                    .map(|t| (t - rec) as f64 / 1000.0)
                    .collect();
                let Some(c) = leer(&d.join(v)) else { println!("{}: sin cuadros", d.display()); continue };
                let p = detectar(&c, &pasos);
                println!("{} ({} cuadros): {:?}", d.file_name().unwrap().to_string_lossy(), c.cuadros.len(), p);
            }
        }
    }

    /// `CS_VIDEO=<video.mp4> cargo test --lib -- --ignored --nocapture un_video`
    #[test]
    #[ignore]
    fn un_video() {
        let c = leer(Path::new(&std::env::var("CS_VIDEO").expect("CS_VIDEO"))).expect("cuadros");
        let g: Vec<u32> = c.cuadros.iter().map(|f| f.iter().map(|b| *b as u32).sum::<u32>() / f.len() as u32).collect();
        println!("{} cuadros, brillo: {:?}", c.cuadros.len(), g);
        println!("{:?}", detectar(&c, &[]));
    }
}
