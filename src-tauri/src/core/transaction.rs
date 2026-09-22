//! Godot adapter for the shared, independently tested transaction coordinator.
use crate::core::{error::{BridgeError, BridgeResult}, godot, paths, types::BridgeProject, update, worker};
use bridge_safety::{Kind, Recovery, Workspace};
use std::path::Path;

pub fn open_recovered() -> BridgeResult<Workspace> {
    let ws = Workspace::open(&paths::root()?)?;
    worker::ensure_quiescent()?;
    ws.recover().map_err(|e| BridgeError::Invalid(format!("Recovery required; operations blocked: {e}")))?;
    Ok(ws)
}

pub fn recover_incomplete() -> BridgeResult<Option<String>> {
    let ws = Workspace::open(&paths::root()?)?;
    worker::ensure_quiescent()?;
    match ws.recover()? {
        Recovery::Clean => Ok(None),
        outcome => Ok(Some(format!("Recovery completed: {outcome:?}"))),
    }
}

/// All source/build writers enter here after acquiring one workspace lock.
/// The caller assembles source only in the coordinator's private candidate directory.
pub fn execute<F>(ws: &Workspace, kind: Kind, meta: &BridgeProject, input: Option<&Path>, assemble: F) -> BridgeResult<()>
where F: FnOnce(&Path, Option<&Path>) -> BridgeResult<()> {
    let result = (|| -> BridgeResult<()> {
        let tx = ws.begin(kind, &meta.project_id, meta.revision, input)?;
        let stage = tx.source()?;
        let retained_package = tx.package()?;
        assemble(&stage, retained_package.as_deref())?;
        let (report, build) = godot::validate(&stage, meta)?;
        update::save_validation(&report)?;
        if !report.passed {
            return Err(BridgeError::Invalid(format!("Candidate validation failed: {}", report.errors.join(" | "))));
        }
        let build = build.ok_or_else(|| BridgeError::Invalid("No validated playable build produced".into()))?;
        tx.promote(&build, &serde_json::to_value(&report)?)?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(()),
        Err(original) => {
            // Never restore/delete staging while an unconfirmed worker can still write into it.
            worker::ensure_quiescent().map_err(|e| BridgeError::Invalid(format!("RECOVERY REQUIRED. {original}. {e}")))?;
            match ws.recover() {
            // A commit is irrevocable. Retry its cleanup, never quarantine it as a rejected update.
            Ok(Recovery::Finalized) => Ok(()),
            Ok(_) => Err(BridgeError::Invalid(format!("Operation rejected; prior source/build retained. {original}"))),
            Err(recovery) => Err(BridgeError::Invalid(format!(
                "RECOVERY REQUIRED. {original}. Recovery failed: {recovery}. Further operations are blocked; retained files were not discarded."
            ))),
        }
        },
    }
}
