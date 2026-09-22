# transactions

## Responsibility
Candidate updates, revision promotion, journaling and recovery.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module transactions` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
A candidate must not become live before required validation. Source and executable revisions must remain coherent.

## Known limitation at the audit checkpoint
Rollback bypasses the update journal; recovery errors are swallowed in apply; old history can collide after rollback and divergent reapply. These are open risks, not guarantees.
See the matching module entries in `project_control/BACKLOG.json` for acceptance criteria.

## Change route
Read the direct dependencies' cards before changing shared interfaces. Inspect affected consumers
as needed; this is not a prohibition on cross-module work. Update this card only when its meaning
changes. Report tests as executed, failed, skipped, or unavailable; never infer success from filenames.
