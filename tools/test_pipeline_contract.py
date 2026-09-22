from pathlib import Path

root = Path(__file__).resolve().parents[1]
required = {
    "src-tauri/src/core/paths.rs": ["CRIMESIM_BRIDGE_ROOT", "CRIMESIM_BRIDGE_DOWNLOADS", "applied_root"],
    "src-tauri/src/core/runtime.rs": ["install_from_dir", "verify_engine_version", "runtime_manifest.json"],
    "src-tauri/src/core/godot.rs": ["Exported build launch", "--quit-after", "--export-debug"],
    "src-tauri/src/core/pipeline.rs": ["assert_end_to_end", "Pipeline initialized"],
    "src-tauri/src/bin/bridge_pipeline_smoke.rs": ["assert_end_to_end", "rollback", "create_chat_pack"],
    "crates/bridge-safety/src/workspace.rs": ["package_archived", "committed", "displaced_source"],
    "src-tauri/src/lib.rs": ["apply_latest_update_and_play", "initialize_pipeline"],
    ".github/workflows/build-windows.yml": ["Run real Godot end-to-end Bridge smoke", "bridge_pipeline_smoke", "release_manifest.json"],
}

missing = []
for rel, needles in required.items():
    p = root / rel
    if not p.is_file():
        missing.append(f"missing file: {rel}")
        continue
    text = p.read_text(encoding="utf-8")
    for needle in needles:
        if needle not in text:
            missing.append(f"{rel}: missing {needle!r}")

if missing:
    raise SystemExit("PIPELINE CONTRACT FAILED\n" + "\n".join(missing))
print("PIPELINE CONTRACT PASSED")
