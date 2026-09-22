# Recovery contract (SAFE-1)

## Scope and owners
`crates/bridge-safety` owns filesystem snapshots, schema-2 journal, exclusive OS lock and paired
promotion. The desktop `core/transaction.rs` adapter owns invocation of the existing Godot
validation/export gates. Update, first initialization, repair and rollback all use that adapter.
The crate has no Tauri, game or GPU dependency; tests exercise the same code linked by the app.

## State transition
Preparing -> Ready -> SourcePromoting -> SourcePromoted -> BuildPromoting -> PairPromoted -> Committed.
A Preparing journal precedes snapshots and assembly. Live source/build are not touched until Ready,
which requires both complete before-images, validated source identity, executable, passing target-revision
report and synced candidate files. Both live tree fingerprints must still equal the captured base.

Before Committed, restart restores BOTH exact before-images (including initial absence). It validates
both before-images before changing either live path. Restoration uses scratch + rename, never a partial
copy over live data. Restoring and Recovered are restartable states. Repeated recovery is idempotent.
After Committed, restart verifies and keeps the new pair, writes its receipt and retries archive/cleanup.
Cleanup failure cannot turn an already committed package into a rejected package.

The source/build pair briefly changes in two filesystem renames while the exclusive workspace lock is
held. It is NOT an atomic two-directory OS operation. App mutation, launch and context export requests
cannot observe/use that midpoint; restart recovery completes before those requests can proceed.
Already-running game processes and their future save compatibility need SAFE-2/SAFE-4.

## Storage and identity
`state/transactions/<unique-id>/` retains before_source, before_build, input.zip and receipt.json.
`state/current_receipt.json` identifies the active committed operation. Rollback uses its exact
before-image, not a reused rev_0001 directory. Legacy numeric history is read only when no new receipt
exists. New snapshots are not automatically deleted; disk retention policy is future work.
The journal stores identity-derived internal paths, not arbitrary candidate/deletion paths.
User saves are not copied, deleted, rolled back or placed in candidate source/build trees.

## Failure policy
Unreadable, unknown or legacy schema-1 pending journals block operations and retain files. They are not
automatically cleared or guessed into schema 2. Missing/corrupt before-images and unexpected live bytes
also block. Both the primary error and recovery error reach the caller. The UI displays recovery required;
there is no best-effort `.ok()` conversion that relabels an uncertain workspace as safely recovered.
Journal replacement never first deletes the previous journal. The temporary file is synced, renamed in
the same directory, and parent directories are synced on Unix. Rust/Windows rename can fail under file
sharing restrictions; that failure retains the old journal. Network filesystems, controller cache failure,
hard power loss, malicious trusted project code and a second unrelated file editor are outside this proof.
Godot executes project code with user privileges; this coordinator is not a security sandbox.

## Evidence to run
`cargo test -p bridge-safety --features fault-injection`
- 17 boundaries for each of initialize/update/rollback/repair (68 cases).
- 10 recovery boundaries after interruption, for each operation (40 cases).
- Additional lock, invalid journal, validation rejection, archive failure, snapshot integrity,
  source UID/cache and divergent reapply tests.
The parent process kills test children at named boundaries without unwinding; a fresh Workspace then
recovers twice. Test build files are fixtures, not claims of Godot correctness. Release dependencies do
not enable fault-injection. Test loops have parent deadlines and never poll GitHub.
`python tools/test_safety_wiring.py` checks caller routing, not runtime correctness.
The existing Windows `bridge_pipeline_smoke` separately proves real Godot init/update/rollback/reapply.

## References
Rust rename: https://doc.rust-lang.org/std/fs/fn.rename.html
Rust file sync/OS locks: https://doc.rust-lang.org/std/fs/struct.File.html
These document primitives, not a universal power-loss guarantee for this application.
