"""Read-only repository inventory and objective memory checks (standard library only)."""
from __future__ import annotations
import fnmatch
import hashlib
import json
import re
import subprocess
from pathlib import Path, PurePosixPath

SKIP_PARTS = {'.git', '.godot', 'target', '__pycache__', 'node_modules', '.memory-output', 'gen'}
SECRET_NAMES = {'.env', '.env.local', '.env.production', 'credentials.json', 'export_credentials.cfg'}


def sensitive(path: str) -> bool:
    name = PurePosixPath(path).name.lower()
    return (name in SECRET_NAMES or name.startswith('.env.') or
            name.endswith(('.pem', '.key', '.p12', '.pfx', '.keystore')))


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def file_digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open('rb') as src:
        while chunk := src.read(128 * 1024):
            h.update(chunk)
    return h.hexdigest()


class Repo:
    def __init__(self, root: Path):
        self.root = root.resolve()
        self.project = self.read_json('project_control/PROJECT.json')
        self.registry = self.read_json('project_control/MODULES.json')
        self.current = self.read_json('project_control/CURRENT.json')
        self.backlog = self.read_json('project_control/BACKLOG.json')
        self.decisions = self.read_json('project_control/DECISIONS.json')
        self.modules = {m['id']: m for m in self.registry['modules']}

    def path(self, relative: str) -> Path:
        if not relative or '\\' in relative or ':' in relative:
            raise ValueError(f'Unsafe repository path: {relative!r}')
        p = PurePosixPath(relative)
        if p.is_absolute() or '..' in p.parts or '.' in relative.split('/'):
            raise ValueError(f'Unsafe repository path: {relative!r}')
        target = self.root.joinpath(*p.parts)
        for part in (target, *target.parents):
            if part == self.root:
                break
            if part.is_symlink():
                raise ValueError(f'Symlink not accepted: {relative}')
        if not target.resolve().is_relative_to(self.root):
            raise ValueError(f'Path escapes repository: {relative}')
        return target

    def read_json(self, relative: str) -> dict:
        value = json.loads(self.path(relative).read_text(encoding='utf-8'))
        if not isinstance(value, dict) or value.get('schema') != 1:
            raise ValueError(f'Unsupported memory schema: {relative}')
        return value

    def git(self, *args: str) -> str | None:
        try:
            probe = subprocess.run(['git', '-C', str(self.root), 'rev-parse', '--show-toplevel'],
                                   capture_output=True, timeout=15, check=False)
            if probe.returncode or Path(probe.stdout.decode('utf-8').strip()).resolve() != self.root:
                return None
            result = subprocess.run(['git', '-C', str(self.root), *args],
                                    capture_output=True, timeout=15, check=False)
        except (OSError, subprocess.TimeoutExpired):
            return None
        return result.stdout.decode('utf-8') if result.returncode == 0 else None

    def inventory(self) -> list[str]:
        tracked = self.git('ls-files', '--cached', '--others', '--exclude-standard', '-z')
        if tracked is None:
            # Source ZIP mode is explicitly unversioned. Never infer a remote commit.
            names = [p.relative_to(self.root).as_posix() for p in self.root.rglob('*')
                     if p.is_file() or p.is_symlink()]
        else:
            names = tracked.split('\0')
        return sorted({n for n in names if n and not (set(PurePosixPath(n).parts) & SKIP_PARTS)})

    def owners(self, path: str) -> list[str]:
        return [m['id'] for m in self.modules.values()
                if any(fnmatch.fnmatchcase(path, pattern) for pattern in m['owns'])]

    def owned(self, module: str, files: list[str] | None = None) -> list[str]:
        if module not in self.modules:
            raise ValueError(f'Unknown module: {module}')
        return [p for p in (files if files is not None else self.inventory()) if module in self.owners(p)]

    def select(self, modules: list[str], paths: list[str]) -> list[str]:
        selected = set(modules)
        for path in paths:
            self.path(path)
            owners = self.owners(path)
            if len(owners) != 1:
                raise ValueError(f'Path must have exactly one owner: {path}: {owners}')
            selected.update(owners)
        unknown = selected - self.modules.keys()
        if unknown:
            raise ValueError('Unknown modules: ' + ', '.join(sorted(unknown)))
        return sorted(selected)

    def impact(self, paths: list[str]) -> dict:
        direct = set(self.select([], paths))
        affected = set(direct)
        while True:
            more = {m['id'] for m in self.modules.values() if set(m['depends_on']) & affected}
            if more <= affected:
                break
            affected |= more
        return {'changed_paths': paths, 'direct_modules': sorted(direct),
                'review_consumers': sorted(affected - direct),
                'note': 'Review candidates, not automatic permission gates or proof of dependency completeness.'}

    def source_digest(self, module: str) -> str:
        # Reviews cannot hash themselves. Hash relevant source/config, not prose history.
        rows = []
        for name in self.owned(module):
            if name.startswith('project_control/') or name.endswith('.md') or sensitive(name):
                continue
            path = self.path(name)
            if path.is_file():
                rows.append(f'{name}\0{file_digest(path)}')
        return digest('\n'.join(rows).encode())

    def check(self) -> dict:
        errors: list[str] = []
        warnings: list[str] = []
        files = self.inventory()
        if len(self.modules) != len(self.registry['modules']):
            errors.append('Duplicate module IDs')
        for field in ['memory_entry', 'version_source', 'engine_pin_source']:
            if not self.path(self.project[field]).is_file():
                errors.append(f'Project reference is missing: {field}')
        for name in files:
            try:
                p = self.path(name)
                if not p.is_file():
                    errors.append(f'Inventory path is missing or not a file: {name}')
            except ValueError as exc:
                errors.append(str(exc))
            owners = self.owners(name)
            if len(owners) != 1:
                errors.append(f'Expected one registered owner: {name}: {owners}')
            if sensitive(name):
                warnings.append(f'Sensitive filename excluded from handoffs: {name}')
        for mid, m in self.modules.items():
            if not self.owned(mid, files):
                errors.append(f'Module has no source: {mid}')
            for field in ['purpose', 'card', 'owns', 'checks']:
                if not m.get(field):
                    errors.append(f'Module {mid} has empty {field}')
            try:
                card = self.path(m['card'])
                if not card.is_file():
                    errors.append(f'Missing module card: {mid}: {m["card"]}')
                elif len(card.read_text(encoding='utf-8').split()) > 400:
                    warnings.append(f'Module card over 400 words: {mid}')
            except ValueError as exc:
                errors.append(str(exc))
            for pattern in m['owns']:
                if not any(c in pattern for c in '*?[') and pattern not in files:
                    # These files are legitimate planned inputs; not evidence they exist.
                    if pattern not in {'Cargo.lock', 'rust-toolchain.toml', '.gitattributes'}:
                        errors.append(f'Dead exact source reference: {mid}: {pattern}')
            for dep in m['depends_on']:
                if dep not in self.modules or dep == mid:
                    errors.append(f'Invalid dependency: {mid} -> {dep}')
            for check in m['checks']:
                if check not in self.registry['checks']:
                    errors.append(f'Unknown check: {mid}: {check}')
        for check, data in self.registry['checks'].items():
            if not data.get('commands') or not data.get('platform'):
                errors.append(f'Check missing commands/platform: {check}')
            for cmd in data.get('commands', []):
                for script in re.findall(r'(?:tools|web)/[^\s]+\.(?:py|js)', cmd):
                    if not self.path(script).is_file():
                        errors.append(f'Check points at missing script: {script}')
        items = self.backlog['items']
        ids = [i['id'] for i in items]
        if len(ids) != len(set(ids)):
            errors.append('Duplicate task IDs')
        statuses = {'open', 'in_progress', 'implemented_pending_ci', 'verified', 'deferred'}
        for item in items:
            if item['status'] not in statuses or not item.get('done_when'):
                errors.append(f'Task missing valid status/acceptance: {item["id"]}')
            if item['status'] == 'verified' and not item.get('verification'):
                errors.append(f'Verified task has no evidence: {item["id"]}')
            if not item.get('modules') or set(item['modules']) - self.modules.keys():
                errors.append(f'Task has invalid module route: {item["id"]}')
            if not self.path(item['evidence_path']).is_file():
                errors.append(f'Task evidence path missing: {item["id"]}')
        for task in [self.current['task_id'], *self.current['next_task_ids']]:
            if task not in ids:
                errors.append(f'Current checkpoint points at unknown task: {task}')
        records = self.decisions['decisions']
        dids = {d['id'] for d in records}
        if len(dids) != len(records):
            errors.append('Duplicate decision IDs')
        for d in records:
            if d['status'] not in {'accepted', 'proposed', 'superseded'} or not d.get('reason') or not d.get('tradeoff'):
                errors.append(f'Invalid decision record: {d["id"]}')
            if set(d['modules']) - self.modules.keys():
                errors.append(f'Decision has unknown module: {d["id"]}')
            if d.get('supersedes') and (d['supersedes'] not in dids or d['supersedes'] == d['id']):
                errors.append(f'Broken decision supersession: {d["id"]}')
            if d['status'] == 'superseded' and not any(x.get('supersedes') == d['id'] for x in records):
                errors.append(f'Superseded decision has no replacement: {d["id"]}')
        if len(self.path('AGENTS.md').read_text(encoding='utf-8').split()) > 650:
            warnings.append('AGENTS.md over 650 words; split task-specific guidance')
        if len(json.dumps(self.current)) > 6000:
            warnings.append('CURRENT.json exceeds 6000 characters; keep history in Git')
        review_path = self.path('project_control/REVIEWS.json')
        if review_path.exists():
            reviews = self.read_json('project_control/REVIEWS.json')['modules']
            for mid in self.modules:
                old = reviews.get(mid)
                if not old or old.get('source_sha256') != self.source_digest(mid):
                    warnings.append(f'Source changed since recorded review: {mid}')
        return {'ok': not errors, 'file_count': len(files), 'module_count': len(self.modules),
                'errors': errors, 'warnings': warnings,
                'scope': 'Structural consistency only; not semantic, compile, runtime or security certification.'}
