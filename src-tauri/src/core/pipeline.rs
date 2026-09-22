use crate::core::{
    error::{BridgeError, BridgeResult},
    godot, paths, project, runtime, transaction, update,
    types::ActionResult,
};
use std::{fs, path::Path};

fn current_build_revision() -> Option<u64> {
    fs::read_to_string(paths::current_build().ok()?.join("bridge_revision.txt"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

pub fn initialize() -> BridgeResult<ActionResult> {
    paths::ensure_layout()?;
    transaction::recover_incomplete()?;

    let (runtime_ok, runtime_detail) = runtime::integrity()?;
    if !runtime_ok {
        return Ok(ActionResult {
            ok: false,
            title: "Godot runtime unavailable".into(),
            detail: runtime_detail,
            path: None,
        });
    }

    let meta = project::bootstrap_demo()?;
    if current_build_revision() == Some(meta.revision) {
        let (smoke_ok, smoke_detail) = godot::smoke_current_build()?;
        if smoke_ok {
            return Ok(ActionResult {
                ok: true,
                title: "Pipeline already initialized".into(),
                detail: format!("Source and playable build are both revision {}. {smoke_detail}", meta.revision),
                path: Some(paths::current_project()?.to_string_lossy().to_string()),
            });
        }
    }

    let (report, candidate) = godot::validate(&paths::current_project()?, &meta)?;
    update::save_validation(&report)?;
    if !report.passed {
        return Ok(ActionResult {
            ok: false,
            title: "Pipeline initialization failed".into(),
            detail: if report.errors.is_empty() { "Godot validation failed".into() } else { report.errors.join(" | ") },
            path: Some(paths::logs_root()?.to_string_lossy().to_string()),
        });
    }
    let candidate = candidate.ok_or_else(|| BridgeError::Invalid("Initialization validation passed without producing a playable build".into()))?;
    update::promote_build(candidate, meta.revision)?;
    Ok(ActionResult {
        ok: true,
        title: "Pipeline initialized".into(),
        detail: format!("Revision {} source and Windows playable build are ready.", meta.revision),
        path: Some(paths::current_project()?.to_string_lossy().to_string()),
    })
}

pub fn assert_end_to_end(fixture_update: &Path) -> BridgeResult<ActionResult> {
    let init = initialize()?;
    if !init.ok {
        return Err(BridgeError::Invalid(format!("Initialization failed: {}", init.detail)));
    }

    let incoming_dir = paths::root()?.join("incoming");
    fs::create_dir_all(&incoming_dir)?;
    let incoming = incoming_dir.join(
        fixture_update.file_name().ok_or_else(|| BridgeError::Invalid("Fixture update has no file name".into()))?
    );
    fs::copy(fixture_update, &incoming)?;

    let applied = update::apply(&incoming)?;
    if !applied.ok {
        return Err(BridgeError::Invalid(format!("Fixture update failed: {}", applied.detail)));
    }

    let current = project::read_project(&paths::current_project()?)?;
    let playable = current_build_revision();
    if current.revision != 1 || playable != Some(1) {
        return Err(BridgeError::Invalid(format!(
            "End-to-end revision mismatch: source {}, playable {:?}", current.revision, playable
        )));
    }

    let (smoke_ok, smoke_detail) = godot::smoke_current_build()?;
    if !smoke_ok {
        return Err(BridgeError::Invalid(format!("Promoted build smoke failed: {smoke_detail}")));
    }

    Ok(ActionResult {
        ok: true,
        title: "End-to-end pipeline passed".into(),
        detail: "Revision 0 was initialized, revision 1 update was transactionally applied, exported, promoted, and launched headlessly.".into(),
        path: Some(paths::current_build()?.to_string_lossy().to_string()),
    })
}
