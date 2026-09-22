use crate::core::{error::{BridgeError, BridgeResult}, fsops::copy_project_tree,
    godot, paths, project, runtime, transaction, update, types::ActionResult};
use bridge_safety::Kind;
use std::{fs, path::Path};

fn current_build_revision() -> Option<u64> {
    fs::read_to_string(paths::current_build().ok()?.join("bridge_revision.txt"))
        .ok()?.trim().parse().ok()
}

pub fn initialize() -> BridgeResult<ActionResult> {
    let ws = transaction::open_recovered()?;
    let (runtime_ok, detail) = runtime::integrity()?;
    if !runtime_ok {
        return Ok(ActionResult { ok: false, title: "Godot runtime unavailable".into(), detail, path: None });
    }
    let exists = ws.source().join("project.godot").is_file();
    let meta = if exists { project::read_project(&ws.source())? } else { project::default_metadata() };
    if exists && current_build_revision() == Some(meta.revision) {
        let (ok, detail) = godot::smoke_current_build()?;
        if ok { return Ok(ActionResult { ok: true, title: "Pipeline already initialized".into(), detail, path: None }); }
    }
    transaction::execute(&ws, if exists { Kind::Repair } else { Kind::Initialize }, &meta, None, |stage, _| {
        if exists { copy_project_tree(&ws.source(), stage)?; }
        else { project::bootstrap_demo_at(stage)?; }
        Ok(())
    })?;
    Ok(ActionResult { ok: true, title: "Pipeline initialized".into(),
        detail: format!("Revision {} source and playable Windows build committed together.", meta.revision),
        path: Some(ws.source().to_string_lossy().into()) })
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
