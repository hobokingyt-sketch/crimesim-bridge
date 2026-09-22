from __future__ import annotations
import hashlib, json, tempfile
from pathlib import Path

sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()

with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    files = {
        'godot.exe': b'godot-runtime',
        'editor_data/export_templates/4.7.2.stable/windows_debug_x86_64.exe': b'debug-template',
        'editor_data/export_templates/4.7.2.stable/windows_release_x86_64.exe': b'release-template',
    }
    entries = []
    for rel, data in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data)
        entries.append({'path': rel, 'sha256': sha(p)})
    manifest = {'schema': 1, 'engine_version': '4.7.2', 'files': entries}
    (root/'runtime_manifest.json').write_text(json.dumps(manifest))

    loaded = json.loads((root/'runtime_manifest.json').read_text())
    for item in loaded['files']:
        assert sha(root/item['path']) == item['sha256']
    (root/'godot.exe').write_bytes(b'corrupted')
    assert sha(root/'godot.exe') != loaded['files'][0]['sha256']

print('RUNTIME MANIFEST MODEL PASSED')
