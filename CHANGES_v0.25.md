# CrimeSim Bridge v0.25 — First Real Windows Pipeline

## Added

- Real Windows/Godot E2E harness: `bridge_pipeline_smoke`.
- Isolated `CRIMESIM_BRIDGE_ROOT`, `CRIMESIM_BRIDGE_DOWNLOADS`, and `CRIMESIM_BRIDGE_RUNTIME_SOURCE` overrides for deterministic CI.
- Godot executable `--version` verification.
- Exported `CrimeSim.exe` launch smoke gate.
- Initialization now produces revision-0 source **and** a validated playable build.
- One-button **Apply Latest Update & Play** command/UI flow.
- Applied-package archive.
- Windows CI release manifest containing installer SHA-256.
- CI diagnostic artifact containing Godot logs/state/current build.

## Changed

- Bridge version is now 0.2.5.
- Pipeline health now requires source/playable revision convergence, verified runtime, passing last validation, and a current executable.
- Windows installer packaging occurs only after real Godot E2E validation passes.

## Fixed

- Successfully consumed update ZIPs no longer remain in Downloads and get rediscovered as stale updates.
- Rollback staging is cleaned after successful rollback.
