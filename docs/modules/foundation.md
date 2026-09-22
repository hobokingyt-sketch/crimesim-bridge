# foundation

## Responsibility
Shared types, paths, hashes and filesystem helpers.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module foundation` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
Filesystem replacement writes and syncs a same-directory temporary file, then renames without unlinking the previous destination. Snapshots reject links/reparse paths and preserve source UIDs.

## Known limitation
Directory durability differs by OS/filesystem. Standard OS flush requests do not prove controller-level power-loss safety. Full incoming ZIP attack handling remains SAFE-3.

## Change route
Read only affected source and neighbor interfaces. Update decisions and checks with intentional changes.
See docs/RECOVERY.md for the recovery state machine; do not add bypass promotion helpers.
