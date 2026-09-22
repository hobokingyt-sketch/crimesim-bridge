use crate::core::{error::{BridgeError, BridgeResult}, fsops::sha256_file, paths};
use serde::Deserialize;
use std::{fs, path::Path, process::Command};
use tauri::Manager;
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
struct RuntimeManifest {
    schema: u32,
    engine_version: String,
    files: Vec<RuntimeFile>,
}

#[derive(Debug, Deserialize)]
struct RuntimeFile {
    path: String,
    sha256: String,
}

fn worker_at(root: &Path) -> Option<std::path::PathBuf> {
    let mut workers = fs::read_dir(root).ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.file_name()
                    .and_then(|v| v.to_str())
                    .map(|name| name.to_ascii_lowercase().ends_with("_console.exe"))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    workers.sort();
    workers.into_iter().next().or_else(|| {
        let legacy = root.join("godot.exe");
        legacy.is_file().then_some(legacy)
    })
}

pub fn integrity_at(root: &Path) -> BridgeResult<(bool, String)> {
    let manifest_path = root.join("runtime_manifest.json");
    if worker_at(root).is_none() {
        return Ok((false, "Godot command-line worker is missing".into()));
    }
    if !manifest_path.exists() {
        return Ok((false, "Runtime integrity manifest is missing".into()));
    }
    let manifest: RuntimeManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    if manifest.schema != 1 {
        return Ok((false, format!("Unsupported runtime manifest schema {}", manifest.schema)));
    }
    for file in &manifest.files {
        let rel = crate::core::paths::normalize_relative(&file.path)?;
        let path = root.join(rel);
        if !path.is_file() {
            return Ok((false, format!("Runtime file missing: {}", file.path)));
        }
        let actual = sha256_file(&path)?;
        if actual != file.sha256 {
            return Ok((false, format!("Runtime hash mismatch: {}", file.path)));
        }
    }
    Ok((true, format!("Godot {} runtime verified ({} files)", manifest.engine_version, manifest.files.len())))
}

pub fn integrity() -> BridgeResult<(bool, String)> {
    integrity_at(&paths::runtime_root()?)
}

pub fn engine_version() -> BridgeResult<String> {
    let godot = paths::runtime_godot()?;
    if !godot.is_file() {
        return Err(BridgeError::Invalid("Godot executable is missing".into()));
    }
    let output = Command::new(&godot).arg("--version").output()?;
    if !output.status.success() {
        return Err(BridgeError::Invalid(format!("Godot --version failed with status {}", output.status)));
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        return Err(BridgeError::Invalid("Godot --version returned no version string".into()));
    }
    Ok(text)
}

pub fn verify_engine_version(expected: &str) -> BridgeResult<(bool, String)> {
    let actual = engine_version()?;
    let ok = actual == expected || actual.starts_with(&format!("{expected}.")) || actual.starts_with(&format!("{expected}-"));
    Ok((ok, if ok {
        format!("Godot executable reports {actual}")
    } else {
        format!("Godot executable reports {actual}; project requires {expected}")
    }))
}

pub fn install_from_dir(source: &Path) -> BridgeResult<()> {
    paths::ensure_layout()?;
    let (source_valid, source_detail) = integrity_at(source)?;
    if !source_valid {
        return Err(BridgeError::Invalid(format!("Godot runtime source failed integrity check: {source_detail}")));
    }
    let target = paths::runtime_root()?;
    if source == target {
        return Ok(());
    }
    let incoming = paths::root()?.join("runtime/godot.__incoming");
    let old = paths::root()?.join("runtime/godot.__old");
    if incoming.exists() { fs::remove_dir_all(&incoming)?; }
    if old.exists() { fs::remove_dir_all(&old)?; }
    copy_tree(source, &incoming)?;
    fs::write(incoming.join("_sc_"), b"")?;
    let (incoming_valid, incoming_detail) = integrity_at(&incoming)?;
    if !incoming_valid {
        let _ = fs::remove_dir_all(&incoming);
        return Err(BridgeError::Invalid(format!("Copied runtime failed integrity check: {incoming_detail}")));
    }
    if target.exists() { fs::rename(&target, &old)?; }
    match fs::rename(&incoming, &target) {
        Ok(_) => {
            if old.exists() { fs::remove_dir_all(old)?; }
            Ok(())
        }
        Err(err) => {
            if old.exists() { let _ = fs::rename(&old, &target); }
            Err(BridgeError::Io(err))
        }
    }
}

pub fn install_bundled(app: &tauri::AppHandle) -> BridgeResult<()> {
    paths::ensure_layout()?;
    let target = paths::runtime_root()?;
    if target.exists() {
        let (valid, _) = integrity_at(&target)?;
        if valid { return Ok(()); }
    }

    if let Some(source_override) = std::env::var_os("CRIMESIM_BRIDGE_RUNTIME_SOURCE") {
        let source = std::path::PathBuf::from(source_override);
        if source.exists() { return install_from_dir(&source); }
    }

    let resources = app.path().resource_dir().map_err(|e| BridgeError::Invalid(e.to_string()))?;
    let candidates = [resources.join("resources/godot"), resources.join("godot")];
    let source = candidates.iter().find(|p| p.join("runtime_manifest.json").exists() && worker_at(p).is_some());
    let Some(source) = source else {
        return Ok(()); // Source/dev build may intentionally omit the large runtime payload.
    };
    install_from_dir(source)
}

fn copy_tree(src: &Path, dst: &Path) -> BridgeResult<()> {
    fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src).follow_links(false) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src).map_err(|e| BridgeError::Invalid(e.to_string()))?;
        if rel.as_os_str().is_empty() { continue; }
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
