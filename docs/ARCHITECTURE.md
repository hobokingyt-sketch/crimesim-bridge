# CrimeSim Bridge architecture

The Bridge coordinates multi-file chat updates; Godot imports, validates and exports them.
It is not the game and it does not generate code without a user-directed chat pass.

## Authoritative runtime paths
- workspace/current_project: last committed source, accessed behind the workspace lock.
- builds/current: matching playable build and its validation report.
- state/transaction_journal.json: schema-2 write-ahead promotion/recovery record.
- state/transactions/<id>: unique before-images, retained input and commit receipt.
- state/current_receipt.json: current operation identity and source/build fingerprints.
- runtime/godot: private Windows engine/wrapper/templates (engine pin unchanged).
- applied: committed update archives; quarantine: rejected inputs.

## Code ownership
crates/bridge-safety owns journal, copies, fingerprints, lock and restartable promotion.
core/transaction.rs is the Godot adapter. core/update.rs chooses an update or rollback source;
core/pipeline.rs chooses initialization or repair. None can promote source/build separately.
core/project.rs authors bootstrap data only in a supplied candidate directory.
The shell displays status, invokes commands and never edits source directly.

See docs/RECOVERY.md for the exact state machine, failure policy and test boundaries.
Read project_control/MODULES.json for routing and BACKLOG.json for outstanding issues.
No module card, previous release note or green static check establishes full production safety.
