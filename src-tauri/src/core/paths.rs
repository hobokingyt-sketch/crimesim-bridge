use crate::core::error::{BridgeError, BridgeResult};
use directories::{ProjectDirs, UserDirs};
use std::{env, fs, path::{Component, Path, PathBuf}};

pub const PROJECT_META: &str = "project_control/bridge_project.json";
pub const LAST_VALIDATION: &str = "state/last_validation.json";
pub const RECOVERY_ERROR: &str = "state/recovery_error.txt";
pub const LAST_RECOVERY: &str = "state/last_recovery.txt";
pub const TRANSACTION_JOURNAL: &str = "state/transaction_journal.json";

pub fn root() -> BridgeResult<PathBuf> {
    if let Some(value) = env::var_os("CRIMESIM_BRIDGE_ROOT") {
        let override_root = PathBuf::from(value);
        if override_root.as_os_str().is_empty() {
            return Err(BridgeError::Invalid("CRIMESIM_BRIDGE_ROOT is empty".into()));
        }
        return Ok(override_root);
    }
    let dirs = ProjectDirs::from("com", "Zeno", "CrimeSimBridge")
        .ok_or_else(|| BridgeError::Invalid("Could not resolve local application data directory".into()))?;
    Ok(dirs.data_local_dir().to_path_buf())
}

pub fn current_project() -> BridgeResult<PathBuf> { Ok(root()?.join("workspace/current_project")) }
pub fn staging_root() -> BridgeResult<PathBuf> { Ok(root()?.join("workspace/staging")) }
pub fn history_root() -> BridgeResult<PathBuf> { Ok(root()?.join("history/source")) }
pub fn build_history_root() -> BridgeResult<PathBuf> { Ok(root()?.join("history/builds")) }
pub fn builds_root() -> BridgeResult<PathBuf> { Ok(root()?.join("builds")) }
pub fn current_build() -> BridgeResult<PathBuf> { Ok(builds_root()?.join("current")) }
pub fn runtime_godot() -> BridgeResult<PathBuf> {
    let runtime = runtime_root()?;
    if runtime.is_dir() {
        let mut workers = fs::read_dir(&runtime)?
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
        if let Some(worker) = workers.into_iter().next() {
            return Ok(worker);
        }
    }
    Ok(runtime.join("godot.exe"))
}
pub fn runtime_root() -> BridgeResult<PathBuf> { Ok(root()?.join("runtime/godot")) }
pub fn logs_root() -> BridgeResult<PathBuf> { Ok(root()?.join("logs")) }
pub fn outgoing_root() -> BridgeResult<PathBuf> { Ok(root()?.join("outgoing")) }
pub fn quarantine_root() -> BridgeResult<PathBuf> { Ok(root()?.join("quarantine")) }
pub fn applied_root() -> BridgeResult<PathBuf> { Ok(root()?.join("applied")) }
pub fn transaction_journal() -> BridgeResult<PathBuf> { Ok(root()?.join(TRANSACTION_JOURNAL)) }

pub fn downloads() -> Option<PathBuf> {
    if let Some(value) = env::var_os("CRIMESIM_BRIDGE_DOWNLOADS") {
        let path = PathBuf::from(value);
        if !path.as_os_str().is_empty() { return Some(path); }
    }
    UserDirs::new().and_then(|d| d.download_dir().map(Path::to_path_buf))
}

pub fn ensure_layout() -> BridgeResult<()> {
    for p in [
        root()?, staging_root()?, history_root()?, build_history_root()?, builds_root()?,
        logs_root()?, outgoing_root()?, quarantine_root()?, applied_root()?, runtime_root()?, root()?.join("state"), root()?.join("incoming"),
    ] { std::fs::create_dir_all(p)?; }
    Ok(())
}

pub fn normalize_relative(input: &str) -> BridgeResult<PathBuf> {
    let path = Path::new(input);
    if path.is_absolute() || input.trim().is_empty() {
        return Err(BridgeError::Invalid(format!("Unsafe or empty path: {input}")));
    }
    let mut clean = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(part) => clean.push(part),
            _ => return Err(BridgeError::Invalid(format!("Unsafe path: {input}"))),
        }
    }
    let lower = clean.to_string_lossy().replace('\\', "/").to_lowercase();
    let forbidden = [".git/", ".godot/", "builds/", "history/", "runtime/", "logs/", "workspace/"];
    if lower == ".git" || lower == ".godot" || forbidden.iter().any(|p| lower.starts_with(p)) {
        return Err(BridgeError::Invalid(format!("Path is Bridge-managed or generated: {input}")));
    }
    if lower == PROJECT_META {
        return Err(BridgeError::Invalid("bridge_project.json is Bridge-owned metadata".into()));
    }
    Ok(clean)
}
