# CrimeSim Bridge v0.25 — First Real Windows Pipeline

CrimeSim Bridge is a Windows-first transactional pipeline for developing a normal multi-file Godot project through ChatGPT without requiring the user to operate the Godot editor.

## Intended user loop

1. Press **Create Chat Pack**.
2. Upload the generated context ZIP to ChatGPT.
3. ChatGPT returns one `CrimeSim_Update_####.zip` containing a manifest and complete changed/new files.
4. Download that ZIP normally.
5. Press **Apply Latest Update & Play**.
6. The Bridge verifies and stages the update, runs the private Godot runtime headlessly, executes smoke tests, exports a Windows build, promotes source + playable build as one revision, archives the consumed update, and launches the promoted game.
7. Test the game and continue the design conversation.

The ZIP is transport only. The game remains a normal structured Godot project.

## What v0.25 proves

v0.25 adds a Windows CI end-to-end harness that uses the actual Godot 4.7.2 runtime and export templates. The runner:

1. verifies the downloaded engine version and runtime hashes;
2. creates an isolated Bridge data root;
3. initializes revision 0 of a real Godot project;
4. imports/tests/exports it headlessly;
5. launches the exported `CrimeSim.exe` headlessly;
6. applies the real revision-1 fixture update through the transaction engine;
7. verifies source revision = playable revision = 1;
8. rolls back to revision 0 and rebuilds it;
9. reapplies revision 1;
10. creates a Chat Context Pack;
11. only then builds the Tauri NSIS installer.

This moves the pipeline from static/model validation to an executable integration test on Windows.

## Safety model

- Live source is never patched directly.
- Every replace/delete verifies the SHA-256 of the expected current file.
- Every create/replace verifies the package payload SHA-256.
- Project ID, Godot version, and exact base revision must match.
- Unsafe/traversal/generated paths are rejected.
- A durable transaction journal records the update phase.
- Interrupted uncommitted transactions restore the last committed source and playable build.
- Source snapshots and playable-build snapshots are retained by revision.
- Rejected update packs enter quarantine with a structured reason.
- Successfully consumed update packs enter the Applied archive and are no longer rediscovered as pending work.
- The private Godot runtime is verified against a SHA-256 runtime manifest and its actual `--version` output.
- Missing Windows export configuration is bootstrapped automatically.
- A candidate cannot commit unless its exported Windows EXE launches successfully in a headless smoke run.

## Test/CI overrides

The Bridge supports three environment overrides used by the integration harness:

- `CRIMESIM_BRIDGE_ROOT` — isolated application-data root.
- `CRIMESIM_BRIDGE_DOWNLOADS` — isolated Downloads transport directory.
- `CRIMESIM_BRIDGE_RUNTIME_SOURCE` — verified private runtime source.

Normal installed use does not require setting these.

## Build

The Windows GitHub Actions workflow downloads official Godot 4.7.2 Windows files, creates the private runtime integrity manifest, runs Python/JavaScript model checks, performs `cargo check`, executes the real Godot end-to-end Bridge smoke binary, then builds an NSIS installer and emits a SHA-256 release manifest.

Godot remains a private worker process. Normal development never requires opening or installing the editor system-wide.

## Repository layout

- `web/` — dependency-free Tauri front end.
- `src-tauri/src/core/` — package, transaction, workspace, Godot, runtime, pipeline, recovery and status modules.
- `src-tauri/src/bin/bridge_pipeline_smoke.rs` — real Windows/Godot integration harness.
- `docs/` — architecture and package contracts.
- `tools/` — fixture/model/contract validation utilities.
- `.github/workflows/` — Windows runtime provisioning, E2E validation and installer build.

See `BRIDGE_CONTROL.md`, `docs/ARCHITECTURE.md`, and `docs/UPDATE_PACKAGE_SPEC.md` before changing the transaction model.
