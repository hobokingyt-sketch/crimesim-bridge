use crate::core::{
    error::{BridgeError, BridgeResult},
    paths, project, runtime,
    types::{BridgeProject, ValidationReport, ValidationStep},
};
use chrono::Utc;
use std::{fs, path::{Path, PathBuf}, process::Command};

fn summarize_errors(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.contains("error") || lower.contains("failed") || lower.contains("parse") {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !out.iter().any(|v| v == trimmed) {
                out.push(trimmed.chars().take(220).collect());
                if out.len() >= 6 { break; }
            }
        }
    }
    out
}

fn write_log(log_name: &str, text: &str) -> BridgeResult<()> {
    fs::create_dir_all(paths::logs_root()?)?;
    fs::write(paths::logs_root()?.join(log_name), text)?;
    Ok(())
}

fn run_godot(godot: &Path, project: &Path, args: &[&str], log_name: &str) -> BridgeResult<(bool, String)> {
    let output = Command::new(godot)
        .arg("--headless")
        .arg("--path")
        .arg(project)
        .args(args)
        .output()?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.stderr.is_empty() {
        text.push_str("\n--- STDERR ---\n");
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    write_log(log_name, &text)?;
    Ok((output.status.success(), text))
}

fn run_exported_smoke(exe: &Path, log_name: &str) -> BridgeResult<(bool, String)> {
    let output = Command::new(exe)
        .arg("--headless")
        .arg("--quit-after")
        .arg("5")
        .arg("--no-header")
        .output()?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.stderr.is_empty() {
        text.push_str("\n--- STDERR ---\n");
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    write_log(log_name, &text)?;
    Ok((output.status.success(), text))
}

pub fn validate(project_root: &Path, meta: &BridgeProject) -> BridgeResult<(ValidationReport, Option<PathBuf>)> {
    let mut steps = Vec::new();
    let mut errors = Vec::new();

    let structure_ok = project_root.join("project.godot").exists()
        && project_root.join(meta.main_scene.trim_start_matches("res://")).exists();
    steps.push(ValidationStep {
        name: "Project structure".into(),
        status: if structure_ok { "passed" } else { "failed" }.into(),
        detail: if structure_ok { "project.godot and configured main scene are present".into() } else { "Missing project.godot or configured main scene".into() },
    });
    if !structure_ok {
        errors.push("Project structure is incomplete".into());
        return Ok((report(meta.revision, steps, errors), None));
    }

    let preset_created = project::ensure_export_preset(project_root, &meta.export_preset)?;
    steps.push(ValidationStep {
        name: "Export preset".into(),
        status: "passed".into(),
        detail: if preset_created { "Bridge created the default Windows Desktop export preset".into() } else { format!("Using export preset: {}", meta.export_preset) },
    });

    let (runtime_ok, runtime_detail) = runtime::integrity()?;
    steps.push(ValidationStep {
        name: "Godot runtime integrity".into(),
        status: if runtime_ok { "passed" } else { "failed" }.into(),
        detail: runtime_detail.clone(),
    });
    if !runtime_ok {
        errors.push(runtime_detail);
        return Ok((report(meta.revision, steps, errors), None));
    }

    let (version_ok, version_detail) = runtime::verify_engine_version(&meta.engine_version)?;
    steps.push(ValidationStep {
        name: "Godot engine version".into(),
        status: if version_ok { "passed" } else { "failed" }.into(),
        detail: version_detail.clone(),
    });
    if !version_ok {
        errors.push(version_detail);
        return Ok((report(meta.revision, steps, errors), None));
    }

    let godot = paths::runtime_godot()?;
    let (ok, text) = run_godot(&godot, project_root, &["--import"], "godot-import.log")?;
    if !ok { errors.extend(summarize_errors(&text)); }
    steps.push(ValidationStep { name: "Godot import".into(), status: if ok { "passed" } else { "failed" }.into(), detail: "Headless resource import".into() });
    if !ok { return Ok((report(meta.revision, steps, errors), None)); }

    let smoke_path = project_root.join("tests/bridge_smoke.gd");
    if smoke_path.exists() {
        let (ok, text) = run_godot(&godot, project_root, &["--script", "res://tests/bridge_smoke.gd"], "godot-smoke.log")?;
        if !ok { errors.extend(summarize_errors(&text)); }
        steps.push(ValidationStep { name: "Project smoke test".into(), status: if ok { "passed" } else { "failed" }.into(), detail: "Project-owned headless smoke harness".into() });
        if !ok { return Ok((report(meta.revision, steps, errors), None)); }
    } else {
        steps.push(ValidationStep { name: "Project smoke test".into(), status: "skipped".into(), detail: "No tests/bridge_smoke.gd yet".into() });
    }

    let candidate = paths::builds_root()?.join("candidate").join(format!("rev_{:04}", meta.revision));
    if candidate.exists() { fs::remove_dir_all(&candidate)?; }
    fs::create_dir_all(&candidate)?;
    let exe = candidate.join("CrimeSim.exe");
    let exe_str = exe.to_str().ok_or_else(|| BridgeError::Invalid("Build path is not valid UTF-8".into()))?;
    let (ok, text) = run_godot(&godot, project_root, &["--export-debug", &meta.export_preset, exe_str], "godot-export.log")?;
    if !ok { errors.extend(summarize_errors(&text)); }
    let export_ok = ok && exe.is_file() && fs::metadata(&exe).map(|m| m.len() > 64 * 1024).unwrap_or(false);
    steps.push(ValidationStep {
        name: "Windows build".into(),
        status: if export_ok { "passed" } else { "failed" }.into(),
        detail: format!("Headless export preset: {}", meta.export_preset),
    });
    if !export_ok {
        if errors.is_empty() { errors.push("Godot did not produce a valid Windows executable".into()); }
        return Ok((report(meta.revision, steps, errors), None));
    }

    let (smoke_ok, text) = run_exported_smoke(&exe, "exported-build-smoke.log")?;
    if !smoke_ok { errors.extend(summarize_errors(&text)); }
    steps.push(ValidationStep {
        name: "Exported build launch".into(),
        status: if smoke_ok { "passed" } else { "failed" }.into(),
        detail: "CrimeSim.exe launched headlessly and exited after five iterations".into(),
    });
    let build = smoke_ok.then_some(candidate);
    Ok((report(meta.revision, steps, errors), build))
}

pub fn smoke_current_build() -> BridgeResult<(bool, String)> {
    let exe = paths::current_build()?.join("CrimeSim.exe");
    if !exe.is_file() {
        return Ok((false, "Current playable build is missing CrimeSim.exe".into()));
    }
    let (ok, text) = run_exported_smoke(&exe, "current-build-smoke.log")?;
    Ok((ok, if ok { "Current promoted build launched successfully".into() } else { summarize_errors(&text).join(" | ") }))
}

fn report(revision: u64, steps: Vec<ValidationStep>, errors: Vec<String>) -> ValidationReport {
    let passed = steps.iter().all(|s| s.status != "failed");
    ValidationReport { passed, revision, created_at: Utc::now().to_rfc3339(), steps, errors }
}
