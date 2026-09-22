# packages

## Responsibility
Game context ZIPs, update manifests, payload reads, incoming and terminal archives.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module packages` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
Update bytes are captured in the transaction directory and revalidated before assembly. Applied archives are content-checked; archive failures retain the committed journal for retry. Context export acquires the workspace lock.

## Known limitation
ZIP size/duplicate/Windows-path attacks remain SAFE-3. Task-scoped game packs and desktop button integration remain PACK-1.

## Change route
Read only affected source and neighbor interfaces. Update decisions and checks with intentional changes.
See docs/RECOVERY.md for the recovery state machine; do not add bypass promotion helpers.
