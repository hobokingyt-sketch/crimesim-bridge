use crate::core::{
    error::{BridgeError, BridgeResult},
    paths,
    project,
    types::UpdateManifest,
};
use chrono::Utc;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

/// The default path remains available to the installed acceptance probe.
pub fn create_chat_pack() -> BridgeResult<PathBuf> {
    Ok(create_scoped_chat_pack(&crate::core::context::Request::default())?.path)
}

pub fn chat_context_options() -> BridgeResult<crate::core::context::Options> {
    let _workspace_guard = crate::core::transaction::open_recovered()?;
    let root = paths::current_project()?;
    let meta = project::read_project(&root)?;
    crate::core::context::options_at(&root, &meta)
}

pub fn create_scoped_chat_pack(request: &crate::core::context::Request) -> BridgeResult<crate::core::context::Pack> {
    let _workspace_guard = crate::core::transaction::open_recovered()?;
    paths::ensure_layout()?;
    let root = paths::current_project()?;
    let meta = project::read_project(&root)?;
    let output = paths::downloads().unwrap_or(paths::outgoing_root()?);
    let report_path = paths::root()?.join(paths::LAST_VALIDATION);
    let report = fs::metadata(&report_path).ok().filter(|m| m.len() <= 64 * 1024)
        .and_then(|_| fs::read(report_path).ok())
        .and_then(|bytes| serde_json::from_slice::<crate::core::types::ValidationReport>(&bytes).ok());
    crate::core::context::export_at(&root, &meta, &output, request, report.as_ref())
}

pub fn read_update_manifest(zip_path: &Path) -> BridgeResult<UpdateManifest> {
    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut manifest_file = archive
        .by_name("bridge_manifest.json")
        .map_err(|_| BridgeError::Invalid("Update pack is missing bridge_manifest.json".into()))?;
    let mut bytes = Vec::new();
    manifest_file.read_to_end(&mut bytes)?;
    let manifest: UpdateManifest = serde_json::from_slice(&bytes)?;
    if manifest.schema != 1 || manifest.package_type != "update" {
        return Err(BridgeError::Invalid("Unsupported or invalid update package schema".into()));
    }
    Ok(manifest)
}

pub fn latest_update_pack() -> BridgeResult<Option<PathBuf>> {
    let mut candidates = Vec::new();
    let mut dirs = vec![paths::root()?.join("incoming")];
    if let Some(downloads) = paths::downloads() {
        dirs.push(downloads);
    }
    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("CrimeSim_Update_") && name.ends_with(".zip") {
                let modified = entry.metadata()?.modified().ok();
                candidates.push((modified, entry.path()));
            }
        }
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(candidates.pop().map(|(_, p)| p))
}

pub fn read_change_bytes(zip_path: &Path, relative: &str) -> BridgeResult<Vec<u8>> {
    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    let entry_name = format!("changes/{}", relative.replace('\\', "/"));
    let mut entry = archive
        .by_name(&entry_name)
        .map_err(|_| BridgeError::Invalid(format!("Update pack missing payload: {entry_name}")))?;
    if entry.enclosed_name().is_none() {
        return Err(BridgeError::Invalid(format!("Unsafe ZIP entry: {entry_name}")));
    }
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}


pub fn quarantine_update(zip_path: &Path, reason: &str) -> BridgeResult<PathBuf> {
    paths::ensure_layout()?;
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let name = zip_path.file_stem().and_then(|v| v.to_str()).unwrap_or("CrimeSim_Update");
    let target = paths::quarantine_root()?.join(format!("{}_{}.rejected.zip", name, stamp));
    if fs::rename(zip_path, &target).is_err() {
        fs::copy(zip_path, &target)?;
        let _ = fs::remove_file(zip_path);
    }
    fs::write(target.with_extension("reason.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "rejected_at": Utc::now().to_rfc3339(),
        "original": zip_path.to_string_lossy(),
        "reason": reason
    }))?)?;
    Ok(target)
}
