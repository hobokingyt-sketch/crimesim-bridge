# PACK-1: focused managed-game handoffs

The app and the game have separate memories. `AGENTS.md` in this repository describes
Rust/Tauri development; generated game `AGENTS.md` describes Godot source. Export accepts
only a registry with `project_kind: game` and the active game's project ID.

## User flow
Choose a game area, describe the next task, and press Create Chat Pack. Upload the one
resulting ZIP. The developer reads START_HERE.md and task.json, then relevant source.
When more content is needed, chat supplies a JSON file_requests object. Paste that object
in Requested files from chat and create another pack; paths and SHA-256 must match the
current inventory. No file browsing or script editing is required from the user.

## Snapshot contract
`core/context` implements policy, registry, seed documents, source capture and ZIP writing.
`package.rs` acquires the recovered workspace lock before options/export. Game source,
revision, active executable and saves are not modified by export. New staged bootstraps
receive concise game instructions, a UI thesis, five module routes and small cards.
Existing projects without the registry use clearly labeled read-only legacy routes.
This fallback is generated handoff guidance, not a silent game migration. Custom or
malformed existing registries are never replaced with defaults. A deliberate future
update can add game memory to an old game using normal create operations.

The pack contains complete selected text files, startup source, direct dependency cards,
revision metadata, a sorted eligible-file inventory and per-file hashes. Other source
and binary assets are explicitly omitted and requestable. A missing source file is not
inferred from its absence in the ZIP. Selected source is never truncated to fit.
Default budgets: 384 KiB selected text, 128 KiB per default text file, 8 MiB explicitly
requested files, 16 user requests, 4,096 task bytes. Large required startup data rejects
the export. Source indexing is bounded at 20,000 files / 2 GiB eligible bytes and is
streamed; exceeding this limit rejects rather than silently producing an incomplete index.

Unknown areas, invalid paths, linked/reparse entries, case aliases, stale asset requests,
ambiguous ownership, cyclic dependencies and missing required cards produce explicit errors.
Godot .uid and source-adjacent .import metadata remain eligible source. Common generated,
save and credential filenames are excluded, including from the inventory; this is a
filename filter, not content-based secret detection. Do not store credentials in game code.

Eligible-file hashes are rechecked before atomic outgoing publication. This is not a Git
commit or cryptographic author signature. The workspace lock prevents other Bridge writers;
it is not protection against arbitrary hostile processes modifying the same user's files.
Captured validation evidence says whether its revision matches the source and retains its
passed/failed value. It must not be mistaken for guaranteed current-build health.
Context schema is 2; incoming update schema remains 1, with full replacement files/base hashes.

## Verification and extension
`cargo test --locked -p crimesim-bridge --lib core::context::tests` runs the production
exporter against temporary game fixtures. Node tests exercise the actual frontend script
with a mock DOM/backend. Installed acceptance exports an Interface pack and independently
reopens it to verify identity and every included payload hash. That native check is not a
claim that all mouse/keyboard interactions are covered.

No engine version, lockfile, recovery/watchdog behavior or gameplay system changes are
required. New ownership paths belong in the game registry; do not add a second copy of
simulation state inside the exporter. SAFE-3 incoming hostile ZIP validation and HEALTH-1
current-build health remain separate tasks.
