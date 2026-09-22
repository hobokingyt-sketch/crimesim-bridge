# Bridge roadmap

> Project Armor I: begin at `AGENTS.md` and `project_control/CURRENT.json`.
> This document describes design intent and historical implementation, not blanket proof of safety.
> Open gaps and acceptance criteria are tracked in `project_control/BACKLOG.json`.

## Foundation v0.1 — implemented; verification scoped

- Multi-file Tauri/Rust application.
- Workspace/project metadata.
- Chat Context Pack creation.
- Revisioned Update Pack contract.
- SHA-256/base-revision drift protection.
- Staging-only application.
- Headless Godot import/smoke/export hooks.
- Source rollback.
- Minimal one-button-oriented UI.
- Windows CI installer pipeline skeleton.

## Hardening v0.2 — implemented; verification scoped

- Durable transaction journal.
- Startup crash recovery to last committed revision.
- Revision-addressable source and playable-build history.
- SHA-256 integrity manifest for bundled Godot runtime/templates.
- Runtime reinstallation when a bundled local runtime is invalid.
- Structured Godot validation error summaries.
- Automatic quarantine of rejected update packs.
- Automatic Windows Desktop export-preset bootstrap.
- Source + playable build required before update commit.
- Recovery-model fixtures/tests.

## First Real Windows Pipeline v0.25 — implementation checkpoint; inspect current evidence

- Isolated Bridge root/download/runtime overrides for deterministic integration testing.
- Verify actual Godot `--version` against pinned project version.
- Build revision 0 during initialization rather than creating source only.
- Exported-Windows-build launch smoke before promotion.
- Real `bridge_pipeline_smoke` Rust integration harness.
- Windows CI executes real Godot import/test/export/launch/update/rollback/reapply before installer packaging.
- Successful update packs move to an Applied archive instead of remaining discoverable.
- One user action: **Apply Latest Update & Play**.
- Release artifact includes installer SHA-256 manifest and E2E diagnostics.

## Production transport v0.3

- Direct GitHub transport while retaining ZIP Bundle Mode.
- Stable/dev channels.
- Authenticated repository/release provenance.
- Save-schema compatibility gates.
- Incremental/hardlink staging for large projects.
- Asset request packs.
- One-button Bridge self-update.

### Signing note

Ed25519 package signing remains deferred until repository/remote transport provides a meaningful signing identity and key-distribution model. Manual Chat ZIP mode already has explicit human approval plus revision/hash validation; a local hard-coded signing key would authenticate nothing useful.
