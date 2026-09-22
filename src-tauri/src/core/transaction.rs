use crate::core::{
    error::{BridgeError, BridgeResult},
    fsops::{atomic_write, atomic_write_json, copy_project_tree, copy_tree_all},
    package, paths, project,
    types::{TransactionJournal, TransactionPhase},
};
use chrono::Utc;
use std::{fs, path::Path};

pub fn begin(project_id: &str, base_revision: u64, target_revision: u64, update_path: &Path, stage_path: &Path) -> BridgeResult<TransactionJournal> {
    let now = Utc::now().to_rfc3339();
    let journal = TransactionJournal {
        schema: 1,
        transaction_id: format!("{}-{}-{}", base_revision, target_revision, Utc::now().timestamp_millis()),
        project_id: project_id.into(),
        base_revision,
        target_revision,
        update_path: update_path.to_string_lossy().to_string(),
        stage_path: stage_path.to_string_lossy().to_string(),
        phase: TransactionPhase::Verified,
        started_at: now.clone(),
        updated_at: now,
        detail: "Package verified; live source unchanged".into(),
    };
    write(&journal)?;
    Ok(journal)
}

pub fn set_phase(journal: &mut TransactionJournal, phase: TransactionPhase, detail: impl Into<String>) -> BridgeResult<()> {
    journal.phase = phase;
    journal.updated_at = Utc::now().to_rfc3339();
    journal.detail = detail.into();
    write(journal)
}

fn write(journal: &TransactionJournal) -> BridgeResult<()> {
    atomic_write_json(&paths::transaction_journal()?, journal)
}

pub fn clear() -> BridgeResult<()> {
    let path = paths::transaction_journal()?;
    if path.exists() { fs::remove_file(path)?; }
    Ok(())
}

pub fn read() -> BridgeResult<Option<TransactionJournal>> {
    let path = paths::transaction_journal()?;
    if !path.exists() { return Ok(None); }
    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}

fn read_build_revision(path: &Path) -> Option<u64> {
    fs::read_to_string(path.join("bridge_revision.txt")).ok()?.trim().parse().ok()
}

fn restore_source(base_revision: u64, target_revision: u64) -> BridgeResult<()> {
    let current = paths::current_project()?;
    let current_revision = project::read_project(&current).ok().map(|m| m.revision);
    if current_revision == Some(base_revision) { return Ok(()); }
    if current_revision != Some(target_revision) && current.exists() {
        return Err(BridgeError::Invalid(format!("Crash recovery found unexpected live source revision {:?}", current_revision)));
    }
    let snapshot = paths::history_root()?.join(format!("rev_{:04}", base_revision));
    if !snapshot.exists() {
        return Err(BridgeError::Invalid(format!("Crash recovery cannot find source snapshot revision {base_revision}")));
    }
    let restore = paths::root()?.join("workspace/current_project.__recover");
    copy_project_tree(&snapshot, &restore)?;
    if current.exists() { fs::remove_dir_all(&current)?; }
    fs::rename(restore, current)?;
    Ok(())
}

fn restore_build(base_revision: u64, target_revision: u64) -> BridgeResult<()> {
    let current = paths::current_build()?;
    let snapshot = paths::build_history_root()?.join(format!("rev_{:04}", base_revision));
    match read_build_revision(&current) {
        Some(r) if r == base_revision => return Ok(()),
        Some(r) if r == target_revision => {},
        None if snapshot.exists() => {},
        None => return Ok(()),
        Some(other) => return Err(BridgeError::Invalid(format!("Crash recovery found unexpected playable revision {other}"))),
    }
    if snapshot.exists() {
        copy_tree_all(&snapshot, &current)?;
    } else if current.exists() {
        fs::remove_dir_all(&current)?;
        fs::create_dir_all(&current)?;
    }
    Ok(())
}

fn cleanup_remnants(journal: &TransactionJournal) -> BridgeResult<()> {
    for path in [
        Path::new(&journal.stage_path).to_path_buf(),
        paths::root()?.join("workspace/current_project.__next"),
        paths::root()?.join("workspace/current_project.__old"),
        paths::root()?.join("workspace/current_project.__recover"),
        paths::builds_root()?.join("candidate").join(format!("rev_{:04}", journal.target_revision)),
        paths::builds_root()?.join("current.__next"),
        paths::builds_root()?.join("current.__old"),
    ] {
        if path.exists() { fs::remove_dir_all(path)?; }
    }
    Ok(())
}

pub fn recover_incomplete() -> BridgeResult<Option<String>> {
    paths::ensure_layout()?;
    let Some(journal) = read()? else { return Ok(None); };
    if journal.phase == TransactionPhase::Committed {
        let update_path = Path::new(&journal.update_path);
        if update_path.exists() {
            let _ = package::archive_applied_update(update_path, journal.target_revision);
        }
        cleanup_remnants(&journal)?;
        clear()?;
        let msg = format!("Finalized committed revision {} after interrupted cleanup.", journal.target_revision);
        atomic_write(&paths::root()?.join(paths::LAST_RECOVERY), msg.as_bytes())?;
        return Ok(Some(msg));
    }

    restore_source(journal.base_revision, journal.target_revision)?;
    restore_build(journal.base_revision, journal.target_revision)?;
    cleanup_remnants(&journal)?;
    clear()?;
    let msg = format!(
        "Recovered interrupted transaction {} at phase {:?}; restored revision {}.",
        journal.transaction_id, journal.phase, journal.base_revision
    );
    atomic_write(&paths::root()?.join(paths::LAST_RECOVERY), msg.as_bytes())?;
    Ok(Some(msg))
}
