# delivery

## Responsibility
Build configuration, fixtures, installer delivery and Windows integration proof.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module delivery` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
Fast recovery CI runs production-library process-death tests separately from Windows Godot E2E and installer packaging. PR Windows builds validate adapters and E2E without publishing an installer.

## Known limitation
Installer output-path, clean-install and locked-toolchain work remain BUILD-1. Cargo.lock is not added by this pass.

## Change route
Read only affected source and neighbor interfaces. Update decisions and checks with intentional changes.
See docs/RECOVERY.md for the recovery state machine; do not add bypass promotion helpers.
