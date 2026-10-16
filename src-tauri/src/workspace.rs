//! Workspace y sistema de archivos (puerto de `sA`, `PH` y los manejadores de `Cme`).
use crate::{bridge, AppState};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Manager};

const PRELOAD_DEPTH: usize = 2;
const IGNORED_DIRS: &[&str] = &["node_modules", ".git", ".cache", "dist", "build", ".next", ".turbo", ".venv"];
const WATCH_INTERVAL: Duration = Duration::from_millis(1000);
const WATCH_MAX_ENTRIES: usize = 50_000;

#[derive(Default)]
pub struct WorkspaceState {
    root: Mutex<Option<PathBuf>>,
    expanded: Mutex<HashSet<PathBuf>>,
    generation: AtomicU64,
}

fn is_ignored(name: &str) -> bool {
    IGNORED_DIRS.contains(&name) || name.ends_with(".log") || name == ".DS_Store"
}

fn ok() -> Value {
    json!({ "success": true })
}

fn fail(msg: impl Into<String>) -> Value {
    json!({ "success": false, "error": msg.into() })
}

fn str_arg<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Árbol de archivos
// ---------------------------------------------------------------------------

/// Un directorio se carga si está dentro de la profundidad inicial, si fue expandido
/// o si es hermano/hijo directo de una carpeta expandida (como `ome`/`ame`).
fn should_expand(path: &Path, depth: usize, expanded: &HashSet<PathBuf>) -> bool {
    if depth < PRELOAD_DEPTH || expanded.contains(path) {
        return true;
    }
    let parent = path.parent();
    expanded.iter().any(|e| {
        e.parent() == parent && e != path || Some(e.as_path()) == parent
    })
}

fn scan(dir: &Path, depth: usize, expanded: &HashSet<PathBuf>) -> Vec<Value> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut entries: Vec<(bool, String, Value)> = rd
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if is_ignored(&name) {
                return None;
            }
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();
            let is_dir = std::fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false);
            if !is_dir {
                return Some((false, name.clone(), json!({ "name": name, "path": path_str, "type": "file" })));
            }
            let children = if should_expand(&path, depth, expanded) {
                Value::Array(scan(&path, depth + 1, expanded))
            } else {
                // Sin cargar: `undefined` si tiene contenido, `[]` si está vacío
                match std::fs::read_dir(&path).map(|mut it| it.next().is_none()) {
                    Ok(true) => json!([]),
                    _ => Value::Null,
                }
            };
            let mut item = json!({ "name": name, "path": path_str, "type": "directory" });
            if !children.is_null() {
                item["children"] = children;
            }
            Some((true, name, item))
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| locale_cmp(&a.1, &b.1)));
    entries.into_iter().map(|(_, _, v)| v).collect()
}

/// Aproximación a `localeCompare`: sin distinguir mayúsculas primero.
fn locale_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| b.cmp(a))
}

fn send_tree(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(root) = state.workspace.root.lock().unwrap().clone() else { return };
    let expanded = state.workspace.expanded.lock().unwrap().clone();
    let items = scan(&root, 0, &expanded);
    bridge::send(app, "file-system-data", vec![Value::Array(items)]);
}

/// Huella del árbol (rutas + mtime) para detectar cambios sin dependencias nativas.
fn fingerprint(root: &Path) -> BTreeMap<PathBuf, Option<SystemTime>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            if out.len() >= WATCH_MAX_ENTRIES {
                return out;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if is_ignored(&name) {
                continue;
            }
            let meta = entry.metadata().ok();
            let path = entry.path();
            if meta.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
                stack.push(path.clone());
            }
            out.insert(path, meta.and_then(|m| m.modified().ok()));
        }
    }
    out
}

pub fn setup_watcher(app: &AppHandle, dir: &str, expanded: Vec<String>) {
    let state = app.state::<AppState>();
    let root = PathBuf::from(dir);
    {
        let mut current = state.workspace.root.lock().unwrap();
        let mut exp = state.workspace.expanded.lock().unwrap();
        if current.as_ref() != Some(&root) {
            exp.clear();
        }
        exp.extend(expanded.into_iter().map(PathBuf::from));
        *current = Some(root.clone());
    }
    let generation = state.workspace.generation.fetch_add(1, Ordering::SeqCst) + 1;
    send_tree(app);

    let app = app.clone();
    thread::spawn(move || {
        let mut last = fingerprint(&root);
        loop {
            thread::sleep(WATCH_INTERVAL);
            if app.state::<AppState>().workspace.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let now = fingerprint(&root);
            if now != last {
                last = now;
                send_tree(&app);
            }
        }
    });
}

