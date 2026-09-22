use crate::{durable, invalid, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Fingerprint { pub present: bool, pub sha256: String }

pub(crate) fn no_links(path: &Path) -> Result<()> {
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(m) => {
                #[cfg(windows)] {
                    use std::os::windows::fs::MetadataExt;
                    if m.file_attributes() & 0x400 != 0 { return Err(invalid(format!("Reparse path refused: {}", p.display()))); }
                }
                if m.file_type().is_symlink() { return Err(invalid(format!("Link path refused: {}", p.display()))); }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn excluded(rel: &Path, source: bool) -> bool {
    source && rel.components().next().map(|c| matches!(c.as_os_str().to_string_lossy().to_ascii_lowercase().as_str(),
        ".godot" | ".git" | "build" | "builds" | "logs")).unwrap_or(false)
}

fn entries(root: &Path, dir: &Path, source: bool, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut children = fs::read_dir(dir)?.map(|e| e.map(|e| e.path())).collect::<Result<Vec<_>>>()?;
    children.sort();
    for path in children {
        let rel = path.strip_prefix(root).map_err(|e| invalid(e.to_string()))?;
        if excluded(rel, source) { continue; }
        no_links(&path)?;
        let m = fs::symlink_metadata(&path)?;
        if !m.is_file() && !m.is_dir() { return Err(invalid("Non-regular snapshot entry")); }
        out.push(path.clone());
        if m.is_dir() { entries(root, &path, source, out)?; }
    }
    Ok(())
}

pub fn fingerprint(root: &Path, source: bool) -> Result<Fingerprint> {
    no_links(root)?;
    match fs::metadata(root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Fingerprint { present: false, sha256: String::new() }),
        Err(e) => return Err(e),
        Ok(m) if !m.is_dir() => return Err(invalid(format!("Expected directory: {}", root.display()))),
        _ => {},
    }
    let mut list = Vec::new(); entries(root, root, source, &mut list)?;
    let mut hash = Sha256::new(); hash.update(b"bridge-tree-v1\0");
    for p in list {
        let rel = p.strip_prefix(root).map_err(|e| invalid(e.to_string()))?.to_str()
            .ok_or_else(|| invalid("Snapshot path is not UTF-8"))?.replace('\\', "/");
        let m = fs::metadata(&p)?;
        hash.update(if m.is_dir() { b"D" } else { b"F" });
        hash.update((rel.len() as u64).to_le_bytes()); hash.update(rel.as_bytes());
        if m.is_file() {
            hash.update(m.len().to_le_bytes());
            let mut f = fs::File::open(p)?; let mut buf = [0u8; 65536];
            loop { let n = f.read(&mut buf)?; if n == 0 { break; } hash.update(&buf[..n]); }
        }
    }
    Ok(Fingerprint { present: true, sha256: format!("{:x}", hash.finalize()) })
}

pub(crate) fn file_hash(path: &Path) -> Result<String> {
    no_links(path)?;
    let mut f = fs::File::open(path)?; let mut h = Sha256::new(); let mut buf = [0u8; 65536];
    loop { let n = f.read(&mut buf)?; if n == 0 { break; } h.update(&buf[..n]); }
    Ok(format!("{:x}", h.finalize()))
}

pub(crate) fn remove(path: &Path) -> Result<()> {
    no_links(path)?;
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => fs::remove_file(path)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    }
    if let Some(p) = path.parent() { durable::sync_dir(p)?; }
    Ok(())
}

pub fn copy_tree(src: &Path, dst: &Path, source: bool) -> Result<()> {
    no_links(src)?; no_links(dst)?;
    if src == dst || dst.starts_with(src) || src.starts_with(dst) { return Err(invalid("Overlapping copy roots")); }
    if !src.is_dir() { return Err(invalid("Snapshot source missing")); }
    remove(dst)?; fs::create_dir_all(dst)?;
    let mut list = Vec::new(); entries(src, src, source, &mut list)?;
    for p in &list {
        let rel = p.strip_prefix(src).map_err(|e| invalid(e.to_string()))?;
        let target = dst.join(rel);
        if p.is_dir() { fs::create_dir_all(target)?; }
        else {
            if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
            fs::copy(p, &target)?;
            fs::OpenOptions::new().write(true).open(target)?.sync_all()?;
        }
    }
    for p in list.iter().rev().filter(|p| p.is_dir()) {
        durable::sync_dir(&dst.join(p.strip_prefix(src).map_err(|e| invalid(e.to_string()))?))?;
    }
    durable::sync_dir(dst)?;
    Ok(())
}
