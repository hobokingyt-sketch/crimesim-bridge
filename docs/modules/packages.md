# packages

## Responsibility
Game context ZIPs, update manifests, payload reads, incoming and terminal archives.

## Source route
Read package.rs, then core/context/mod.rs for PACK-1. Policy, registry and staged seed
content are separate modules. The exact ownership/check routes are in MODULES.json.

## Contract
Update bytes are captured in the transaction directory and revalidated before assembly.
Applied archive failures retain the committed journal for retry. Context options/export
acquire the recovered workspace lock. Outgoing exports do not edit game source or saves.
Selected source is complete; omitted paths/hashes/reasons remain discoverable. Requests
for omitted files are hash-bound, limited and cannot bypass the filename privacy filter.
Bridge repository memory and managed-game memory are distinct identities.
See docs/GAME_CONTEXT.md for read-only legacy routing, budgets and verification boundaries.

## Remaining work
Incoming ZIP size/duplicate/Windows-path attacks remain SAFE-3. Filename filters are not
secret scanners. Context status/report evidence is not proof of current playable health.
Do not bypass transaction/recovery ownership to make an export succeed.
