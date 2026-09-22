# transactions

## Responsibility
Candidate updates, revision promotion, journaling and recovery.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module transactions` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
The Godot adapter invokes the shared bridge-safety crate for all source/build promotion. Its schema-2 journal is the commit record. Before-images are transaction-specific. No promotion after failed validation; any recovery failure blocks later operations.

## Known limitation
Process-death tests cover filesystem transitions, not physical power loss. Unknown/legacy journals are deliberately preserved and block. Save-schema transitions remain SAFE-4.

## Change route
Read only affected source and neighbor interfaces. Update decisions and checks with intentional changes.
See docs/RECOVERY.md for the recovery state machine; do not add bypass promotion helpers.
