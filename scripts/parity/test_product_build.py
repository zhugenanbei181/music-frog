"""Build provenance, actual dependency impacts and immutable executable/resource retention."""
import json
import os
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from product_build import archive, build_id, dep_paths, impact, verify


class ProductBuildTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='product build ')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.source = self.root / 'crates/iced/src/main.rs'
        self.source.parent.mkdir(parents=True)
        self.source.write_text('fn main() {}')
        self.binary = self.root / 'target/debug/infiltrator-iced'
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b'original executable')
        escaped = str(self.source).replace(' ', '\\ ')
        self.binary.with_suffix('.d').write_text(f'{self.binary}: {escaped}\n')
        self.manifest = self.root / 'product.json'
        self.addCleanup(patch.stopall)
        patch('product_build.subprocess.check_output', return_value='rustc fixture host').start()

    def test_make_escapes_and_continuations_preserve_real_paths(self):
        self.assertEqual(dep_paths('binary: /src/with\\ space.rs \\\n /src/other.rs\n'),
                         ['/src/with space.rs', '/src/other.rs'])
        with self.assertRaises(ValueError):
            dep_paths('no dependency target')

    def test_replacing_latest_executable_or_unrelated_sources_does_not_remove_passed_build(self):
        record = archive(self.root, 'iced', self.binary, self.manifest)
        self.binary.write_bytes(b'another product build')
        (self.root / 'README.md').write_text('changed documentation')
        other = self.root / 'crates/bevy/src/main.rs'
        other.parent.mkdir(parents=True)
        other.write_text('changed peer frontend')
        self.assertEqual(verify(self.root, self.manifest)['build_id'], record['build_id'])
        self.assertEqual(impact(self.root, record)['status'], 'unchanged')
        self.assertEqual((self.root / record['binary']).read_bytes(), b'original executable')
        self.source.write_text('fn main() { new_behavior(); }')
        self.assertEqual(impact(self.root, record)['changed_inputs'], ['crates/iced/src/main.rs'])
        self.assertEqual(impact(self.root, record)['status'], 'review_required')
        self.assertEqual(verify(self.root, self.manifest)['build_id'], record['build_id'])

    def test_cargo_target_cleanup_does_not_destroy_archived_build_evidence(self):
        record = archive(self.root, 'iced', self.binary, self.manifest)
        shutil.rmtree(self.root / 'target')
        self.assertEqual(verify(self.root, self.manifest)['build_id'], record['build_id'])
        self.assertEqual((self.root / record['binary']).read_bytes(), b'original executable')
        self.assertTrue((self.root / record['binary']).with_name('build.json').exists())

    def test_modified_or_cross_peer_archive_cannot_satisfy_build_identity(self):
        record = archive(self.root, 'iced', self.binary, self.manifest)
        with self.assertRaisesRegex(ValueError, 'another peer'):
            verify(self.root, self.manifest, 'bevy')
        archived = self.root / record['binary']
        archived.chmod(0o755)
        archived.write_bytes(b'altered archive')
        with self.assertRaisesRegex(ValueError, 'archived product binary'):
            verify(self.root, self.manifest)

    def test_missing_input_provenance_or_an_escaping_artifact_cannot_be_an_archive(self):
        record = archive(self.root, 'iced', self.binary, self.manifest)
        altered = dict(record, inputs={})
        altered['build_id'] = build_id(altered)
        self.manifest.write_text(json.dumps(altered))
        with self.assertRaisesRegex(ValueError, 'input provenance'):
            verify(self.root, self.manifest)
        altered = dict(record, binary='../outside-binary')
        self.manifest.write_text(json.dumps(altered))
        with self.assertRaisesRegex(ValueError, 'escapes repository'):
            verify(self.root, self.manifest)

    def test_a_newer_compiled_input_requires_a_rebuild_for_new_evidence(self):
        timestamp = self.binary.stat().st_mtime_ns + 1_000_000
        os.utime(self.source, ns=(timestamp, timestamp))
        with self.assertRaisesRegex(ValueError, 'compiled input changed'):
            archive(self.root, 'iced', self.binary, self.manifest)

    def test_runtime_resource_is_archived_and_changes_only_request_reverification(self):
        self.binary = self.binary.with_name('infiltrator-bevy-ui')
        icon = self.root / 'crates/infiltrator-bevy-widgets/assets/icons/one.png'
        icon.parent.mkdir(parents=True)
        icon.write_bytes(b'original icon')
        self.binary.write_bytes(b'bevy executable')
        self.binary.with_suffix('.d').write_text(f'bevy: {str(self.source).replace(" ", chr(92)+" ")}\n')
        record = archive(self.root, 'bevy', self.binary, self.manifest)
        icon.write_bytes(b'new runtime icon')
        self.assertEqual(impact(self.root, record)['changed_inputs'], [str(icon.relative_to(self.root))])
        self.assertEqual(verify(self.root, self.manifest)['build_id'], record['build_id'])
        frozen_icon = self.root / record['runtime'] / 'assets/icons/one.png'
        frozen_icon.write_bytes(b'forged frozen icon')
        with self.assertRaisesRegex(ValueError, 'runtime resource'):
            verify(self.root, self.manifest)
