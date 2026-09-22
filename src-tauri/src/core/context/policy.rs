//! Outgoing context policy, not an incoming update ZIP security validator.
use crate::core::error::{BridgeError, BridgeResult};
use std::path::Path;

pub const TEXT_BUDGET: u64 = 384 * 1024;
pub const FILE_LIMIT: u64 = 128 * 1024;
pub const REQUEST_BUDGET: u64 = 8 * 1024 * 1024;
pub const INDEX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const INDEX_FILES: usize = 20_000;
pub const CONTROL_LIMIT: u64 = 64 * 1024;

pub fn invalid(message: impl Into<String>) -> BridgeError { BridgeError::Invalid(message.into()) }

pub fn safe_path(value: &str) -> BridgeResult<()> {
    if value.is_empty() || value.len() > 512 || value.contains(['\\', ':', '\0']) || value.starts_with('/') {
        return Err(invalid(format!("Not a safe game-relative path: {value}")));
    }
    for part in value.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.ends_with(['.', ' '])
            || part.chars().any(|c| c.is_control() || "<>\"|?*".contains(c)) {
            return Err(invalid(format!("Not a canonical game-relative path: {value}")));
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
            || (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4 && stem.as_bytes()[3].is_ascii_digit() {
            return Err(invalid(format!("Reserved Windows filename: {value}")));
        }
    }
    Ok(())
}

pub fn excluded(value: &str) -> bool {
    value.split('/').any(|p| {
        let p = p.to_ascii_lowercase();
        [".git", ".godot", "build", "builds", "target", "node_modules", "logs", "saves", "save", "user_data", "__pycache__", "runtime", ".ssh", ".aws"].contains(&p.as_str())
            || p == ".env" || p.starts_with(".env.") || p.starts_with("id_rsa") || p.starts_with("id_ed25519")
            || ["credentials.json", "credentials.toml", "secrets.json", "secrets.toml"].contains(&p.as_str()) || p.ends_with(".key")
            || p.ends_with(".p12") || p.ends_with(".pfx") || p.ends_with(".pem")
    })
}

pub fn text_file(path: &str) -> bool {
    let ext = Path::new(path).extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
    ["gd", "tscn", "tres", "cfg", "json", "md", "txt", "csv", "svg", "shader", "gdshader", "uid", "import", "godot", "gdextension"].contains(&ext.as_str())
}

pub fn linked(path: &Path) -> BridgeResult<bool> {
    let metadata = std::fs::symlink_metadata(path)?;
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 { return Ok(true); }
    }
    Ok(metadata.file_type().is_symlink())
}
