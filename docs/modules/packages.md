# packages

## Responsibility
Game context ZIPs, update manifests, payload reads, incoming and terminal archives.

## Owned source and checks
The exact file patterns, dependency routes and checks are in `project_control/MODULES.json`.
Use `python tools/project_memory.py brief --module packages` for a bounded reading route.
Read actual source before changing behavior; this card is a map, not an implementation.

## Contract
Full-file replacements are tied to exact base hashes. Omitted assets are unknown contents, not missing gameplay.

## Known limitation at the audit checkpoint
Desktop packs are broad game-workspace exports. ZIP byte/entry limits, duplicate destinations and stronger path rules still need adversarial tests.
See the matching module entries in `project_control/BACKLOG.json` for acceptance criteria.

## Change route
Read the direct dependencies' cards before changing shared interfaces. Inspect affected consumers
as needed; this is not a prohibition on cross-module work. Update this card only when its meaning
changes. Report tests as executed, failed, skipped, or unavailable; never infer success from filenames.
