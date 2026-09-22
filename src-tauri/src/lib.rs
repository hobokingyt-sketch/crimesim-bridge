pub mod core;

use core::{package, pipeline, play, runtime, status, transaction, types::{ActionResult, BridgeStatus}, update};

fn user_error<E: std::fmt::Display>(e: E) -> String { e.to_string() }

#[tauri::command]
fn get_status() -> Result<BridgeStatus, String> { status::get().map_err(user_error) }

#[tauri::command]
fn initialize_pipeline() -> Result<ActionResult, String> {
    pipeline::initialize().map_err(user_error)
}

#[tauri::command]
fn bootstrap_demo_project() -> Result<ActionResult, String> {
    pipeline::initialize().map_err(user_error)
}

#[tauri::command]
fn create_chat_pack() -> Result<ActionResult, String> {
    let path = package::create_chat_pack().map_err(user_error)?;
    Ok(ActionResult {
        ok: true,
        title: "Chat Context Pack created".into(),
        detail: "Upload this one ZIP to ChatGPT. It represents the normal multi-file project without flattening it.".into(),
        path: Some(path.to_string_lossy().to_string()),
    })
}

#[tauri::command]
fn scan_latest_update() -> Result<ActionResult, String> {
    let latest = package::latest_update_pack().map_err(user_error)?;
    Ok(match latest {
        Some(path) => ActionResult {
            ok: true,
            title: "Update pack found".into(),
            detail: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            path: Some(path.to_string_lossy().to_string()),
        },
        None => ActionResult {
            ok: false,
            title: "No update pack found".into(),
            detail: "Download a CrimeSim_Update_####.zip into Downloads, then scan again.".into(),
            path: None,
        },
    })
}

#[tauri::command]
fn apply_latest_update() -> Result<ActionResult, String> {
    let latest = package::latest_update_pack().map_err(user_error)?
        .ok_or_else(|| "No CrimeSim update pack found in Downloads".to_string())?;
    update::apply(&latest).map_err(user_error)
}

#[tauri::command]
fn apply_latest_update_and_play() -> Result<ActionResult, String> {
    let latest = package::latest_update_pack().map_err(user_error)?
        .ok_or_else(|| "No CrimeSim update pack found in Downloads".to_string())?;
    let applied = update::apply(&latest).map_err(user_error)?;
    if !applied.ok { return Ok(applied); }
    let launched = play::current().map_err(user_error)?;
    if !launched.ok { return Ok(launched); }
    Ok(ActionResult {
        ok: true,
        title: applied.title,
        detail: format!("{} The promoted playable build was launched.", applied.detail),
        path: launched.path,
    })
}

#[tauri::command]
fn rollback() -> Result<ActionResult, String> { update::rollback().map_err(user_error) }

#[tauri::command]
fn play_current() -> Result<ActionResult, String> { play::current().map_err(user_error) }

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if let Err(err) = runtime::install_bundled(app.handle()) {
                eprintln!("Bundled Godot runtime install failed: {err}");
            }
            match transaction::recover_incomplete() {
                Ok(Some(msg)) => eprintln!("Bridge crash recovery: {msg}"),
                Ok(None) => {},
                Err(err) => eprintln!("Bridge crash recovery failed: {err}"),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            initialize_pipeline,
            bootstrap_demo_project,
            create_chat_pack,
            scan_latest_update,
            apply_latest_update,
            apply_latest_update_and_play,
            rollback,
            play_current
        ])
        .run(tauri::generate_context!())
        .expect("error while running CrimeSim Bridge");
}
