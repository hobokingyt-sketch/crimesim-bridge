"""Bounded source handoffs with explicit provenance and no execution of metadata commands."""
from __future__ import annotations
import json
import os
import tempfile
import zipfile
from pathlib import Path
from .index import Repo, digest, file_digest, sensitive


def brief(repo: Repo, selected: list[str]) -> str:
    selected = repo.select(selected, [])
    task = next(i for i in repo.backlog['items'] if i['id'] == repo.current['task_id'])
    lines = [repo.path('AGENTS.md').read_text(encoding='utf-8').rstrip(),
             '\n## Current checkpoint (recheck remote HEAD/CI before work)',
             f'Task: {repo.current["task_id"]} / {task["status"]}',
             repo.current['summary'],
             'Next: ' + ', '.join(repo.current['next_task_ids'])]
    if not selected:
        lines += ['\n## Choose a module; do not read every source file']
        lines += [f'- {mid}: {m["purpose"]} ({m["card"]})' for mid, m in repo.modules.items()]
    for mid in selected:
        m = repo.modules[mid]
        lines += ['\n' + repo.path(m['card']).read_text(encoding='utf-8').rstrip(),
                  'Source: ' + ', '.join(repo.owned(mid)),
                  'Read dependency contracts as needed: ' + ', '.join(repo.modules[d]['card'] for d in m['depends_on']),
                  'Check IDs: ' + ', '.join(m['checks'])]
    if selected:
        lines += ['\n## Relevant open work']
        lines += [f'- {i["id"]} ({i["status"]}): {i["problem"]} Acceptance: {i["done_when"]}'
                  for i in repo.backlog['items'] if set(i['modules']) & set(selected) and i['status'] != 'verified']
        lines += ['\n## Relevant accepted reasoning']
        lines += [f'- {d["id"]}: {d["decision"]} Reason: {d["reason"]}'
                  for d in repo.decisions['decisions'] if d['status'] == 'accepted' and set(d['modules']) & set(selected)]
    text = '\n'.join(lines) + '\n'
    if len(text) > 24000:
        raise ValueError('Brief exceeds 24000 characters. Select fewer modules; nothing was truncated.')
    return text


def pack(repo: Repo, selected: list[str], output: Path, budget: int = 196608,
         working_copy: bool = False) -> dict:
    if not selected:
        raise ValueError('Select at least one module; whole-repository dumps are not the default')
    if budget < 1024 or budget > 16 * 1024 * 1024:
        raise ValueError('Budget must be between 1 KiB and 16 MiB')
    report = repo.check()
    if not report['ok']:
        raise ValueError('Memory checks failed: ' + '; '.join(report['errors']))
    output = output.resolve()
    if output.is_relative_to(repo.root):
        raise ValueError('Write handoffs outside the source repository')
    if output.exists():
        raise ValueError('Output already exists; choose a new name')
    status = repo.git('status', '--porcelain', '--untracked-files=all')
    dirty = bool(status.strip()) if status is not None else None
    if dirty and not working_copy:
        raise ValueError('Working tree is dirty. Commit first or explicitly label --working-copy')
    head = repo.git('rev-parse', 'HEAD')
    selected = repo.select(selected, [])
    required = {'AGENTS.md', 'project_control/PROJECT.json', 'project_control/CURRENT.json',
                'project_control/MODULES.json'}
    related = set(selected)
    for mid in selected:
        related.update(repo.modules[mid]['depends_on'])
    required.update(repo.modules[m]['card'] for m in related)
    desired = set(required)
    for mid in selected:
        desired.update(repo.owned(mid))
    # Freeze bytes once; later mutations cannot change the payload behind its hash.
    data: dict[str, bytes] = {}
    inventory = []
    names = repo.inventory()
    for name in names:
        if sensitive(name):
            inventory.append({'path': name, 'included': False, 'reason': 'sensitive_filename'})
            continue
        path = repo.path(name)
        size = path.stat().st_size
        if name in desired and size <= budget:
            data[name] = path.read_bytes()
        content_hash = digest(data[name]) if name in data else file_digest(path)
        inventory.append({'path': name, 'bytes': size, 'sha256': content_hash, 'included': False,
                          'reason': 'content_budget' if name in desired else 'outside_selected_scope'})
    start = brief(repo, selected).encode('utf-8')
    if required - data.keys():
        raise ValueError('A required context file exceeds budget or is unavailable')
    total = len(start) + sum(len(data[n]) for n in required)
    if total > budget:
        raise ValueError('Required context exceeds budget; increase budget or select fewer modules')
    included = set(required)
    for name in sorted(desired - required):
        if name not in data:
            continue
        if total + len(data[name]) <= budget:
            total += len(data[name])
            included.add(name)
    for row in inventory:
        name = row['path']
        if name in included:
            row.update(included=True, reason='selected_source_or_contract')
        elif name in data:
            row['reason'] = 'content_budget'
    after = repo.git('status', '--porcelain', '--untracked-files=all')
    after_head = repo.git('rev-parse', 'HEAD')
    if after != status or after_head != head or repo.inventory() != names:
        raise ValueError('Repository changed while assembling handoff; retry from a stable snapshot')
    # A dirty file can change without changing porcelain output. Compare bytes as well.
    for row in inventory:
        if 'sha256' in row and file_digest(repo.path(row['path'])) != row['sha256']:
            raise ValueError(f'File changed while assembling handoff: {row["path"]}')
    manifest = {'schema': 1, 'package_type': 'bridge_source_context',
                'repository': repo.project['repository'], 'git_head': head.strip() if head else None,
                'working_tree_dirty': dirty, 'committed_snapshot': bool(head and dirty is False),
                'selected_modules': selected, 'content_bytes': total, 'content_budget_bytes': budget,
                'read_first': 'START_HERE.md', 'files': inventory,
                'note': 'Context only, not a game update ZIP. Omitted contents are unknown; fetch before editing.'}
    output.parent.mkdir(parents=True, exist_ok=True)
    fd, temp = tempfile.mkstemp(prefix='bridge-context-', suffix='.tmp', dir=output.parent)
    os.close(fd)
    try:
        with zipfile.ZipFile(temp, 'w', zipfile.ZIP_DEFLATED) as archive:
            archive.writestr('START_HERE.md', start)
            archive.writestr('context_manifest.json', json.dumps(manifest, indent=2))
            for name in sorted(included):
                archive.writestr('source/' + name, data[name])
        # Exclusive destination avoids accidentally replacing another handoff.
        with output.open('xb') as dest, open(temp, 'rb') as src:
            while chunk := src.read(128 * 1024):
                dest.write(chunk)
    finally:
        Path(temp).unlink(missing_ok=True)
    return {'path': str(output), 'selected_modules': selected, 'included_files': len(included),
            'omitted_files': sum(not r['included'] for r in inventory), 'content_bytes': total,
            'archive_sha256': digest(output.read_bytes())}
