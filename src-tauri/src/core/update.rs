use crate::core::{
    error::{BridgeError, BridgeResult},
    fsops::{copy_project_tree, sha256_bytes, sha256_file},
    package, paths, project, transaction,
    types::{ActionResult, OperationKind, ValidationReport},
};
use bridge_safety::Kind;
use std::{fs, path::Path};

pub fn save_validation(report: &ValidationReport) -> BridgeResult<()> {
    crate::core::fsops::atomic_write_json(&paths::root()?.join(paths::LAST_VALIDATION), report)
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

pub fn apply(zip_path: &Path) -> BridgeResult<ActionResult> {
    let ws = transaction::open_recovered()?;
    let original_hash = sha256_file(zip_path)?;
    let (manifest, current) = match verify_update(zip_path) {
        Ok(value) => value,
        Err(error) => {
            // Preserve terminal-package behavior even when rejection precedes a transaction.
            // Never archive different bytes downloaded to this filename during verification.
            if sha256_file(zip_path)? != original_hash {
                return Err(BridgeError::Invalid("Input changed during verification; no package was moved".into()));
            }
            let rejected = package::quarantine_update(zip_path, &error.to_string())?;
            return Ok(ActionResult { ok: false, title: "Update rejected before staging".into(),
                detail: error.to_string(), path: Some(rejected.to_string_lossy().into()) });
        }
    };
    let mut target = current.clone();
    target.revision = manifest.target_revision;
    let outcome = transaction::execute(&ws, Kind::Update, &target, Some(zip_path), |stage, retained| {
        let retained = retained.ok_or_else(|| BridgeError::Invalid("Retained update missing".into()))?;
        // Revalidate the immutable captured pack. Never reread mutable Downloads payloads during apply.
        let (captured, _) = verify_update(retained)?;
        if sha256_file(retained)? != original_hash {
            return Err(BridgeError::Invalid("Update changed while being captured".into()));
        }
        copy_project_tree(&ws.source(), stage)?;
        apply_to_stage(retained, stage, &captured)?;
        project::write_project(stage, &target)?;
        Ok(())
    });
    match outcome {
        Ok(()) => Ok(ActionResult { ok: true, title: format!("Revision {} applied", target.revision),
            detail: manifest.summary, path: Some(ws.source().to_string_lossy().into()) }),
        Err(err) => {
            if ws.journal()?.is_some() { return Err(err); }
            let quarantined = if zip_path.is_file() && sha256_file(zip_path)? == original_hash {
                Some(package::quarantine_update(zip_path, &err.to_string())?)
            } else { None };
            Ok(ActionResult { ok: false, title: "Update rejected; prior project retained".into(),
                detail: err.to_string(), path: quarantined.map(|p| p.to_string_lossy().into()) })
        }
    }
}

pub fn rollback() -> BridgeResult<ActionResult> {
    let ws = transaction::open_recovered()?;
    let current = project::read_project(&ws.source())?;
    if current.revision == 0 {
        return Ok(ActionResult { ok: false, title: "Nothing to roll back".into(),
            detail: "Revision 0 has no earlier source snapshot.".into(), path: None });
    }
    let target = match ws.rollback_source()? {
        Some(p) => p,
        // Read-only compatibility for pre-Armor history. New history uses transaction identities.
        None if ws.receipt()?.is_none() => paths::history_root()?.join(format!("rev_{:04}", current.revision - 1)),
        None => return Err(BridgeError::Invalid("No retained before-image is available".into())),
    };
    let meta = project::read_project(&target)?;
    if meta.project_id != current.project_id || meta.engine_version != current.engine_version {
        return Err(BridgeError::Invalid("Rollback snapshot identity or engine differs".into()));
    }
    transaction::execute(&ws, Kind::Rollback, &meta, None, |stage, _| {
        copy_project_tree(&target, stage)
    })?;
    Ok(ActionResult { ok: true, title: format!("Rolled back to revision {}", meta.revision),
        detail: "Source and playable build restored through the same journaled validation process.".into(), path: None })
}
