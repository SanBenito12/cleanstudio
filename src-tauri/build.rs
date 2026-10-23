fn main() {
    // Extractor de cuadros del detector de parpadeos (src/cuadros.swift), compilado con el
    // swiftc de Xcode e incrustado en el binario (calidad.rs lo escribe en la caché al usarlo).
    // Sin Xcode queda vacío y el informe de calidad omite los parpadeos.
    println!("cargo:rerun-if-changed=src/cuadros.swift");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("cuadros");
    let ok = std::process::Command::new("xcrun")
        .args(["swiftc", "-O", "src/cuadros.swift", "-o"])
        .arg(&out)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        println!("cargo:warning=No se pudo compilar src/cuadros.swift: el informe no detectará parpadeos");
        let _ = std::fs::write(&out, b"");
    }
    tauri_build::build()
}
