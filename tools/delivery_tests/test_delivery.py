from __future__ import annotations
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('delivery', Path(__file__).parents[1] / 'delivery.py')
delivery = importlib.util.module_from_spec(spec)
spec.loader.exec_module(delivery)


class DeliveryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.target = self.root / 'custom output'
        self.folder = self.target / 'release/bundle/nsis'
        self.folder.mkdir(parents=True)
        self.installer = self.folder / 'CrimeSim_0.2.5_x64-setup.exe'
        self.installer.write_bytes(b'MZ' + b'0' * 65536)
        self.metadata = {'target_directory': str(self.target), 'packages': [{'name': 'crimesim-bridge', 'version': '0.2.5'}]}
        self.lock = self.root / 'Cargo.lock'
        self.lock.write_text('# fixture dependency lock\n')
        self.provenance = {'source_commit': 'a' * 40, 'source_tree': 'b' * 40, 'cargo_lock_sha256': delivery.digest(self.lock)}
        self.report = {'ok': True, 'frontend_ready': True, 'runtime_verified': True,
                       'reinstall_preserved_data': True, 'uninstall_preserved_data': True,
                       'source_revision': 1, 'playable_revision': 1,
                       'bridge_version': '0.2.5', 'source_commit': 'a' * 40,
                       'installer_sha256': delivery.digest(self.installer),
                       'checks': ['native_frontend_ipc', 'bundled_runtime', 'initialize', 'update', 'exported_launch', 'rollback', 'reapply', 'context_pack', 'save_canary']}
        self.output = self.root / 'release'

    def assemble(self):
        return delivery.assemble(self.metadata, self.report, self.provenance, self.lock, self.output)

    def test_metadata_drives_output_path(self):
        self.assertEqual(delivery.discover(self.metadata), self.installer.resolve())

    def test_relative_metadata_rejected(self):
        self.metadata['target_directory'] = 'target'
        with self.assertRaises(ValueError): delivery.discover(self.metadata)

    def test_no_stale_guess_when_missing(self):
        self.installer.unlink()
        with self.assertRaises(ValueError): delivery.discover(self.metadata)

    def test_ambiguous_installers_rejected(self):
        (self.folder / 'old-setup.exe').write_bytes(self.installer.read_bytes())
        with self.assertRaises(ValueError): delivery.discover(self.metadata)

    def test_tiny_or_non_executable_rejected(self):
        for data in (b'MZ', b'not exe' * 10000):
            self.installer.write_bytes(data)
            with self.assertRaises(ValueError): delivery.discover(self.metadata)

    def test_every_acceptance_boolean_must_be_true(self):
        for field in ('ok', 'frontend_ready', 'runtime_verified', 'reinstall_preserved_data', 'uninstall_preserved_data'):
            for value in (False, None, 'true', 1):
                self.report[field] = value
                with self.assertRaises(ValueError): self.assemble()
                self.assertFalse(self.output.exists())
            self.report[field] = True

    def test_revision_must_be_integer_not_boolean(self):
        self.report['source_revision'] = True
        with self.assertRaises(ValueError): self.assemble()

    def test_report_binds_commit(self):
        self.report['source_commit'] = 'c' * 40
        with self.assertRaises(ValueError): self.assemble()

    def test_report_binds_version(self):
        self.report['bridge_version'] = '0.2.4'
        with self.assertRaises(ValueError): self.assemble()

    def test_report_binds_installer(self):
        self.installer.write_bytes(b'MZ' + b'x' * 65536)
        with self.assertRaises(ValueError): self.assemble()

    def test_lock_binds_provenance(self):
        self.lock.write_text('changed lock')
        with self.assertRaises(ValueError): self.assemble()

    def test_incomplete_native_check_rejected(self):
        self.report['checks'].remove('native_frontend_ipc')
        with self.assertRaises(ValueError): self.assemble()

    def test_no_reuse_of_output_directory(self):
        self.output.mkdir()
        (self.output/'old.txt').write_text('retain')
        with self.assertRaises(ValueError): self.assemble()
        self.assertEqual((self.output/'old.txt').read_text(), 'retain')

    def test_complete_package_keeps_verified_bytes(self):
        result = self.assemble()
        self.assertEqual(delivery.digest(self.output / self.installer.name), result['sha256'])
        self.assertEqual(json.loads((self.output/'release_manifest.json').read_text()), result)
        self.assertEqual(result['authenticode'], 'unsigned-development-build')
        self.assertTrue((self.output / 'Cargo.lock').is_file())

if __name__ == '__main__': unittest.main()
