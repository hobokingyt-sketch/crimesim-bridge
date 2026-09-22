use crate::core::{
    error::{BridgeError, BridgeResult},
    fsops::{copy_project_tree, copy_tree_all, sha256_bytes, sha256_file},
    godot, package, paths, project, transaction,
    types::{ActionResult, OperationKind, TransactionPhase, ValidationReport},
};
use std::{fs, path::{Path, PathBuf}};

pub fn save_validation(report: &ValidationReport) -> BridgeResult<()> {
    let path = paths::root()?.join(paths::LAST_VALIDATION);
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    fs::write(path, serde_json::to_vec_pretty(report)?)?;
    Ok(())
}

fn verify_update(zip_path: &Path) -> BridgeResult<(crate::core::types::UpdateManifest, crate::core::types::BridgeProject)> {
    let current_root = paths::current_project()?;
    let current = project::read_project(&current_root)?;
    let manifest = package::read_update_manifest(zip_path)?;
    if manifest.project_id != current.project_id { return Err(BridgeError::Invalid(format!("Update is for project '{}' but current project is '{}'", manifest.project_id, current.project_id))); }
    if manifest.engine_version != current.engine_version { return Err(BridgeError::Invalid(format!("Engine version mismatch: package {} vs project {}", manifest.engine_version, current.engine_version))); }
    if manifest.base_revision != current.revision { return Err(BridgeError::Invalid(format!("Base revision mismatch: package expects {}, current is {}", manifest.base_revision, current.revision))); }
    if manifest.target_revision != current.revision + 1 { return Err(BridgeError::Invalid("Target revision must be exactly current revision + 1".into())); }
    if manifest.operations.is_empty() { return Err(BridgeError::Invalid("Update contains no file operations".into())); }

    for op in &manifest.operations {
        let rel = paths::normalize_relative(&op.path)?;
        let current_file = current_root.join(&rel);
        match op.op {
            OperationKind::Create => {
                if current_file.exists() { return Err(BridgeError::Invalid(format!("Create target already exists: {}", op.path))); }
                let bytes = package::read_change_bytes(zip_path, &op.path)?;
                let expected = op.new_sha256.as_ref().ok_or_else(|| BridgeError::Invalid(format!("Create missing new_sha256: {}", op.path)))?;
                if &sha256_bytes(&bytes) != expected { return Err(BridgeError::Invalid(format!("Payload hash mismatch: {}", op.path))); }
            }
            OperationKind::Replace => {
                if !current_file.is_file() { return Err(BridgeError::Invalid(format!("Replace target missing: {}", op.path))); }
                let expected_base = op.base_sha256.as_ref().ok_or_else(|| BridgeError::Invalid(format!("Replace missing base_sha256: {}", op.path)))?;
                if &sha256_file(&current_file)? != expected_base { return Err(BridgeError::Invalid(format!("Current file drift detected: {}", op.path))); }
                let bytes = package::read_change_bytes(zip_path, &op.path)?;
                let expected_new = op.new_sha256.as_ref().ok_or_else(|| BridgeError::Invalid(format!("Replace missing new_sha256: {}", op.path)))?;
                if &sha256_bytes(&bytes) != expected_new { return Err(BridgeError::Invalid(format!("Replacement payload hash mismatch: {}", op.path))); }
            }
            OperationKind::Delete => {
                if !current_file.is_file() { return Err(BridgeError::Invalid(format!("Delete target missing: {}", op.path))); }
                let expected_base = op.base_sha256.as_ref().ok_or_else(|| BridgeError::Invalid(format!("Delete missing base_sha256: {}", op.path)))?;
                if &sha256_file(&current_file)? != expected_base { return Err(BridgeError::Invalid(format!("Current file drift detected: {}", op.path))); }
            }
        }
    }
    Ok((manifest, current))
}

