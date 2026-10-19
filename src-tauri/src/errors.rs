//! Traductor de pasos y errores del motor al español, para los reportes.
//! Usa la misma tabla que la interfaz (`src/shim/errores_es.json`).
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Tabla {
    pasos: Vec<(String, String)>,
    errores: Vec<Entrada>,
}

#[derive(Deserialize)]
struct Entrada {
    patron: String,
    explicacion: String,
    sugerencia: String,
}

struct Compilada {
    pasos: Vec<(Regex, String)>,
    errores: Vec<(Regex, String, String)>,
}

fn tabla() -> &'static Compilada {
    static TABLA: OnceLock<Compilada> = OnceLock::new();
    TABLA.get_or_init(|| {
        let t: Tabla = serde_json::from_str(include_str!("../../src/shim/errores_es.json"))
            .expect("errores_es.json inválido");
        Compilada {
            pasos: t.pasos.into_iter().filter_map(|(p, to)| Regex::new(&p).ok().map(|re| (re, to))).collect(),
            errores: t
                .errores
                .into_iter()
                .filter_map(|e| Regex::new(&e.patron).ok().map(|re| (re, e.explicacion, e.sugerencia)))
                .collect(),
        }
    })
}

/// Nombre del paso en español ("Tap on id: x" → "Tocar id: x").
pub fn paso(label: &str) -> String {
    for (re, to) in &tabla().pasos {
        if re.is_match(label) {
            return re.replace(label, to.as_str()).to_string();
        }
    }
    label.to_string()
}

/// Explicación y sugerencia en español para un error del motor, si se reconoce.
pub fn explicar(msg: &str) -> Option<(String, String)> {
    for (re, explicacion, sugerencia) in &tabla().errores {
        if let Some(caps) = re.captures(msg) {
            let mut e = String::new();
            caps.expand(explicacion, &mut e);
            let mut s = String::new();
            caps.expand(sugerencia, &mut s);
            return Some((e, s));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traduce_pasos_y_errores() {
        assert_eq!(paso("Tap on id: input_correo"), "Tocar id: input_correo");
        assert_eq!(
            paso("Scrolling DOWN until \"Agregar\" is visible with speed 40, timeout 20000 ms"),
            "Desplazar (DOWN) hasta que \"Agregar\" esté visible"
        );
        assert_eq!(
            paso("Launch app \"com.x\" with clear state"),
            "Abrir la app \"com.x\" con estado limpio"
        );
        let (e, _) = explicar("Failed to get app binary directory for bundle com.x on device A").unwrap();
        assert_eq!(e, "La app com.x no está instalada en el dispositivo.");
        assert!(explicar("algo desconocido").is_none());
    }
}
