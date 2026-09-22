# CrimeSim Update Package v1

Filename convention:

`CrimeSim_Update_####.zip`

Required root file:

`bridge_manifest.json`

Changed and created files are stored under:

`changes/<project-relative-path>`

Deleted files have no payload.

## Manifest

```json
{
  "schema": 1,
  "package_type": "update",
  "project_id": "crime_sim",
  "engine_version": "4.7.2",
  "base_revision": 0,
  "target_revision": 1,
  "created_at": "2026-09-21T00:00:00Z",
  "summary": "Example update",
  "operations": [
    {
      "op": "replace",
      "path": "game/main.gd",
      "base_sha256": "<sha256-of-current-file>",
      "new_sha256": "<sha256-of-payload>"
    }
  ]
}
```

Operation rules:

- `create`: target must not exist. `new_sha256` required.
- `replace`: target must exist. `base_sha256` and `new_sha256` required.
- `delete`: target must exist. `base_sha256` required.
- Paths must be project-relative and may not contain traversal or absolute components.
- Bridge/generated paths such as `.godot`, `.git`, runtime, build, history, logs, and Bridge-owned project metadata are rejected.
- `target_revision` must equal `base_revision + 1` in v1.

The Bridge verifies every declared payload before staging. Full replacement files are used instead of line-number patches so that update behavior is deterministic.

## v0.2 application behavior

The package schema remains v1. Bridge v0.2 hardens how that package is consumed rather than changing the transport format.

- The package is bound to the exact current revision and per-file base hashes.
- Candidate files are written only into staging.
- A durable transaction journal exists before live promotion.
- A revision cannot commit unless Godot import/smoke/export gates pass and a playable Windows build exists.
- Rejected packages are removed from the ordinary incoming path and placed in Bridge quarantine with a structured reason file.
- Interrupted uncommitted promotion restores the prior source and playable build on the next Bridge launch.

## v0.25 terminal package states

The transport schema remains v1.

After processing, a package must have one unambiguous terminal state:

- **Applied** — the source/build revision committed successfully. The ZIP is moved out of Downloads/incoming discovery into the Bridge `applied` archive.
- **Rejected** — verification, validation, export or transaction work failed. The ZIP moves into `quarantine` with a structured reason.
- **Pending** — the ZIP is still in Downloads/incoming and has not reached a terminal state.

A successfully applied package must not remain discoverable as the "latest update", because replaying an already-consumed base revision is both confusing and unsafe.
