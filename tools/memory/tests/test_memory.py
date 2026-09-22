"""Behavior tests for routing, consistency, scoped archives and failure handling."""
from __future__ import annotations
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools'))
from memory.index import Repo, digest
from memory.handoff import brief, pack


class MemoryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'repo'
        shutil.copytree(ROOT, self.root, ignore=shutil.ignore_patterns('.git', 'target', '__pycache__', '.memory-output'))
        self.repo = Repo(self.root)
        self.output = Path(self.temp.name) / 'context.zip'

    def mutate(self, name, fn):
        p = self.root / name
        v = json.loads(p.read_text(encoding='utf-8'))
        fn(v)
        p.write_text(json.dumps(v), encoding='utf-8')
        self.repo = Repo(self.root)

    def assertBad(self, text):
        r = self.repo.check()
        self.assertFalse(r['ok'])
        self.assertIn(text, '\n'.join(r['errors']))

    def test_baseline_consistent(self):
        self.assertTrue(self.repo.check()['ok'])

    def test_missing_card(self):
        (self.root / 'docs/modules/engine.md').unlink()
        self.assertBad('Missing module card')

    def test_unregistered_file(self):
        (self.root / 'surprise.gd').write_text('extends Node')
        self.assertBad('Expected one registered owner')

    def test_duplicate_owner(self):
        self.mutate('project_control/MODULES.json', lambda v: v['modules'][0]['owns'].append('web/*'))
        self.assertBad('Expected one registered owner')

    def test_dead_exact_path(self):
        self.mutate('project_control/MODULES.json', lambda v: v['modules'][0]['owns'].append('missing.rs'))
        self.assertBad('Dead exact source reference')

    def test_missing_dependency(self):
        self.mutate('project_control/MODULES.json', lambda v: v['modules'][0]['depends_on'].append('imaginary'))
        self.assertBad('Invalid dependency')

    def test_unknown_check(self):
        self.mutate('project_control/MODULES.json', lambda v: v['modules'][0]['checks'].append('imaginary'))
        self.assertBad('Unknown check')

    def test_bad_task_reference(self):
        self.mutate('project_control/CURRENT.json', lambda v: v.update(task_id='MISSING'))
        self.assertBad('unknown task')

    def test_false_verified_without_evidence(self):
        self.mutate('project_control/BACKLOG.json', lambda v: v['items'][0].update(status='verified', verification=None))
        self.assertBad('Verified task has no evidence')

    def test_missing_acceptance(self):
        self.mutate('project_control/BACKLOG.json', lambda v: v['items'][0].update(done_when=''))
        self.assertBad('acceptance')

    def test_missing_evidence_source(self):
        self.mutate('project_control/BACKLOG.json', lambda v: v['items'][0].update(evidence_path='not-here.py'))
        self.assertBad('evidence path missing')

    def test_superseded_decision_needs_replacement(self):
        self.mutate('project_control/DECISIONS.json', lambda v: v['decisions'][0].update(status='superseded'))
        self.assertBad('no replacement')

    def test_size_is_warning_not_gate(self):
        p = self.root / 'AGENTS.md'
        p.write_text(p.read_text(encoding='utf-8') + ' context' * 700, encoding='utf-8')
        r = self.repo.check()
        self.assertTrue(r['ok'])
        self.assertTrue(any('650 words' in w for w in r['warnings']))

    def test_changed_source_warns_not_freezes(self):
        reviews = {'schema': 1, 'modules': {m: {'source_sha256': self.repo.source_digest(m)} for m in self.repo.modules}}
        (self.root / 'project_control/REVIEWS.json').write_text(json.dumps(reviews), encoding='utf-8')
        with (self.root / 'web/app.js').open('a', encoding='utf-8') as f:
            f.write('\n// Test-only comment\n')
        r = self.repo.check()
        self.assertTrue(r['ok'])
        self.assertIn('Source changed since recorded review: shell', r['warnings'])

    def test_specific_path_routes_to_owner(self):
        self.assertEqual(self.repo.select([], ['web/app.js']), ['shell'])

    def test_unknown_module_does_not_dump_all(self):
        with self.assertRaises(ValueError):
            brief(self.repo, ['unknown'])

    def test_brief_is_bounded_and_scoped(self):
        b = brief(self.repo, ['packages'])
        self.assertLess(len(b), 24000)
        self.assertIn('PACK-1', b)
        self.assertNotIn('pub fn bootstrap_demo', b)

    def test_impact_tracks_consumers(self):
        r = self.repo.impact(['src-tauri/src/core/package.rs'])
        self.assertEqual(r['direct_modules'], ['packages'])
        self.assertIn('transactions', r['review_consumers'])
        self.assertIn('shell', r['review_consumers'])

    def test_unsafe_paths(self):
        for p in ['../outside', '/outside', 'C:/outside', 'x/../y', 'x\\y', './x']:
            with self.subTest(path=p), self.assertRaises(ValueError):
                self.repo.path(p)

    def test_symlink_rejected(self):
        p = self.root / 'web/link.js'
        try:
            p.symlink_to(self.root / 'web/app.js')
        except OSError:
            self.skipTest('Host does not permit symlink creation')
        with self.assertRaises(ValueError):
            self.repo.path('web/link.js')

    def test_scoped_pack_hashes_and_omissions(self):
        r = pack(self.repo, ['packages'], self.output)
        with zipfile.ZipFile(self.output) as z:
            m = json.loads(z.read('context_manifest.json'))
            self.assertEqual(m['package_type'], 'bridge_source_context')
            self.assertIsNone(m['git_head'])
            self.assertFalse(m['committed_snapshot'])
            self.assertIn('source/src-tauri/src/core/package.rs', z.namelist())
            self.assertNotIn('source/src-tauri/src/core/godot.rs', z.namelist())
            for row in m['files']:
                if row['included']:
                    self.assertEqual(digest(z.read('source/' + row['path'])), row['sha256'])
            self.assertGreater(r['omitted_files'], 0)

    def test_budget_omits_whole_files(self):
        (self.root / 'web/huge.js').write_bytes(b'a' * 100000)
        pack(self.repo, ['shell'], self.output, budget=40000)
        with zipfile.ZipFile(self.output) as z:
            m = json.loads(z.read('context_manifest.json'))
            row = next(f for f in m['files'] if f['path'] == 'web/huge.js')
            self.assertFalse(row['included'])
            self.assertEqual(row['reason'], 'content_budget')
            self.assertNotIn('source/web/huge.js', z.namelist())

    def test_too_small_budget_fails(self):
        with self.assertRaises(ValueError):
            pack(self.repo, ['packages'], self.output, budget=1024)
        self.assertFalse(self.output.exists())

    def test_do_not_overwrite_handoff(self):
        self.output.write_text('keep')
        with self.assertRaises(ValueError):
            pack(self.repo, ['packages'], self.output)
        self.assertEqual(self.output.read_text(), 'keep')

    def test_output_not_in_source(self):
        with self.assertRaises(ValueError):
            pack(self.repo, ['packages'], self.root / 'context.zip')

    def test_module_required(self):
        with self.assertRaises(ValueError):
            pack(self.repo, [], self.output)

    def test_sensitive_filename_not_in_payload(self):
        (self.root / 'web/.env').write_text('TEST_SECRET=do-not-pack')
        pack(self.repo, ['shell'], self.output)
        with zipfile.ZipFile(self.output) as z:
            self.assertNotIn('source/web/.env', z.namelist())
            self.assertFalse(any(b'TEST_SECRET' in z.read(n) for n in z.namelist()))

    def test_metadata_commands_are_not_executed(self):
        self.mutate('project_control/MODULES.json', lambda v: v['checks']['memory'].update(commands=['INVALID_PROGRAM_DO_NOT_EXECUTE']))
        pack(self.repo, ['packages'], self.output)
        self.assertTrue(self.output.is_file())

    def test_source_mutation_during_pack_is_detected(self):
        original = self.repo.git
        calls = 0
        def changed(*args):
            nonlocal calls
            if args and args[0] == 'status':
                calls += 1
                if calls == 2:
                    (self.root / 'web/app.js').write_text('changed')
            return original(*args)
        with patch.object(self.repo, 'git', side_effect=changed), self.assertRaisesRegex(ValueError, 'changed while'):
            pack(self.repo, ['packages'], self.output)
        self.assertFalse(self.output.exists())

    def test_dirty_git_context_is_explicit(self):
        for args in [('init', '-q'), ('config', 'user.name', 'Test'), ('config', 'user.email', 'test@example.invalid'),
                     ('add', '.'), ('commit', '-qm', 'fixture')]:
            subprocess.run(['git', '-C', str(self.root), *args], check=True, capture_output=True)
        (self.root / 'web/app.js').write_text('changed')
        with self.assertRaisesRegex(ValueError, 'dirty'):
            pack(self.repo, ['packages'], self.output)
        pack(self.repo, ['packages'], self.output, working_copy=True)
        with zipfile.ZipFile(self.output) as z:
            m = json.loads(z.read('context_manifest.json'))
            self.assertTrue(m['working_tree_dirty'])
            self.assertFalse(m['committed_snapshot'])

    def test_cli_unknown_module_is_nonzero(self):
        result = subprocess.run([sys.executable, str(self.root / 'tools/project_memory.py'),
                                 '--root', str(self.root), 'brief', '--module', 'missing'], capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b'Unknown module', result.stderr)


if __name__ == '__main__':
    unittest.main()
