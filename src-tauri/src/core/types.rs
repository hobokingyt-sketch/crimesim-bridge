use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeProject {
    pub schema: u32,
    pub project_id: String,
    pub project_name: String,
    pub engine: String,
    pub engine_version: String,
    pub revision: u64,
    pub save_schema: u64,
    pub main_scene: String,
    pub export_preset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind { Create, Replace, Delete }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperation {
    pub op: OperationKind,
    pub path: String,
    pub base_sha256: Option<String>,
    pub new_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub schema: u32,
    pub package_type: String,
    pub project_id: String,
    pub engine_version: String,
    pub base_revision: u64,
    pub target_revision: u64,
    pub created_at: String,
    pub summary: String,
    pub operations: Vec<FileOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationStep {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub passed: bool,
    pub revision: u64,
    pub created_at: String,
    pub steps: Vec<ValidationStep>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStatus {
    pub bridge_version: String,
    pub workspace_root: String,
    pub project_present: bool,
    pub project_name: Option<String>,
    pub source_revision: Option<u64>,
    pub playable_revision: Option<u64>,
    pub engine_version: Option<String>,
    pub godot_runtime_present: bool,
    pub godot_runtime_integrity: String,
    pub pipeline_ready: bool,
    pub latest_update: Option<String>,
    pub last_validation: Option<ValidationReport>,
    pub last_recovery: Option<String>,
    pub recovery_required: bool,
    pub recovery_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub ok: bool,
    pub title: String,
    pub detail: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetCatalogEntry {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub included: bool,
    pub reason: Option<String>,
}
