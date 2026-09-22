"""Developer entry point for repository-owned memory. Zeno does not run this CLI."""
from __future__ import annotations
import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from memory.index import Repo
from memory.handoff import brief, pack


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    subs = parser.add_subparsers(dest='command', required=True)
    subs.add_parser('check')
    for name in ['brief', 'pack']:
        p = subs.add_parser(name)
        p.add_argument('--module', action='append', default=[])
        p.add_argument('--path', action='append', default=[])
        if name == 'pack':
            p.add_argument('--output', type=Path, required=True)
            p.add_argument('--budget-kib', type=int, default=192)
            p.add_argument('--working-copy', action='store_true')
    p = subs.add_parser('impact')
    p.add_argument('--base', required=True)
    p = subs.add_parser('review')
    p.add_argument('--module', action='append', required=True)
    p.add_argument('--note', required=True)
    args = parser.parse_args()
    try:
        repo = Repo(args.root)
        if args.command == 'check':
            report = repo.check()
            print(json.dumps(report, indent=2))
            return 0 if report['ok'] else 1
        if args.command == 'brief':
            print(brief(repo, repo.select(args.module, args.path)), end='')
            return 0
        if args.command == 'pack':
            result = pack(repo, repo.select(args.module, args.path), args.output,
                          args.budget_kib * 1024, args.working_copy)
        elif args.command == 'impact':
            # Resolve revision first; untrusted text never becomes a Git option.
            base = repo.git('rev-parse', '--verify', '--end-of-options', args.base + '^{commit}')
            if not base:
                raise ValueError('Base commit not available locally; fetch that exact revision first')
            changed = repo.git('diff', '--name-only', '-z', base.strip(), '--')
            if changed is None:
                raise ValueError('Could not compare requested base')
            result = repo.impact([x for x in changed.split('\0') if x])
        else:
            if not args.note.strip():
                raise ValueError('A review note is required')
            dest = repo.path('project_control/REVIEWS.json')
            result = repo.read_json('project_control/REVIEWS.json') if dest.exists() else {'schema': 1, 'modules': {}}
            for mid in repo.select(args.module, []):
                result['modules'][mid] = {'source_sha256': repo.source_digest(mid),
                                          'reviewed_at': datetime.now(timezone.utc).isoformat(),
                                          'note': args.note.strip(),
                                          'meaning': 'Developer review declaration; not automated semantic proof.'}
            tmp = dest.with_suffix('.tmp')
            tmp.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
            tmp.replace(dest)
        print(json.dumps(result, indent=2))
        return 0
    except (OSError, ValueError, KeyError, TypeError) as exc:
        print(f'MEMORY ERROR: {exc}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
