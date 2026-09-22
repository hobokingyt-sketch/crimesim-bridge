pub mod core;
mod install_check;

use core::{package, pipeline, play, runtime, status, transaction, types::{ActionResult, BridgeStatus}, update};

fn user_error<E: std::fmt::Display>(e: E) -> String { e.to_string() }

async fn background<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|e| format!("Bridge task failed: {e}"))?
}


#[tauri::command]
async fn get_status() -> Result<BridgeStatus, String> {
    background(|| status::get().map_err(user_error)).await
}

#[tauri::command]
async fn initialize_pipeline() -> Result<ActionResult, String> {
    background(|| pipeline::initialize().map_err(user_error)).await
}

#[tauri::command]
async fn bootstrap_demo_project() -> Result<ActionResult, String> {
    background(|| pipeline::initialize().map_err(user_error)).await
}

#[tauri::command]
async fn create_chat_pack() -> Result<ActionResult, String> {
    background(|| {
    let path = package::create_chat_pack().map_err(user_error)?;
    Ok(ActionResult {
        ok: true,
        title: "Chat Context Pack created".into(),
        detail: "Upload this one ZIP to ChatGPT. It represents the normal multi-file project without flattening it.".into(),
        path: Some(path.to_string_lossy().to_string()),
    })

    }).await
}

#[tauri::command]
async fn scan_latest_update() -> Result<ActionResult, String> {
    background(|| {
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

    }).await
}

#[tauri::command]
async fn apply_latest_update() -> Result<ActionResult, String> {
    background(|| {
    let latest = package::latest_update_pack().map_err(user_error)?
        .ok_or_else(|| "No CrimeSim update pack found in Downloads".to_string())?;
    update::apply(&latest).map_err(user_error)

    }).await
}

#[tauri::command]
async fn apply_latest_update_and_play() -> Result<ActionResult, String> {
    background(|| {
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

    }).await
}

#[tauri::command]
async fn rollback() -> Result<ActionResult, String> {
    background(|| update::rollback().map_err(user_error)).await
}

#[tauri::command]
async fn play_current() -> Result<ActionResult, String> {
    background(|| play::current().map_err(user_error)).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = install_check::configure() {
        eprintln!("{error}");
        std::process::exit(2);
    }
    tauri::Builder::default()
        .setup(|app| {
            match transaction::recover_incomplete() {
                Ok(_) => {
                    if let Err(err) = runtime::install_bundled(app.handle()) {
                        eprintln!("Bundled Godot runtime install failed: {err}");
                    }
                }
                Err(err) => eprintln!("Recovery required; runtime installation skipped: {err}"),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            install_check::frontend_ready,
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
