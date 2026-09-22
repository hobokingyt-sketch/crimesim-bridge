# CrimeSim Bridge v0.2 Hardening changes

## Added

- Durable transaction journal with explicit phases.
- Startup crash recovery to the last committed source + playable build revision.
- Revision-addressable playable-build history in addition to source history.
- Atomic-ish `__next`/`__old` promotion for both source and builds.
- SHA-256 runtime integrity manifest support for Godot/editor export templates.
- Runtime verification before any Godot validation/export action.
- Automatic reinstall of a bad private runtime when a valid bundled runtime is available.
- Automatic minimal Windows Desktop `export_presets.cfg` bootstrap.
- Required playable build before a revision can commit.
- Structured Godot error summaries while retaining full logs.
- Automatic rejected-package quarantine with reason JSON.
- Recovery/runtime state surfaced in desktop UI.
- Context packs now carry Bridge state and last validation result when present.
- Crash recovery and runtime-integrity model tests.
- Windows CI now runs repository/model tests and `cargo check` before building the NSIS installer.

## Deliberately deferred

- Ed25519 update signing. It becomes meaningful with Repository/remote transport and a real signing identity. Hard-coding or locally sharing a signing key in manual Chat ZIP mode would add ceremony without real provenance.
- GitHub transport, stable/dev channels, self-updater and large-asset request packs remain v0.3 work.

## Validation performed in this environment

- `tools/validate_repo.py` — passed.
- `tools/test_transaction_model.py` — passed.
- `tools/test_crash_recovery_model.py` — passed.
- `tools/test_runtime_manifest_model.py` — passed.
- `node --check web/app.js` — passed.
- Python tool scripts compile — passed.

Rust/Windows compilation cannot run in this Linux container because a Rust toolchain is not installed. The Windows CI workflow now performs `cargo check --workspace` before installer construction so the first real Windows build fails closed on compile errors.