pub fn directory_contents(app: &AppHandle, path: &str) {
    let state = app.state::<AppState>();
    let p = PathBuf::from(path);
    state.workspace.expanded.lock().unwrap().insert(p.clone());
    let expanded = state.workspace.expanded.lock().unwrap().clone();
    let items = scan(&p, 0, &expanded);
    bridge::send(app, "directory-contents", vec![json!(path), Value::Array(items)]);
}

// ---------------------------------------------------------------------------
// Archivos
// ---------------------------------------------------------------------------

fn read_text(path: &str) -> String {
    match std::fs::read(path) {
        // Igual que el original: los binarios se entregan vacíos
        Ok(bytes) if !bytes.contains(&0) => String::from_utf8(bytes).unwrap_or_default(),
        _ => String::new(),
    }
}

pub fn load_file_content(app: &AppHandle, path: &str) {
    bridge::send(app, "file-content-loaded", vec![json!({ "content": read_text(path), "path": path })]);
}

pub fn open_file(app: &AppHandle, opts: &Value) {
    let exts: Vec<String> = opts
        .get("extensions")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_else(|| vec!["yaml".into(), "yml".into()]);
    let ext_refs: Vec<&str> = exts.iter().map(String::as_str).collect();
    if let Some(path) = bridge::choose_file("Abrir archivo YAML", &ext_refs) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            bridge::send(app, "file-opened", vec![json!({ "content": content, "path": path })]);
        }
    }
}

pub fn save_file(app: &AppHandle, opts: &Value) {
    let content = str_arg(opts, "content");
    let mut path = str_arg(opts, "path").to_string();
    if path.is_empty() {
        match bridge::choose_file_name("Guardar flujo", "flow.yaml") {
            Some(p) => path = p,
            None => return,
        }
    }
    match std::fs::write(&path, content) {
        Ok(_) => bridge::send(app, "alert", vec![json!("File saved successfully.")]),
        Err(e) => bridge::send(app, "alert", vec![json!(format!("Failed to save file: {}", e))]),
    }
}

pub fn open_workspace(app: &AppHandle) -> Value {
    match bridge::choose_folder("Selecciona la carpeta del workspace") {
        Some(dir) => {
            app.state::<AppState>().settings.add_recent_workspace(&dir);
            json!(dir)
        }
        None => Value::Null,
    }
}

pub fn create_workspace(app: &AppHandle) -> Value {
    let Some(dir) = bridge::choose_file_name("Crea una carpeta nueva para tu workspace", "Maestro") else {
        return Value::Null;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return Value::Null;
    }
    app.state::<AppState>().settings.add_recent_workspace(&dir);
    setup_watcher(app, &dir, Vec::new());
    json!(dir)
}

pub fn recent_workspaces(app: &AppHandle) -> Value {
    json!(app.state::<AppState>().settings.recent_workspaces())
}

const EXISTS_DIR: &str = "A folder with this name already exists. Please choose a different name.";
const EXISTS_FILE: &str =
    "A file with this name already exists. Please choose a different name or rename the existing file.";

fn exists_error(path: &Path) -> Option<Value> {
    if !path.exists() {
        return None;
    }
    Some(fail(if path.is_dir() { EXISTS_DIR } else { EXISTS_FILE }))
}

pub fn create_file(app: &AppHandle, opts: &Value) -> Value {
    let path = PathBuf::from(str_arg(opts, "path"));
    if let Some(err) = exists_error(&path) {
        return err;
    }
    match std::fs::write(&path, str_arg(opts, "content")) {
        Ok(_) => {
            send_tree(app);
            ok()
        }
        Err(e) => fail(e.to_string()),
    }
}

pub fn create_directory(app: &AppHandle, opts: &Value) -> Value {
    let path = PathBuf::from(str_arg(opts, "path"));
    if let Some(err) = exists_error(&path) {
        return err;
    }
    match std::fs::create_dir_all(&path) {
        Ok(_) => {
            send_tree(app);
            ok()
        }
        Err(e) => fail(e.to_string()),
    }
}

