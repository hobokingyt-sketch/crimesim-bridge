# CrimeSim Bridge architecture

## Principle

The Bridge is a transaction coordinator between a chat-produced change package and a normal Godot project. Godot does not apply updates. The Bridge applies a candidate update to staging; Godot only imports, validates, tests and exports that candidate.

## Authoritative layers

1. `workspace/current_project` — last committed source revision.
2. `history/source/rev_####` — retained committed source snapshots.
3. `builds/current` — last committed playable Windows build.
4. `history/builds/rev_####` — retained playable build snapshots.
5. `workspace/staging` — disposable candidate source.
6. `builds/candidate` — disposable candidate export.
7. `state/transaction_journal.json` — durable mutation intent/phase.
8. `runtime/godot` — private runtime verified against `runtime_manifest.json` and the executable's `--version` output.
9. `quarantine` — rejected packages and machine-readable reasons.
10. `applied` — successfully consumed update packages, removed from normal pending-update discovery.

## Transaction lifecycle

`verified -> staged -> validated -> source_promoting -> source_promoted -> build_promoting -> committed`

The journal is written before any promotion. A process/machine interruption at any phase except `committed` causes startup recovery to restore the journal's base revision. A crash after `committed` but before cleanup completes cleanup and archives the consumed package when possible.

## Promotion

Source and build promotion use `__next` / `__old` directories rather than copying directly over live trees. The immediately previous committed revision is snapshotted before promotion.

## Validation gates

- Project/main-scene structure.
- Windows export-preset presence/bootstrap.
- Private Godot runtime SHA-256 integrity.
- Godot executable version equals the project's pinned engine version.
- Headless Godot import.
- Project-owned smoke test when present.
- Headless Windows debug export.
- Exported `CrimeSim.exe` exists, is nontrivial in size, launches headlessly, and exits successfully after a bounded number of iterations.

A transaction cannot commit unless a playable Windows build passes every required gate.

## One-button flow

`Apply Latest Update & Play` resolves the newest pending package, transactionally applies it, promotes source/build only after validation, archives the consumed ZIP, then launches the promoted game. Applying and launching are separate internal operations but one user action.

## CI integration harness

`bridge_pipeline_smoke` runs the core without the Tauri UI against an isolated root supplied by environment variables. On Windows CI it installs the prepared private runtime, initializes revision 0, builds/launches it, applies revision 1, validates revision convergence, tests rollback, reapplies revision 1, and creates a context pack. The NSIS installer is built only after this passes.

## Failure behavior

Before live promotion: reject, clean staging, quarantine package.

During/after promotion but before commit: durable journal remains; recovery restores base source/build state. The rejected package is quarantined when the apply call regains control.

Full Godot stdout/stderr is kept in logs; the UI receives a bounded human-readable error summary.
