use serde::{Deserialize, Serialize};
use std::process::Child;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct AppState {
    pub studio_server: Arc<Mutex<Option<Child>>>,
    pub simulator_server: Arc<Mutex<Option<Child>>>,
    pub active_stream_url: Arc<Mutex<Option<String>>>,
    pub server_logs: Arc<Mutex<Vec<String>>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EngineDiagnostics {
    pub studio_server_running: bool,
    pub studio_server_port: u16,
    pub studio_jar_exists: bool,
    pub java_bin_exists: bool,
    pub simulator_bin_exists: bool,
    pub simulator_verify_output: Vec<String>,
    pub android_sdk_path: Option<String>,
    pub active_stream_url: Option<String>,
    pub maestro_oss_path: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WorkspaceFile {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LocalDevice {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub state: String,
    pub runtime: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AiFlowRequest {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub prompt: String,
    pub app_id: Option<String>,
    pub current_yaml: Option<String>,
    pub view_hierarchy_summary: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AiFlowResponse {
    pub yaml: String,
    pub explanation: String,
    pub provider_used: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReportStep {
    pub command: String,
    pub status: String,
    pub duration_ms: u64,
    pub error_es: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ExportReportRequest {
    pub flow_name: String,
    pub app_id: String,
    pub device_name: String,
    pub total_duration_ms: u64,
    pub steps: Vec<ReportStep>,
    pub yaml_content: String,
}
