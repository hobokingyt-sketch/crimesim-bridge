# project

## Responsibility
Managed game metadata and bootstrap source/export presets.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module project` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
A new workspace is a Godot bootstrap, not a developed game. Preserve Godot resource identities and do not silently change the engine pin.

## Known limitation at the audit checkpoint
Game project memory is only a minimal placeholder. Future templates need their own identity and contracts, distinct from Bridge memory.
See the matching module entries in `project_control/BACKLOG.json` for acceptance criteria.

## Change route
Read the direct dependencies' cards before changing shared interfaces. Inspect affected consumers
as needed; this is not a prohibition on cross-module work. Update this card only when its meaning
changes. Report tests as executed, failed, skipped, or unavailable; never infer success from filenames.
