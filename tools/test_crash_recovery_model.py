from __future__ import annotations
import json, shutil, tempfile
from pathlib import Path


def write_rev(root: Path, rev: int, kind: str):
    root.mkdir(parents=True, exist_ok=True)
    if kind == 'source':
        pc = root / 'project_control'
        pc.mkdir(parents=True, exist_ok=True)
        (pc / 'bridge_project.json').write_text(json.dumps({'revision': rev}))
        (root / 'marker.txt').write_text(f'source-{rev}')
    else:
        (root / 'bridge_revision.txt').write_text(str(rev))
        (root / 'CrimeSim.exe').write_bytes(f'build-{rev}'.encode())


def copytree(src: Path, dst: Path):
    if dst.exists(): shutil.rmtree(dst)
    shutil.copytree(src, dst)

with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    current_source = root / 'workspace/current_project'
    history_source = root / 'history/source/rev_0007'
    current_build = root / 'builds/current'
    history_build = root / 'history/builds/rev_0007'

    write_rev(history_source, 7, 'source')
    write_rev(history_build, 7, 'build')
    write_rev(current_source, 8, 'source')
    write_rev(current_build, 8, 'build')

    # Model the Bridge rule for any uncommitted crash: target revision is discarded.
    copytree(history_source, current_source)
    copytree(history_build, current_build)

    source_rev = json.loads((current_source/'project_control/bridge_project.json').read_text())['revision']
    build_rev = int((current_build/'bridge_revision.txt').read_text())
    assert source_rev == 7
    assert build_rev == 7
    assert (current_source/'marker.txt').read_text() == 'source-7'
    assert (current_build/'CrimeSim.exe').read_bytes() == b'build-7'

print('CRASH RECOVERY MODEL PASSED')
