# engine

## Responsibility
Private Godot installation, process invocation, validation, export and launch.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module engine` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
Keep the official Windows executable/console-wrapper pair together. Hash verification is integrity checking, not proof of authorship or sandboxing.

## Known limitation at the audit checkpoint
Command::output has no parent-enforced deadline. --quit-after limits engine iterations, not a hung process. Worker discovery is not bound to a required manifest entry.
See the matching module entries in `project_control/BACKLOG.json` for acceptance criteria.

## Change route
Read the direct dependencies' cards before changing shared interfaces. Inspect affected consumers
as needed; this is not a prohibition on cross-module work. Update this card only when its meaning
changes. Report tests as executed, failed, skipped, or unavailable; never infer success from filenames.
