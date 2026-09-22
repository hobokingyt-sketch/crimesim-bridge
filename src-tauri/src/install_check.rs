//! Opt-in acceptance probe in the actual installed application, never the default startup.
//! Only a newly created, explicitly supplied directory may hold its disposable project.
use crate::core::{package, paths, pipeline, runtime, status, update};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, sync::{OnceLock, atomic::{AtomicBool, Ordering}}};
use tauri::Manager;

struct Check { root: PathBuf, started: AtomicBool }
static CHECK: OnceLock<Check> = OnceLock::new();

pub fn configure() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !args.iter().any(|a| a == "--bridge-install-check") { return Ok(()); }
    if args.len() != 2 || args[0] != "--bridge-install-check" {
        return Err("Install check requires exactly --bridge-install-check <new absolute directory>".into());
    }
    for key in ["CRIMESIM_BRIDGE_ROOT", "CRIMESIM_BRIDGE_DOWNLOADS", "CRIMESIM_BRIDGE_RUNTIME_SOURCE"] {
        if std::env::var_os(key).is_some() {
            return Err(format!("Install check refuses an existing {key} override"));
        }
    }
    let root = PathBuf::from(&args[1]);
    if !root.is_absolute() || root.exists() {
        return Err("Install check requires a new absolute directory; existing data is never reset".into());
    }
    fs::create_dir(&root).map_err(|e| e.to_string())?;
    // Before Tauri starts threads. These overrides never target the normal user workspace.
    std::env::set_var("CRIMESIM_BRIDGE_ROOT", &root);
    std::env::set_var("CRIMESIM_BRIDGE_DOWNLOADS", root.join("downloads"));
    CHECK.set(Check { root, started: AtomicBool::new(false) }).map_err(|_| "Install check configured twice")?;
    Ok(())
}

fn probe(app: &tauri::AppHandle, controls_ready: bool) -> Result<Value, String> {
    let root = &CHECK.get().ok_or("Install check not configured")?.root;
    if !controls_ready { return Err("Installed frontend controls did not render".into()); }
    let resources = app.path().resource_dir().map_err(|e| e.to_string())?;
    let build: Value = serde_json::from_slice(&fs::read(resources.join("build_info.json")).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let commit = build["source_commit"].as_str().ok_or("Build has no source commit")?;
    if commit.len() != 40 || !commit.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Install check requires packaged commit provenance".into());
    }
    let (valid, detail) = runtime::integrity().map_err(|e| e.to_string())?;
    if !valid { return Err(format!("Installed Godot runtime is invalid: {detail}")); }
    let canary = root.join("saves/install-canary.txt");
    fs::create_dir_all(canary.parent().ok_or("Canary path invalid")?).map_err(|e| e.to_string())?;
    fs::write(&canary, b"preserve-this-user-data").map_err(|e| e.to_string())?;
    let fixture = root.join("fixture/CrimeSim_Update_0001.zip");
    fs::create_dir_all(fixture.parent().ok_or("Fixture path invalid")?).map_err(|e| e.to_string())?;
    fs::write(&fixture, include_bytes!("../../fixtures/CrimeSim_Update_0001.zip")).map_err(|e| e.to_string())?;
    pipeline::assert_end_to_end(&fixture).map_err(|e| e.to_string())?;
    let rollback = update::rollback().map_err(|e| e.to_string())?;
    if !rollback.ok { return Err(rollback.detail); }
    let incoming = paths::root().map_err(|e| e.to_string())?.join("incoming/CrimeSim_Update_0001.zip");
    fs::copy(&fixture, &incoming).map_err(|e| e.to_string())?;
    let applied = update::apply(&incoming).map_err(|e| e.to_string())?;
    if !applied.ok { return Err(applied.detail); }
    let request = crate::core::context::Request { module_id: "ui".into(), task: "Installed scoped game handoff acceptance".into(), ..Default::default() };
    let pack = package::create_scoped_chat_pack(&request).map_err(|e| e.to_string())?;
    let game_meta = crate::core::project::read_project(&paths::current_project().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    crate::core::context::verify_generated(&pack, &game_meta, "ui").map_err(|e| e.to_string())?;
    let context = pack.path;
    let current = status::get().map_err(|e| e.to_string())?;
    if !current.pipeline_ready || current.source_revision != Some(1) || current.playable_revision != Some(1) {
        return Err("Installed pipeline source/build state did not converge at revision 1".into());
    }
    if fs::read(&canary).map_err(|e| e.to_string())? != b"preserve-this-user-data" {
        return Err("Install-check save canary changed".into());
    }
    Ok(json!({"schema": 1, "ok": true, "frontend_ready": true, "runtime_verified": true,
        "bridge_version": env!("CARGO_PKG_VERSION"), "source_commit": commit,
        "executable": std::env::current_exe().map_err(|e| e.to_string())?, "resources": resources,
        "workspace": root, "context_pack": context, "save_canary": canary,
        "context_scope": "ui", "context_payload_hashes_verified": true,
        "source_revision": 1, "playable_revision": 1,
        "checks": ["native_frontend_ipc", "bundled_runtime", "initialize", "update", "exported_launch", "rollback", "reapply", "context_pack", "save_canary", "scoped_game_handoff"]}))
}

#[tauri::command]
pub fn frontend_ready(app: tauri::AppHandle, controls_ready: bool) {
    let Some(check) = CHECK.get() else { return; };
    if check.started.swap(true, Ordering::SeqCst) { return; }
    tauri::async_runtime::spawn_blocking(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| probe(&app, controls_ready)));
        let report = match result {
            Ok(Ok(report)) => report,
            Ok(Err(detail)) => json!({"schema": 1, "ok": false, "detail": detail}),
            Err(_) => json!({"schema": 1, "ok": false, "detail": "Installed acceptance probe panicked"}),
        };
        let passed = report["ok"] == true;
        let path = CHECK.get().expect("configured above").root.join("install_check.json");
        let saved = crate::core::fsops::atomic_write_json(&path, &report).is_ok();
        app.exit(if passed && saved { 0 } else { 1 });
    });
}
