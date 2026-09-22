use crate::core::error::{BridgeError, BridgeResult};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, io::{Read, Write}, path::Path};
use walkdir::WalkDir;

pub fn sha256_file(path: &Path) -> BridgeResult<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 128 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> BridgeResult<()> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let tmp = path.with_extension(format!("{}.tmp", path.extension().and_then(|v| v.to_str()).unwrap_or("bridge")));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    if path.exists() { fs::remove_file(path)?; }
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> BridgeResult<()> {
    atomic_write(path, &serde_json::to_vec_pretty(value)?)
}

pub fn copy_project_tree(src: &Path, dst: &Path) -> BridgeResult<()> {
    copy_tree(src, dst, true)
}

pub fn copy_tree_all(src: &Path, dst: &Path) -> BridgeResult<()> {
    copy_tree(src, dst, false)
}

fn copy_tree(src: &Path, dst: &Path, skip_generated: bool) -> BridgeResult<()> {
    if dst.exists() { fs::remove_dir_all(dst)?; }
    fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src).follow_links(false) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src).map_err(|e| BridgeError::Invalid(e.to_string()))?;
        if rel.as_os_str().is_empty() || (skip_generated && is_generated(rel)) { continue; }
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn is_generated(rel: &Path) -> bool {
    let s = rel.to_string_lossy().replace('\\', "/").to_lowercase();
    s == ".godot" || s.starts_with(".godot/") || s == ".git" || s.starts_with(".git/")
        || s == "build" || s.starts_with("build/") || s == "builds" || s.starts_with("builds/")
        || s == "logs" || s.starts_with("logs/")
}