fn apply_to_stage(zip_path: &Path, stage: &Path, manifest: &crate::core::types::UpdateManifest) -> BridgeResult<()> {
    for op in &manifest.operations {
        let rel = paths::normalize_relative(&op.path)?;
        let target = stage.join(&rel);
        match op.op {
            OperationKind::Create | OperationKind::Replace => {
                let bytes = package::read_change_bytes(zip_path, &op.path)?;
                if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
                fs::write(target, bytes)?;
            }
            OperationKind::Delete => { if target.exists() { fs::remove_file(target)?; } }
        }
    }
    Ok(())
}

fn promote_source(stage: &Path, old_revision: u64) -> BridgeResult<()> {
    let current = paths::current_project()?;
    let history = paths::history_root()?.join(format!("rev_{:04}", old_revision));
    if !history.exists() { copy_project_tree(&current, &history)?; }
    let next = paths::root()?.join("workspace/current_project.__next");
    let old = paths::root()?.join("workspace/current_project.__old");
    if next.exists() { fs::remove_dir_all(&next)?; }
    if old.exists() { fs::remove_dir_all(&old)?; }
    copy_project_tree(stage, &next)?;
    fs::rename(&current, &old)?;
    match fs::rename(&next, &current) {
        Ok(_) => { fs::remove_dir_all(old)?; Ok(()) }
        Err(e) => { let _ = fs::rename(&old, &current); Err(BridgeError::Io(e)) }
    }
}

fn read_build_revision(path: &Path) -> Option<u64> {
    fs::read_to_string(path.join("bridge_revision.txt")).ok()?.trim().parse().ok()
}

pub fn promote_build(candidate: PathBuf, revision: u64) -> BridgeResult<()> {
    let current = paths::current_build()?;
    if let Some(old_revision) = read_build_revision(&current) {
        let snapshot = paths::build_history_root()?.join(format!("rev_{:04}", old_revision));
        if !snapshot.exists() { copy_tree_all(&current, &snapshot)?; }
    }
    let next = paths::builds_root()?.join("current.__next");
    let old = paths::builds_root()?.join("current.__old");
    if next.exists() { fs::remove_dir_all(&next)?; }
    if old.exists() { fs::remove_dir_all(&old)?; }
    copy_tree_all(&candidate, &next)?;
    fs::write(next.join("bridge_revision.txt"), revision.to_string())?;
    if current.exists() { fs::rename(&current, &old)?; }
    match fs::rename(&next, &current) {
        Ok(_) => {
            if old.exists() { fs::remove_dir_all(old)?; }
            if candidate.exists() { fs::remove_dir_all(candidate)?; }
            Ok(())
        }
        Err(e) => {
            if old.exists() { let _ = fs::rename(&old, &current); }
            Err(BridgeError::Io(e))
        }
    }
}

