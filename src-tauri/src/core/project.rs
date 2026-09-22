use crate::core::{
    error::{BridgeError, BridgeResult},
    paths,
    types::BridgeProject,
};
use std::{fs, path::Path};

pub fn read_project(project_root: &Path) -> BridgeResult<BridgeProject> {
    let path = project_root.join(paths::PROJECT_META);
    if !path.exists() {
        return Err(BridgeError::Invalid(format!("Missing Bridge project metadata: {}", path.display())));
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

pub fn write_project(project_root: &Path, project: &BridgeProject) -> BridgeResult<()> {
    let path = project_root.join(paths::PROJECT_META);
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    fs::write(path, serde_json::to_vec_pretty(project)?)?;
    Ok(())
}

pub fn ensure_export_preset(project_root: &Path, preset_name: &str) -> BridgeResult<bool> {
    let path = project_root.join("export_presets.cfg");
    if path.exists() { return Ok(false); }
    let content = format!(r#"[preset.0]

name="{preset_name}"
platform="Windows Desktop"
runnable=true
advanced_options=false
dedicated_server=false
custom_features=""
export_filter="all_resources"
include_filter=""
exclude_filter=""
export_path=""
encryption_include_filters=""
encryption_exclude_filters=""
encrypt_pck=false
encrypt_directory=false
script_export_mode=2

[preset.0.options]

custom_template/debug=""
custom_template/release=""
binary_format/architecture="x86_64"
application/modify_resources=true
application/product_name="Criminal Simulation"
ssh_remote_deploy/enabled=false
"#);
    fs::write(path, content)?;
    Ok(true)
}

pub fn bootstrap_demo_at(root: &Path) -> BridgeResult<BridgeProject> {
    if root.join("project.godot").exists() {
        let project = read_project(&root)?;
        ensure_export_preset(&root, &project.export_preset)?;
        return Ok(project);
    }

    fs::create_dir_all(root.join("game"))?;
    fs::create_dir_all(root.join("tests"))?;
    fs::create_dir_all(root.join("project_control"))?;

    fs::write(root.join("project.godot"), r#"; CrimeSim Bridge bootstrap project
config_version=5

[application]
config/name="CrimeSim Bridge Bootstrap"
run/main_scene="res://game/main.tscn"

[display]
window/size/viewport_width=2560
window/size/viewport_height=1440
window/size/window_width_override=1280
window/size/window_height_override=720

[rendering]
renderer/rendering_method="gl_compatibility"
renderer/rendering_method.mobile="gl_compatibility"
"#)?;

    fs::write(root.join("game/main.tscn"), r#"[gd_scene load_steps=2 format=3]

[ext_resource path="res://game/main.gd" type="Script" id="1"]

[node name="Main" type="Node"]
script = ExtResource("1")
"#)?;
    fs::write(root.join("game/main.gd"), "extends Node\n\nvar bridge_boot_ok := true\n")?;
    fs::write(root.join("tests/bridge_smoke.gd"), r#"extends SceneTree

func _init() -> void:
    var packed: PackedScene = load("res://game/main.tscn") as PackedScene
    if packed == null:
        push_error("Bridge smoke test could not load main scene")
        quit(10)
        return
    var instance: Node = packed.instantiate()
    if instance == null:
        push_error("Bridge smoke test could not instantiate main scene")
        quit(11)
        return
    instance.free()
    quit(0)
"#)?;

    let project = default_metadata();
    write_project(&root, &project)?;
    ensure_export_preset(&root, &project.export_preset)?;
    fs::write(root.join("project_control/PROJECT.md"), "# Criminal Simulation\n\nBridge bootstrap. The map is the world; UI is the command OS.\n")?;
    fs::write(root.join("project_control/CURRENT_STATE.md"), "# Current State\n\nRevision 0. Bridge bootstrap only. No game systems implemented.\n")?;
    Ok(project)
}

/// Pinned game identity. This creates no files or live workspace.
pub fn default_metadata() -> BridgeProject {
    BridgeProject {
        schema: 1,
        project_id: "crime_sim".into(),
        project_name: "Criminal Simulation".into(),
        engine: "Godot".into(),
        engine_version: "4.7.2".into(),
        revision: 0,
        save_schema: 1,
        main_scene: "res://game/main.tscn".into(),
        export_preset: "Windows Desktop".into(),
    }
}
