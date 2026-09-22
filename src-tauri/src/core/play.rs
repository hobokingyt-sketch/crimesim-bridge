use crate::core::{error::{BridgeError, BridgeResult}, godot, paths, types::ActionResult};
use std::process::Command;

pub fn current() -> BridgeResult<ActionResult> {
    let _workspace_guard = crate::core::transaction::open_recovered()?;
    let exe = paths::current_build()?.join("CrimeSim.exe");
    if !exe.exists() {
        return Ok(ActionResult {
            ok: false,
            title: "No playable build".into(),
            detail: "A validated Windows build has not been promoted yet.".into(),
            path: None,
        });
    }
    Command::new(&exe).spawn().map_err(BridgeError::Io)?;
    Ok(ActionResult {
        ok: true,
        title: "Game launched".into(),
        detail: "Current validated build started.".into(),
        path: Some(exe.to_string_lossy().to_string()),
    })
}

pub fn smoke_current() -> BridgeResult<ActionResult> {
    let (ok, detail) = godot::smoke_current_build()?;
    Ok(ActionResult {
        ok,
        title: if ok { "Playable smoke passed" } else { "Playable smoke failed" }.into(),
        detail,
        path: Some(paths::current_build()?.to_string_lossy().to_string()),
    })
}