fn apply_inner(zip_path: &Path) -> BridgeResult<ActionResult> {
    paths::ensure_layout()?;
    transaction::recover_incomplete()?;
    let (manifest, current) = verify_update(zip_path)?;
    let stage = paths::staging_root()?.join(format!("rev_{:04}", manifest.target_revision));
    let mut journal = transaction::begin(&current.project_id, current.revision, manifest.target_revision, zip_path, &stage)?;

    copy_project_tree(&paths::current_project()?, &stage)?;
    apply_to_stage(zip_path, &stage, &manifest)?;
    let mut staged_meta = current.clone();
    staged_meta.revision = manifest.target_revision;
    project::write_project(&stage, &staged_meta)?;
    project::ensure_export_preset(&stage, &staged_meta.export_preset)?;
    transaction::set_phase(&mut journal, TransactionPhase::Staged, "Candidate source assembled in staging")?;

    let (report, candidate_build) = godot::validate(&stage, &staged_meta)?;
    save_validation(&report)?;
    if !report.passed {
        let detail = if report.errors.is_empty() { "Staged project failed validation".into() } else { report.errors.join(" | ") };
        if stage.exists() { fs::remove_dir_all(&stage)?; }
        transaction::clear()?;
        let quarantined = package::quarantine_update(zip_path, &detail)?;
        return Ok(ActionResult { ok: false, title: "Update rejected and quarantined".into(), detail, path: Some(quarantined.to_string_lossy().to_string()) });
    }
    let candidate_build = candidate_build.ok_or_else(|| BridgeError::Invalid("Validation passed without producing a playable Windows build".into()))?;
    transaction::set_phase(&mut journal, TransactionPhase::Validated, "All required validation gates passed")?;

    transaction::set_phase(&mut journal, TransactionPhase::SourcePromoting, "Promoting staged source")?;
    promote_source(&stage, current.revision)?;
    transaction::set_phase(&mut journal, TransactionPhase::SourcePromoted, "Source promotion complete")?;

    transaction::set_phase(&mut journal, TransactionPhase::BuildPromoting, "Promoting candidate playable build")?;
    promote_build(candidate_build, staged_meta.revision)?;
    transaction::set_phase(&mut journal, TransactionPhase::Committed, "Source and playable build committed")?;

    let archived_update = package::archive_applied_update(zip_path, staged_meta.revision).ok();
    if stage.exists() { fs::remove_dir_all(stage)?; }
    transaction::clear()?;
    Ok(ActionResult {
        ok: true,
        title: format!("Revision {} applied", staged_meta.revision),
        detail: match archived_update {
            Some(path) => format!("{} Update archived as {}.", manifest.summary, path.file_name().unwrap_or_default().to_string_lossy()),
            None => format!("{} Source/build committed; update archive cleanup will be retried if needed.", manifest.summary),
        },
        path: Some(paths::current_project()?.to_string_lossy().to_string())
    })
}

pub fn apply(zip_path: &Path) -> BridgeResult<ActionResult> {
    match apply_inner(zip_path) {
        Ok(result) => Ok(result),
        Err(err) => {
            let reason = err.to_string();
            let recovery = transaction::recover_incomplete().ok().flatten();
            let quarantine = if zip_path.exists() { package::quarantine_update(zip_path, &reason).ok() } else { None };
            Ok(ActionResult {
                ok: false,
                title: "Update failed safely".into(),
                detail: match recovery { Some(r) => format!("{reason}. {r}"), None => reason },
                path: quarantine.map(|p| p.to_string_lossy().to_string()),
            })
        }
    }
}

pub fn rollback() -> BridgeResult<ActionResult> {
    transaction::recover_incomplete()?;
    let current_root = paths::current_project()?;
    let current = project::read_project(&current_root)?;
    if current.revision == 0 { return Ok(ActionResult { ok: false, title: "Nothing to roll back".into(), detail: "Revision 0 has no earlier source snapshot.".into(), path: None }); }
    let target_revision = current.revision - 1;
    let target = paths::history_root()?.join(format!("rev_{:04}", target_revision));
    if !target.exists() { return Ok(ActionResult { ok: false, title: "Rollback unavailable".into(), detail: format!("No source snapshot exists for revision {target_revision}."), path: None }); }
    let rollback_stage = paths::staging_root()?.join(format!("rollback_{:04}", target_revision));
    copy_project_tree(&target, &rollback_stage)?;
    let target_meta = project::read_project(&rollback_stage)?;
    let (report, candidate) = godot::validate(&rollback_stage, &target_meta)?;
    save_validation(&report)?;
    if !report.passed { return Ok(ActionResult { ok: false, title: "Rollback rejected".into(), detail: report.errors.join(" | "), path: None }); }
    promote_source(&rollback_stage, current.revision)?;
    if let Some(candidate) = candidate { promote_build(candidate, target_revision)?; }
    if rollback_stage.exists() { fs::remove_dir_all(&rollback_stage)?; }
    Ok(ActionResult { ok: true, title: format!("Rolled back to revision {target_revision}"), detail: "Source and playable build were rebuilt from the retained snapshot.".into(), path: None })
}
