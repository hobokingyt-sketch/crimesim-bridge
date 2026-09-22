# foundation

## Responsibility
Shared types, paths, hashes and filesystem helpers.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module foundation` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
Keep filesystem ownership explicit. Generated caches and user saves are not authoritative project source.

## Known limitation at the audit checkpoint
atomic_write currently deletes the destination before rename; the name is not a crash-durability guarantee.
See the matching module entries in `project_control/BACKLOG.json` for acceptance criteria.

## Change route
Read the direct dependencies' cards before changing shared interfaces. Inspect affected consumers
as needed; this is not a prohibition on cross-module work. Update this card only when its meaning
changes. Report tests as executed, failed, skipped, or unavailable; never infer success from filenames.
