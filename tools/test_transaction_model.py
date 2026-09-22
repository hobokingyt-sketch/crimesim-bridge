from __future__ import annotations
import hashlib, json, tempfile, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "fixtures/CrimeSim_Update_0001.zip"
base_text = b"# Criminal Simulation\n\nBridge bootstrap. The map is the world; UI is the command OS.\n"
sha = lambda b: hashlib.sha256(b).hexdigest()

with tempfile.TemporaryDirectory() as td:
    project = Path(td) / "project"
    target = project / "project_control/PROJECT.md"
    target.parent.mkdir(parents=True)
    target.write_bytes(base_text)

    with zipfile.ZipFile(FIXTURE) as z:
        manifest = json.loads(z.read("bridge_manifest.json"))
        assert manifest["base_revision"] == 0 and manifest["target_revision"] == 1
        op = manifest["operations"][0]
        assert op["op"] == "replace"
        assert sha(target.read_bytes()) == op["base_sha256"]
        payload = z.read("changes/" + op["path"])
        assert sha(payload) == op["new_sha256"]
        target.write_bytes(payload)
        assert sha(target.read_bytes()) == op["new_sha256"]

print("TRANSACTION FIXTURE PASSED")
