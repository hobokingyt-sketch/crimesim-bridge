# delivery

## Responsibility
Build configuration, fixtures, installer delivery and Windows integration proof.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module delivery` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
Keep fast checks, real Windows validation and installer publication distinct. A configured workflow is not a successful release.

## Known limitation at the audit checkpoint
Workspace target output is at root target/ but CI cache and installer collection use src-tauri/target/. No committed Cargo.lock; dependency/toolchain reproducibility is incomplete.
See the matching module entries in `project_control/BACKLOG.json` for acceptance criteria.

## Change route
Read the direct dependencies' cards before changing shared interfaces. Inspect affected consumers
as needed; this is not a prohibition on cross-module work. Update this card only when its meaning
changes. Report tests as executed, failed, skipped, or unavailable; never infer success from filenames.
