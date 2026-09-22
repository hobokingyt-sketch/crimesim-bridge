from __future__ import annotations
import json, pathlib, subprocess, sys, zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
errors = []

for rel in ['src-tauri/tauri.conf.json', 'src-tauri/capabilities/default.json']:
    try: json.loads((ROOT / rel).read_text(encoding='utf-8'))
    except Exception as exc: errors.append(f'{rel}: {exc}')

js = subprocess.run(['node', '--check', str(ROOT/'web/app.js')], capture_output=True, text=True)
if js.returncode: errors.append('web/app.js: ' + (js.stderr or js.stdout))

fixture = ROOT/'fixtures/CrimeSim_Update_0001.zip'
if not fixture.exists(): errors.append('fixture update missing')
else:
    try:
        with zipfile.ZipFile(fixture) as z:
            names = set(z.namelist())
            if 'bridge_manifest.json' not in names: errors.append('fixture: bridge_manifest.json missing')
            manifest = json.loads(z.read('bridge_manifest.json'))
            for op in manifest.get('operations', []):
                if op['op'] in ('create','replace') and 'changes/' + op['path'] not in names:
                    errors.append(f"fixture: missing changes/{op['path']}")
    except Exception as exc: errors.append(f'fixture: {exc}')

required_rs = [
    'lib.rs','core/types.rs','core/paths.rs','core/fsops.rs','core/project.rs','core/package.rs',
    'core/update.rs','core/godot.rs','core/runtime.rs','core/status.rs','core/play.rs','core/transaction.rs','core/pipeline.rs','bin/bridge_pipeline_smoke.rs'
]
for rel in required_rs:
    if not (ROOT/'src-tauri/src'/rel).exists(): errors.append(f'missing Rust module: {rel}')

cargo = (ROOT/'src-tauri/Cargo.toml').read_text()
conf = (ROOT/'src-tauri/tauri.conf.json').read_text()
if 'version = "0.2.5"' not in cargo: errors.append('Cargo version is not 0.2.5')
if '"version": "0.2.5"' not in conf: errors.append('Tauri version is not 0.2.5')

workflow = (ROOT/'.github/workflows/build-windows.yml').read_text()
for required in ['runtime_manifest.json', 'Get-FileHash', 'windows_debug_x86_64.exe', 'windows_release_x86_64.exe', 'bridge_pipeline_smoke', 'release_manifest.json']:
    if required not in workflow: errors.append(f'workflow missing runtime integrity element: {required}')

update = (ROOT/'src-tauri/src/core/update.rs').read_text()
for required in ['TransactionPhase::SourcePromoting', 'TransactionPhase::Committed', 'quarantine_update', 'build_history_root']:
    if required not in update: errors.append(f'update engine missing hardening element: {required}')

transaction = (ROOT/'src-tauri/src/core/transaction.rs').read_text()
for required in ['recover_incomplete', 'restore_source', 'restore_build']:
    if required not in transaction: errors.append(f'transaction engine missing: {required}')

if errors:
    print('VALIDATION FAILED')
    print('\n'.join(f'- {e}' for e in errors))
    sys.exit(1)
print('VALIDATION PASSED')