/// Mueve a la Papelera (como `shell.trashItem`), con nombre único si ya existe.
fn move_to_trash(path: &Path) -> std::io::Result<()> {
    let trash = crate::paths::home_dir().join(".Trash");
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut target = trash.join(&name);
    let mut n = 1;
    while target.exists() {
        let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        target = trash.join(format!("{} {}-{}", name, stamp, n));
        n += 1;
    }
    std::fs::rename(path, &target)
}

pub fn delete(app: &AppHandle, opts: &Value) -> Value {
    let path = PathBuf::from(str_arg(opts, "path"));
    if !path.exists() {
        return fail("File or directory does not exist");
    }
    match move_to_trash(&path) {
        Ok(_) => {
            send_tree(app);
            ok()
        }
        Err(e) => fail(e.to_string()),
    }
}

pub fn duplicate(app: &AppHandle, opts: &Value) -> Value {
    let path = PathBuf::from(str_arg(opts, "path"));
    if !path.exists() {
        return fail("File does not exist");
    }
    if !path.is_file() {
        return fail("Only files can be duplicated");
    }
    let dir = path.parent().unwrap_or(Path::new("/"));
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let mut n = 1;
    let target = loop {
        let suffix = if n == 1 { " copy".to_string() } else { format!(" copy {}", n) };
        let candidate = dir.join(format!("{}{}{}", stem, suffix, ext));
        if !candidate.exists() {
            break candidate;
        }
        n += 1;
    };
    match std::fs::copy(&path, &target) {
        Ok(_) => {
            send_tree(app);
            json!({ "success": true, "path": target })
        }
        Err(e) => fail(e.to_string()),
    }
}

pub fn move_files(app: &AppHandle, opts: &Value) -> Value {
    let dest = PathBuf::from(str_arg(opts, "destinationPath"));
    let sources: Vec<String> = opts
        .get("sourcePaths")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let results: Vec<Value> = sources
        .iter()
        .map(|src| {
            let src_path = PathBuf::from(src);
            if !src_path.exists() {
                return json!({ "path": src, "success": false, "error": "Source file does not exist" });
            }
            if !dest.exists() {
                return json!({ "path": src, "success": false, "error": "Destination directory does not exist" });
            }
            let target = dest.join(src_path.file_name().unwrap_or_default());
            if target.exists() {
                return json!({
                    "path": src,
                    "success": false,
                    "error": "A file with this name already exists in the destination"
                });
            }
            match std::fs::rename(&src_path, &target) {
                Ok(_) => json!({ "path": src, "success": true }),
                Err(e) => json!({ "path": src, "success": false, "error": e.to_string() }),
            }
        })
        .collect();
    send_tree(app);
    let failed = results.iter().filter(|r| r["success"] != json!(true)).count();
    let mut out = json!({ "success": failed == 0, "results": results });
    if failed > 0 {
        out["error"] = json!(format!("Failed to move {} file(s)", failed));
    }
    out
}

pub fn rename(app: &AppHandle, opts: &Value) -> Value {
    let old = PathBuf::from(str_arg(opts, "oldPath"));
    if !old.exists() {
        return fail("File or directory does not exist");
    }
    let target = old.parent().unwrap_or(Path::new("/")).join(str_arg(opts, "newName"));
    if target.exists() {
        return fail("A file or directory with this name already exists");
    }
    match std::fs::rename(&old, &target) {
        Ok(_) => {
            send_tree(app);
            ok()
        }
        Err(e) => fail(e.to_string()),
    }
}

pub fn pick_app_file() -> Value {
    bridge::choose_file("Selecciona la app (.apk o .app)", &["apk", "app", "com.apple.application-bundle"])
        .map(Value::from)
        .unwrap_or(Value::Null)
}

pub fn read_file(path: &str) -> Result<Value, String> {
    std::fs::read(path)
        .map(|bytes| json!({ "type": "Buffer", "data": bytes }))
        .map_err(|e| format!("Failed to read file: {}", e))
}

pub fn show_in_file_manager(path: &str) -> Value {
    if path.is_empty() || !Path::new(path).exists() {
        return fail("File or directory does not exist");
    }
    bridge::reveal_in_finder(path);
    ok()
}
