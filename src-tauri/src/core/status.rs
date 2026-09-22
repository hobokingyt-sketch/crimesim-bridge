use crate::core::{error::BridgeResult, package, paths, project, runtime, worker, types::{BridgeStatus, ValidationReport}};
use std::fs;

pub fn get() -> BridgeResult<BridgeStatus> {
    let _workspace_guard = bridge_safety::Workspace::open(&paths::root()?)?;
    let project_root = paths::current_project()?;
    let project_present = project_root.join("project.godot").exists();
    let meta = if project_present { project::read_project(&project_root).ok() } else { None };
    let playable_revision = fs::read_to_string(paths::current_build()?.join("bridge_revision.txt"))
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok());
    let last_validation: Option<ValidationReport> = fs::read(paths::root()?.join(paths::LAST_VALIDATION))
        .ok()
        .and_then(|v| serde_json::from_slice(&v).ok());
    let latest_update = package::latest_update_pack()?.map(|p| p.to_string_lossy().to_string());
    let (runtime_ok, runtime_detail) = match runtime::integrity() {
        Ok(v) => v,
        Err(err) => (false, format!("integrity check error: {err}")),
    };
    let last_recovery = fs::read_to_string(paths::root()?.join(paths::LAST_RECOVERY)).ok();
    let worker_pending = paths::root()?.join(worker::PENDING_WORKER).exists();
    let recovery_required = paths::transaction_journal()?.exists() || worker_pending;
    let recovery_error = fs::read_to_string(paths::root()?.join(paths::RECOVERY_ERROR)).ok()
        .or_else(|| worker_pending.then(|| "Worker cleanup must complete before project recovery.".into()));
    let source_revision = meta.as_ref().map(|m| m.revision);
    let validation_ok = last_validation.as_ref().map(|r| r.passed).unwrap_or(false);
    let pipeline_ready = !recovery_required && recovery_error.is_none() && project_present
        && runtime_ok
        && source_revision.is_some()
        && source_revision == playable_revision
        && validation_ok
        && paths::current_build()?.join("CrimeSim.exe").is_file();

    Ok(BridgeStatus {
        bridge_version: env!("CARGO_PKG_VERSION").into(),
        workspace_root: paths::root()?.to_string_lossy().to_string(),
        project_present,
        project_name: meta.as_ref().map(|m| m.project_name.clone()),
        source_revision,
        playable_revision,
        engine_version: meta.as_ref().map(|m| m.engine_version.clone()),
        godot_runtime_present: paths::runtime_godot()?.exists(),
        godot_runtime_integrity: if runtime_ok { format!("verified · {runtime_detail}") } else { format!("invalid · {runtime_detail}") },
        pipeline_ready,
        latest_update,
        last_validation,
        last_recovery,
        recovery_required,
        recovery_error,
    })
}
