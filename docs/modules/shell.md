# shell

## Responsibility
Desktop controls, command dispatch, initialization and displayed health.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module shell` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
Initialization and repair assemble candidates in staging and enter the same coordinator as update/rollback. Pending journals and recovery errors suppress READY and block mutation, context export and launch requests.

## Worker boundary
Long filesystem/process commands run on the Tauri blocking worker pool, not the webview event loop. A pending worker guard suppresses READY and exposes cleanup-required status. No desktop Cancel button is implemented.

## Known limitation
Current build health versus latest attempted validation remains HEALTH-1. Desktop UX still needs installed testing; parent-enforced worker deadlines remain SAFE-2.

## Change route
Read only affected source and neighbor interfaces. Update decisions and checks with intentional changes.
See docs/RECOVERY.md for the recovery state machine; do not add bypass promotion helpers.
