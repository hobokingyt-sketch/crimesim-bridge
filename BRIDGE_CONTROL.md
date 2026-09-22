# CrimeSim Bridge control file

> Project Armor I: begin at `AGENTS.md` and `project_control/CURRENT.json`.
> This document describes design intent and historical implementation, not blanket proof of safety.
> Open gaps and acceptance criteria are tracked in `project_control/BACKLOG.json`.

## Mission

Make chat-directed development of a normal multi-file Godot project behave like a safe one-button workflow for a non-coder.

## Non-negotiable invariants

1. Never apply an update package directly to live source.
2. Never accept replace/delete when the current file hash differs from the package base hash.
3. Never let an update package author Bridge-owned revision metadata.
4. Never promote a staged project after a failed required validation gate.
5. Source revision and playable revision are explicit and must converge before a transaction is committed.
6. A durable transaction journal must exist before mutation can affect live source/build state.
7. Any interrupted, uncommitted transaction restores the last committed source/build revision.
8. Preserve revision-addressable source and playable-build rollback material before promotion.
9. Treat ZIPs as transport only; project structure remains multi-file.
10. Godot is a private verified worker process; normal users do not open the editor.
11. Generated caches are disposable and excluded from authoritative source transport.
12. Prefer deterministic complete-file replacement over fuzzy patches.
13. Rejected packages must leave the normal incoming path and enter quarantine with a reason.
14. Successfully consumed packages must leave incoming/Downloads discovery and enter the Applied archive.
15. Never run validation/export with an unverified private Godot runtime.
16. Verify the Godot executable's reported version against the project engine version before validation.
17. A playable build is not valid merely because export succeeded; its exported executable must launch successfully in a bounded smoke run.
18. CI must exercise the real transaction pipeline with a real Godot runtime before packaging a Bridge release.

19. Short-lived Godot workers have parent-enforced deadlines and bounded output; ordinary gameplay does not.
20. Confirm that owned worker descendants have stopped before recovering or modifying staging. Retain unresolved worker guards and report cleanup required.

## Current focus

SAFE-2 worker deadlines and output bounds. See project_control/CURRENT.json for exact verification state and next tasks.

## Deferred

- Direct GitHub transport.
- Stable/dev channels.
- Automatic Bridge updater.
- Authenticated package signing bound to a remote/repository identity.
- Incremental/hardlink staging.
- Large-asset request packs.
