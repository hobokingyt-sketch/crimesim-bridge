# memory

## Responsibility
Task routing, project decisions, evidence checkpoints and scoped source handoffs.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module memory` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
Every maintained module has a discoverable owner, source paths, checks and a short contract. Small changes must remain inexpensive.

## Known limitation at the audit checkpoint
These tools guide and check structure; they cannot force an AI to reason correctly. No desktop task selector is implemented in this pass.
See the matching module entries in `project_control/BACKLOG.json` for acceptance criteria.

## Change route
Read the direct dependencies' cards before changing shared interfaces. Inspect affected consumers
as needed; this is not a prohibition on cross-module work. Update this card only when its meaning
changes. Report tests as executed, failed, skipped, or unavailable; never infer success from filenames.
