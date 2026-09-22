from __future__ import annotations
import hashlib, json, zipfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "fixtures" / "CrimeSim_Update_0001.zip"

base = "# Criminal Simulation\n\nBridge bootstrap. The map is the world; UI is the command OS.\n".encode()
new = "# Criminal Simulation\n\nBridge fixture revision 1. The map remains the world; UI remains the command OS.\n".encode()
sha = lambda b: hashlib.sha256(b).hexdigest()
manifest = {
    "schema": 1,
    "package_type": "update",
    "project_id": "crime_sim",
    "engine_version": "4.7.2",
    "base_revision": 0,
    "target_revision": 1,
    "created_at": datetime.now(timezone.utc).isoformat(),
    "summary": "Fixture update proving a full-file replacement package.",
    "operations": [{
        "op": "replace",
        "path": "project_control/PROJECT.md",
        "base_sha256": sha(base),
        "new_sha256": sha(new),
    }],
}
OUT.parent.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(OUT, "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("bridge_manifest.json", json.dumps(manifest, indent=2))
    z.writestr("changes/project_control/PROJECT.md", new)
print(OUT)
