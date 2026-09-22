use crate::core::error::BridgeResult;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

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
    bridge_safety::durable::write(path, bytes)?;
    Ok(())
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> BridgeResult<()> {
    bridge_safety::durable::json(path, value)?;
    Ok(())
}

pub fn copy_project_tree(src: &Path, dst: &Path) -> BridgeResult<()> {
    bridge_safety::copy_tree(src, dst, true)?;
    Ok(())
}

pub fn copy_tree_all(src: &Path, dst: &Path) -> BridgeResult<()> {
    bridge_safety::copy_tree(src, dst, false)?;
    Ok(())
}

pub fn is_generated(rel: &Path) -> bool {
    let s = rel.to_string_lossy().replace('\\', "/").to_lowercase();
    s == ".godot" || s.starts_with(".godot/") || s == ".git" || s.starts_with(".git/")
        || s == "build" || s.starts_with("build/") || s == "builds" || s.starts_with("builds/")
        || s == "logs" || s.starts_with("logs/")
}
