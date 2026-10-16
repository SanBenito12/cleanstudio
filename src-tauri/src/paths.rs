//! Localización de los motores (studio-server.jar, JVM, simulator-server) y del Android SDK.
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const MAESTRO_UNPACKED: &str = "/Applications/Maestro Studio.app/Contents/Resources/app.asar.unpacked";

/// Raíz del repositorio en desarrollo (`clean-studio/`).
pub fn project_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Directorios donde buscar los motores, en orden de preferencia:
/// recursos empaquetados → `resources/engines/` del repo → Maestro Studio instalado.
fn engine_roots(app: &AppHandle) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        roots.push(res.join("engines"));
    }
    roots.push(project_dir().join("resources").join("engines"));
    roots.push(PathBuf::from(MAESTRO_UNPACKED));
    roots
}

fn find_engine(app: &AppHandle, rel: &Path) -> PathBuf {
    let roots = engine_roots(app);
    roots
        .iter()
        .map(|r| r.join(rel))
        .find(|p| p.exists())
        .unwrap_or_else(|| roots.last().unwrap().join(rel))
}

pub fn studio_jar(app: &AppHandle) -> PathBuf {
    find_engine(app, Path::new("dist-server/studio-server.jar"))
}

pub fn java_home(app: &AppHandle) -> PathBuf {
    let arch = if cfg!(target_arch = "aarch64") { "mac-arm64" } else { "mac-x64" };
    find_engine(app, &Path::new("bundled-jvm").join(arch).join("Contents/Home"))
}

pub fn java_bin(app: &AppHandle) -> PathBuf {
    java_home(app).join("bin").join("java")
}

pub fn simulator_server(app: &AppHandle) -> Option<PathBuf> {
    let p = find_engine(app, Path::new("bundled-simulator-server/mac-universal/simulator-server"));
    p.exists().then_some(p)
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// `$HOME` y `~` como en el original (`cH` + replace("$HOME")).
pub fn expand_home(p: &str) -> String {
    let home = home_dir().to_string_lossy().to_string();
    let p = p.replacen("$HOME", &home, 1);
    if p == "~" {
        home
    } else if let Some(rest) = p.strip_prefix("~/") {
        format!("{}/{}", home, rest)
    } else {
        p
    }
}

fn sdk_with_adb(p: &Path) -> Option<String> {
    p.join("platform-tools").join("adb").exists().then(|| p.to_string_lossy().to_string())
}

/// Detección automática del Android SDK (puerto de `Upe`/`kpe` del main.js original).
pub fn detect_android_sdk() -> Option<String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(v) = std::env::var_os(var) {
            candidates.push(PathBuf::from(v));
        }
    }
    candidates.push(home_dir().join("Library/Android/sdk"));
    candidates.push(PathBuf::from("/opt/homebrew/share/android-sdk"));
    candidates.push(PathBuf::from("/usr/local/share/android-sdk"));
    if let Some(found) = candidates.iter().find_map(|c| sdk_with_adb(c)) {
        return Some(found);
    }
    // adb de Homebrew enlazado: <sdk>/platform-tools/adb
    for adb in ["/opt/homebrew/bin/adb", "/usr/local/bin/adb"] {
        if let Ok(real) = std::fs::canonicalize(adb) {
            let tools = real.parent()?;
            if tools.file_name().map(|n| n == "platform-tools").unwrap_or(false) {
                return tools.parent().map(|p| p.to_string_lossy().to_string());
            }
        }
    }
    for cask in [
        "/opt/homebrew/Caskroom/android-platform-tools",
        "/usr/local/Caskroom/android-platform-tools",
    ] {
        if let Ok(rd) = std::fs::read_dir(cask) {
            let mut versions: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
            versions.sort();
            if let Some(found) = versions.last().and_then(|v| sdk_with_adb(v)) {
                return Some(found);
            }
        }
    }
    None
}

/// `null` si es válido; si no, el mensaje de error del original.
pub fn validate_android_sdk(path: &str) -> Option<String> {
    let p = PathBuf::from(expand_home(path));
    if !p.join("platform-tools/adb").exists() {
        return Some("Missing tool: platform-tools/adb".to_string());
    }
    if !p.join("emulator/emulator").exists() {
        eprintln!("El Android SDK en {:?} no tiene emulator/emulator; los flujos AVD no funcionarán.", p);
    }
    None
}
