# project

## Responsibility
Managed game metadata and bootstrap source/export presets.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module project` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
Bootstrap writes only to a caller-supplied candidate directory. Default metadata is separate from file creation. The pinned engine, scene resources and profession/game scope do not change in this pass.

## Known limitation
Managed-game memory is still a minimal placeholder (PACK-1). Save-schema transition policy remains SAFE-4.

## Change route
Read only affected source and neighbor interfaces. Update decisions and checks with intentional changes.
See docs/RECOVERY.md for the recovery state machine; do not add bypass promotion helpers.
