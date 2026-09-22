use crimesim_bridge_lib::core::{package, paths, pipeline, project, runtime, status, update};
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = env::var_os("CRIMESIM_BRIDGE_ROOT")
        .map(PathBuf::from)
        .ok_or("CRIMESIM_BRIDGE_ROOT must be set for bridge_pipeline_smoke")?;
    let runtime_source = env::var_os("CRIMESIM_BRIDGE_RUNTIME_SOURCE")
        .map(PathBuf::from)
        .ok_or("CRIMESIM_BRIDGE_RUNTIME_SOURCE must be set for bridge_pipeline_smoke")?;

    if root.exists() { fs::remove_dir_all(&root)?; }
    fs::create_dir_all(&root)?;

    runtime::install_from_dir(&runtime_source)?;
    let (runtime_ok, runtime_detail) = runtime::integrity()?;
    if !runtime_ok { return Err(runtime_detail.into()); }
    println!("runtime: {runtime_detail}");

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent().ok_or("Could not resolve repository root")?;
    let fixture = repo_root.join("fixtures/CrimeSim_Update_0001.zip");
    if !fixture.is_file() { return Err(format!("Fixture missing: {}", fixture.display()).into()); }

    let result = pipeline::assert_end_to_end(&fixture)?;
    println!("pipeline: {} — {}", result.title, result.detail);

    let rolled = update::rollback()?;
    if !rolled.ok { return Err(format!("Rollback smoke failed: {}", rolled.detail).into()); }
    let after_rollback = project::read_project(&paths::current_project()?)?;
    if after_rollback.revision != 0 {
        return Err(format!("Rollback expected revision 0, got {}", after_rollback.revision).into());
    }
    println!("rollback: revision 0 restored and rebuilt");

    let incoming = paths::root()?.join("incoming/CrimeSim_Update_0001.zip");
    fs::copy(&fixture, &incoming)?;
    let reapplied = update::apply(&incoming)?;
    if !reapplied.ok { return Err(format!("Reapply smoke failed: {}", reapplied.detail).into()); }
    println!("reapply: {}", reapplied.title);

    let context_pack = package::create_chat_pack()?;
    if !context_pack.is_file() { return Err("Context pack was not created".into()); }
    println!("context-pack: {}", context_pack.display());

    let final_status = status::get()?;
    if !final_status.pipeline_ready || final_status.source_revision != Some(1) || final_status.playable_revision != Some(1) {
        return Err(format!(
            "Final status unhealthy: ready={}, source={:?}, playable={:?}",
            final_status.pipeline_ready, final_status.source_revision, final_status.playable_revision
        ).into());
    }

    let applied = paths::applied_root()?;
    let archived_count = fs::read_dir(&applied)?.filter_map(Result::ok).count();
    if archived_count == 0 { return Err("Successful update was not archived".into()); }

    println!("PASS: real Godot runtime -> revision 0 build -> fixture revision 1 -> exported launch -> rollback -> reapply -> context pack");
    Ok(())
}
