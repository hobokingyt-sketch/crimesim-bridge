//! Single gateway for short-lived Godot work. Ordinary Play is deliberately separate.
use crate::core::{error::{BridgeError, BridgeResult}, fsops::atomic_write_json, paths};
use bridge_worker::{Outcome, Request};
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, fs, path::Path, sync::atomic::{AtomicBool, AtomicU64, Ordering}, time::{Duration, SystemTime, UNIX_EPOCH}};

pub const PENDING_WORKER: &str = "state/worker_guard.json";
static NEXT_ID: AtomicU64 = AtomicU64::new(0);
#[derive(Debug, Clone, Copy)]
pub enum Phase { Version, Import, Harness, Export, Smoke }
impl Phase {
    fn timeout(self) -> Duration { Duration::from_secs(match self {
        Self::Version => 15, Self::Import => 180, Self::Harness => 60, Self::Export => 300, Self::Smoke => 30,
    }) }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Guard { schema: u32, job_name: String, phase: String }

/// Must run while holding the workspace lock, before any source/build recovery or mutation.
/// Persistent named jobs avoid killing an unrelated process after PID reuse.
pub fn ensure_quiescent() -> BridgeResult<()> {
    let path = paths::root()?.join(PENDING_WORKER);
    if !path.exists() { return Ok(()); }
    if fs::metadata(&path)?.len() > 4096 { return Err(BridgeError::Invalid("Worker guard is oversized; retained for inspection".into())); }
    let guard: Guard = serde_json::from_slice(&fs::read(&path)?)?;
    if guard.schema != 1 { return Err(BridgeError::Invalid("Unknown worker guard schema; operations blocked".into())); }
    bridge_worker::recover_owned_job(&guard.job_name, Duration::from_secs(5))
        .map_err(|e| BridgeError::Invalid(format!("Worker cleanup required before project recovery: {e}")))?;
    fs::remove_file(path)?;
    Ok(())
}

pub fn run(phase: Phase, executable: &Path, cwd: &Path, args: &[&str], log_name: &str) -> BridgeResult<Outcome> {
    ensure_quiescent()?;
    if log_name.contains(['/', '\\']) { return Err(BridgeError::Invalid("Worker log must be a fixed filename".into())); }
    let name = format!("Local\\CrimeSimBridgeWorker_{}_{}_{}", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed));
    let guard_path = paths::root()?.join(PENDING_WORKER);
    let guard = Guard { schema: 1, job_name: name.clone(), phase: format!("{phase:?}") };
    // Written before spawn. If Bridge dies, Windows closes the non-inherited job handle;
    // the next Bridge can confirm that named job is gone/empty before touching staging.
    atomic_write_json(&guard_path, &guard)?;
    fs::create_dir_all(paths::logs_root()?)?;
    let log_path = paths::logs_root()?.join(log_name);
    fs::write(&log_path, format!("Worker {phase:?} starting; deadline={}s. Result pending.\n", phase.timeout().as_secs()))?;
    let request = Request { executable: executable.to_path_buf(), cwd: cwd.to_path_buf(),
        args: args.iter().map(|s| OsString::from(*s)).collect(), timeout: phase.timeout(),
        cleanup_timeout: Duration::from_secs(5), capture_limit: 256 * 1024, job_name: name };
    match bridge_worker::run(&request, &AtomicBool::new(false)) {
        Ok(outcome) => {
            // Clear only after observed tree shutdown. If persistence fails, retain the guard.
            fs::write(&log_path, outcome.log())?;
            if !outcome.cleanup_confirmed {
                return Err(BridgeError::Invalid(format!("WORKER CLEANUP REQUIRED. {}", outcome.summary())));
            }
            fs::remove_file(&guard_path)?;
            Ok(outcome)
        },
        Err(error) => {
            fs::write(&log_path, format!("Worker could not start: {error}\n"))?;
            // Atomic Windows job attachment guarantees no unowned runnable child on spawn error.
            ensure_quiescent()?;
            Err(BridgeError::Invalid(format!("Worker {phase:?} could not start: {error}")))
        },
    }
}
