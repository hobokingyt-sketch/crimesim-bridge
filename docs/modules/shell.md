# shell

## Responsibility
Desktop controls, command dispatch, initialization and displayed health.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module shell` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
Keep orchestration and UI separate from state-owning modules. Display verified current-build health separately from the latest attempted update.

## Known limitation at the audit checkpoint
Initialization promotes outside the normal transaction journal. Buttons disabling is not a backend workspace lock. Desktop first-run UX is unverified.
See the matching module entries in `project_control/BACKLOG.json` for acceptance criteria.

## Change route
Read the direct dependencies' cards before changing shared interfaces. Inspect affected consumers
as needed; this is not a prohibition on cross-module work. Update this card only when its meaning
changes. Report tests as executed, failed, skipped, or unavailable; never infer success from filenames.
