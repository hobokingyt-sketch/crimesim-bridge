# Project Armor II: recoverable promotion (SAFE-1)

Candidate implementation; exact-commit Rust/Windows evidence must be checked before merge.

- Shared bridge-safety crate used by update, initialization, repair and rollback.
- Schema-2 journal, synced before-images, repeated interruption recovery and commit receipts.
- No unlink-before-rename journal gap; no swallowed recovery failures.
- Operation-specific history removes new integer-revision snapshot collisions.
- OS workspace lock serializes app writes, launch requests and context snapshots.
- Captured update bytes are revalidated; commit archive failures retain retry state.
- Recovery-required status is visible and never READY.
- Forced process-death tests in the actual coordinator and separate real Godot E2E.

Not an installer release. No game systems or engine version change.
Legacy incomplete journals block instead of unsafe migration. Save-schema handling, process-tree
watchdogs, comprehensive ZIP attack validation and scoped desktop game packs remain tracked work.
