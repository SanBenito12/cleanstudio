//! Ajustes persistentes (equivalente al electron-store `Ri` del proceso main original).
use crate::paths;
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

const MAX_RECENT_WORKSPACES: usize = 6;

pub struct Settings {
    file: Mutex<Option<PathBuf>>,
    data: Mutex<Map<String, Value>>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { file: Mutex::new(None), data: Mutex::new(Map::new()) }
    }
}

impl Settings {
    pub fn load(&self, app: &AppHandle) {
        let Ok(dir) = app.path().app_config_dir() else { return };
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("config.json");
        if let Ok(text) = std::fs::read_to_string(&file) {
            if let Ok(Value::Object(map)) = serde_json::from_str(&text) {
                *self.data.lock().unwrap() = map;
            }
        }
        *self.file.lock().unwrap() = Some(file);
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.data.lock().unwrap().get(key).cloned()
    }

    pub fn set(&self, key: &str, value: Value) {
        let snapshot = {
            let mut data = self.data.lock().unwrap();
            data.insert(key.to_string(), value);
            Value::Object(data.clone())
        };
        if let Some(file) = self.file.lock().unwrap().as_ref() {
            let _ = std::fs::write(file, serde_json::to_string_pretty(&snapshot).unwrap_or_default());
        }
    }

    pub fn android_sdk_path(&self) -> Option<String> {
        self.get("androidSdkPath")
            .and_then(|v| v.as_str().map(String::from))
            .filter(|s| !s.is_empty())
            .or_else(paths::detect_android_sdk)
    }

    pub fn set_android_sdk_path(&self, path: &str) {
        self.set("androidSdkPath", json!(paths::expand_home(path)));
    }

    pub fn bool_or(&self, key: &str, default: bool) -> bool {
        self.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    pub fn editor_settings(&self) -> Value {
        self.get("editorSettings").unwrap_or_else(|| json!({ "autosaveEnabled": true }))
    }

    pub fn recent_workspaces(&self) -> Vec<String> {
        self.get("recentWorkspaces")
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .unwrap_or_default()
            .into_iter()
            .take(MAX_RECENT_WORKSPACES)
            .collect()
    }

    pub fn add_recent_workspace(&self, path: &str) {
        if path.is_empty() {
            return;
        }
        let mut list = vec![path.to_string()];
        list.extend(self.recent_workspaces().into_iter().filter(|p| p != path));
        list.truncate(MAX_RECENT_WORKSPACES);
        self.set("recentWorkspaces", json!(list));
    }
}
