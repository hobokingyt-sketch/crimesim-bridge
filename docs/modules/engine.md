# engine

## Responsibility
Private Godot installation, bounded worker execution, validation, export and launch.

## Owned source and checks
Use the engine entry in project_control/MODULES.json and `brief --module engine`.
Read actual source; this card is a route, not a replacement for code.

## Contract
The official Windows engine/wrapper pair stays together. All build/version/smoke workers
use core/worker.rs and bridge-worker. Parent deadlines, bounded captures and named jobs
cover children and permit startup cleanup before source recovery. Normal Play is separate.
See docs/WORKERS.md for budgets, failure behavior and platform boundaries.

## Remaining limitations
Hashes are integrity checks, not authorship/sandboxing. Binding the chosen worker and
required partner/templates to the manifest is still SAFE-5. Linux test support does not
claim Windows Job Object parent-death guarantees. Installed UX remains unverified.

## Change route
Inspect foundation, transactions and shell consumers when changing worker/recovery contracts.
Update tests and policy deliberately; module boundaries do not prohibit cross-module work.
