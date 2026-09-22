use crate::core::{
    error::{BridgeError, BridgeResult},
    fsops::{is_generated, sha256_file},
    paths,
    project,
    types::{AssetCatalogEntry, UpdateManifest},
};
use chrono::Utc;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "wav", "ogg", "mp3", "glb", "gltf", "fbx", "blend", "ttf", "otf",
    "zip", "7z", "rar", "exe", "dll", "pck",
];
const LARGE_ASSET_LIMIT: u64 = 2 * 1024 * 1024;

pub fn create_chat_pack() -> BridgeResult<PathBuf> {
    paths::ensure_layout()?;
    let project_root = paths::current_project()?;
    let meta = project::read_project(&project_root)?;
    let output_dir = paths::downloads().unwrap_or(paths::outgoing_root()?);
    fs::create_dir_all(&output_dir)?;
    let output = output_dir.join(format!("CrimeSim_Context_{:04}.zip", meta.revision));

    let file = fs::File::create(&output)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut catalog = Vec::new();

    for entry in WalkDir::new(&project_root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&project_root)
            .map_err(|e| BridgeError::Invalid(e.to_string()))?;
        if is_generated(rel) {
            continue;
        }

        let metadata = entry.metadata().map_err(|e| BridgeError::Invalid(e.to_string()))?;
        let hash = sha256_file(entry.path())?;
        let ext = rel
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_binary = BINARY_EXTENSIONS.contains(&ext.as_str());
        let include = !is_binary || metadata.len() <= LARGE_ASSET_LIMIT;
        catalog.push(AssetCatalogEntry {
            path: rel.to_string_lossy().replace('\\', "/"),
            bytes: metadata.len(),
            sha256: hash,
            included: include,
            reason: (!include).then(|| "large binary represented by catalog only".into()),
        });

        if include {
            let zip_name = format!("source/{}", rel.to_string_lossy().replace('\\', "/"));
            zip.start_file(zip_name, options)?;
            let mut input = fs::File::open(entry.path())?;
            std::io::copy(&mut input, &mut zip)?;
        }
    }

    zip.start_file("asset_catalog.json", options)?;
    zip.write_all(&serde_json::to_vec_pretty(&catalog)?)?;
    let last_validation = paths::root()?.join(paths::LAST_VALIDATION);
    if last_validation.exists() {
        zip.start_file("bridge_context/last_validation.json", options)?;
        zip.write_all(&fs::read(last_validation)?)?;
    }
    zip.start_file("bridge_context/bridge_state.json", options)?;
    zip.write_all(&serde_json::to_vec_pretty(&serde_json::json!({
        "bridge_version": env!("CARGO_PKG_VERSION"),
        "source_revision": meta.revision,
        "save_schema": meta.save_schema,
        "generated_at": Utc::now().to_rfc3339()
    }))?)?;
    zip.start_file("context_manifest.json", options)?;
    zip.write_all(
        &serde_json::to_vec_pretty(&serde_json::json!({
            "schema": 1,
            "package_type": "context",
            "project_id": meta.project_id,
            "project_name": meta.project_name,
            "engine": meta.engine,
            "engine_version": meta.engine_version,
            "revision": meta.revision,
            "save_schema": meta.save_schema,
            "created_at": Utc::now().to_rfc3339(),
            "transport_note": "Single handoff archive containing a normal multi-file project. Large binaries may be catalog-only."
        }))?,
    )?;
    zip.finish()?;
    Ok(output)
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


pub fn archive_applied_update(zip_path: &Path, revision: u64) -> BridgeResult<PathBuf> {
    paths::ensure_layout()?;
    let name = zip_path.file_name().and_then(|v| v.to_str()).unwrap_or("CrimeSim_Update.zip");
    let target = paths::applied_root()?.join(format!("rev_{:04}_{}", revision, name));
    if target.exists() { fs::remove_file(&target)?; }
    if fs::rename(zip_path, &target).is_err() {
        fs::copy(zip_path, &target)?;
        let _ = fs::remove_file(zip_path);
    }
    Ok(target)
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
