"""Altered images, stale source/binary, missing activation and wrong dimensions fail closed."""
import copy
import csv
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from PIL import Image

from visual_receipts import interaction_bounds, interaction_signature, pixel_signature, sha256, verified_receipts


class VisualReceiptTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / 'target/debug').mkdir(parents=True)
        (self.root / 'target/debug/infiltrator-iced').write_bytes(b'test binary')
        (self.root / 'confirm-standard').mkdir()
        Image.new('RGB', (1180,780), '#333333').save(self.root / 'confirm-standard/image.png')
        (self.root / 'confirm-standard/rendered-frame.png').write_bytes((self.root / 'confirm-standard/image.png').read_bytes())
        (self.root / 'confirm-standard/marker.log').write_text('CAPTURE_READY page=runtime skin=dark scenario=confirm activated=true bounds=100,100,200,80\n')
        self.signature = {'width': 1180, 'height': 780, 'edge_bbox': [1, 1, 100, 200],
                          'luma_grid_sha256': 'test-signature', 'luma_stddev': 14.5}
        self.row = {'feature_id': 'confirm', 'surface': 'iced', 'viewport': 'standard',
                    'image': 'confirm-standard/image.png', 'marker': 'confirm-standard/marker.log', 'sha256': sha256(self.root / 'confirm-standard/image.png'),
                    'pixel_signature': self.signature, 'interaction_bounds': [100.0,100.0,200.0,80.0],
                    'interaction_pixel_signature': {'width':200,'height':80,'luma_stddev':14.5}, 'source_fingerprint': 'source',
                    'binary_sha256': sha256(self.root / 'target/debug/infiltrator-iced'),
                    'app_pid': 42, 'window_id': '8', 'capture_manifest': 'capture.tsv'}
        (self.root / 'capture-metadata.txt').write_text(f"source_fingerprint=source\nbinary_sha256={self.row['binary_sha256']}\n")
        with (self.root / 'capture.tsv').open('w') as file:
            writer = csv.writer(file, delimiter='\t')
            writer.writerow(['scenario', 'status', 'sha256', 'app_pid', 'window_id'])
            writer.writerow(['confirm-standard', 'ok', self.row['sha256'], '42', '8'])
        self.row['marker_sha256'] = sha256(self.root / self.row['marker'])
        self.row['capture_manifest_sha256'] = sha256(self.root / self.row['capture_manifest'])
        self.row['capture_metadata_sha256'] = sha256(self.root / 'capture-metadata.txt')
        self.row['rendered_frame'] = 'confirm-standard/rendered-frame.png'
        self.row['rendered_frame_sha256'] = sha256(self.root / self.row['rendered_frame'])
        self.row['rendered_frame_mismatch'] = 0.0
        self.schema = 3
        self.path = self.root / 'receipts.json'
        self.addCleanup(patch.stopall)
        patch('visual_receipts.source_fingerprint', return_value='source').start()
        patch('visual_receipts.pixel_signature', return_value=self.signature).start()
        patch('visual_receipts.interaction_signature', return_value=self.row['interaction_pixel_signature']).start()

    def verify(self, rows=None):
        self.path.write_text(json.dumps({'schema_version': self.schema, 'receipts': rows or [self.row]}))
        return verified_receipts(self.path, self.root)

    def test_verification_binds_every_receipt_dimension(self):
        self.assertEqual(self.verify(), {('confirm', 'iced', 'standard')})

    def test_source_provenance_is_bound_to_the_capture_but_unrelated_worktree_changes_do_not_erase_it(self):
        self.row['source_fingerprint'] = 'forged'
        with self.assertRaisesRegex(ValueError, 'metadata mismatch'):
            self.verify()
        self.row['source_fingerprint'] = 'source'
        with patch('visual_receipts.source_fingerprint', return_value='unrelated worktree change'):
            self.assertEqual(self.verify(), {('confirm', 'iced', 'standard')})

    def frozen_product(self):
        from product_build import archive
        binary = self.root / 'target/debug/infiltrator-iced'
        source = self.root / 'crates/iced/src/main.rs'
        source.parent.mkdir(parents=True)
        source.write_text('fn main() {}')
        binary.write_bytes(b'test binary')
        binary.with_suffix('.d').write_text(f'iced: {source}\n')
        manifest = self.root / 'product.json'
        with patch('product_build.subprocess.check_output', return_value='test compiler'):
            record = archive(self.root, 'iced', binary, manifest)
        self.schema = 4
        self.row.update(build_id=record['build_id'], build_manifest='product.json',
                        build_manifest_sha256=sha256(manifest))
        (self.root / 'capture-metadata.txt').write_text(
            f"source_fingerprint=source\nbinary_sha256={self.row['binary_sha256']}\n"
            f"build_manifest=product.json\nbinary={record['binary']}\n")
        self.row['capture_metadata_sha256'] = sha256(self.root / 'capture-metadata.txt')
        return record, source

    def test_immutable_product_pixels_survive_latest_binary_and_source_changes(self):
        record, source = self.frozen_product()
        (self.root / 'target/debug/infiltrator-iced').write_bytes(b'new latest binary')
        source.write_text('new source needing impact review')
        self.assertEqual(self.verify(), {('confirm', 'iced', 'standard')})
        from product_build import impact
        self.assertEqual(impact(self.root, record)['status'], 'review_required')
        self.assertEqual(verified_receipts(self.path, self.root, {'iced': record['build_id']}),
                         {('confirm', 'iced', 'standard')})
        with self.assertRaisesRegex(ValueError, 'selected product build'):
            verified_receipts(self.path, self.root, {'iced': 'another build'})

    def test_archived_product_and_build_manifest_tampering_remains_rejected(self):
        record, _ = self.frozen_product()
        manifest = self.root / 'product.json'
        original = manifest.read_text()
        manifest.write_text(original + ' ')
        with self.assertRaisesRegex(ValueError, 'build manifest'):
            self.verify()
        manifest.write_text(original)
        binary = self.root / record['binary']
        binary.chmod(0o755)
        binary.write_bytes(b'altered archived executable')
        with self.assertRaisesRegex(ValueError, 'archived product binary'):
            self.verify()

    def test_modified_pixels_and_geometry_are_rejected(self):
        (self.root / 'confirm-standard/image.png').write_bytes(b'altered')
        with self.assertRaisesRegex(ValueError, 'altered capture pixels'):
            self.verify()
        self.row['sha256'] = sha256(self.root / 'confirm-standard/image.png')
        self.row['pixel_signature'] = dict(self.signature, edge_bbox=[9, 9, 99, 99])
        with self.assertRaisesRegex(ValueError, 'geometry'):
            self.verify()

    def test_legacy_binary_identity_remains_checked_without_a_repository_freshness_gate(self):
        (self.root / 'target/debug/infiltrator-iced').write_bytes(b'new binary')
        with self.assertRaisesRegex(ValueError, 'stale capture binary'):
            self.verify()

    def test_binary_replacement_during_its_digest_read_is_rejected(self):
        from visual_receipts import sha256 as real_digest
        binary = self.root / 'target/debug/infiltrator-iced'
        def changing_digest(path):
            result = real_digest(path)
            if path == binary:
                replacement = binary.with_suffix('.replacement')
                replacement.write_bytes(binary.read_bytes())
                replacement.replace(binary)
            return result
        with patch('visual_receipts.sha256', side_effect=changing_digest):
            with self.assertRaisesRegex(ValueError, 'binary changed during'):
                self.verify()

    def test_route_only_marker_wrong_viewport_and_duplicate_are_rejected(self):
        (self.root / 'confirm-standard/marker.log').write_text('CAPTURE_READY page=runtime skin=dark\n')
        with self.assertRaisesRegex(ValueError, 'not activated'):
            self.verify()
        (self.root / 'confirm-standard/marker.log').write_text('scenario=confirm activated=true bounds=100,100,200,80')
        self.row['viewport'] = 'compact'
        with self.assertRaisesRegex(ValueError, 'dimensions'):
            self.verify()
        self.row['viewport'] = 'standard'
        self.row['marker_sha256'] = sha256(self.root / self.row['marker'])
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            self.verify([self.row, copy.deepcopy(self.row)])

    def test_foreign_window_and_path_escape_cannot_be_receipts(self):
        self.row['window_id'] = 'foreign'
        with self.assertRaisesRegex(ValueError, 'window receipt mismatch'):
            self.verify()
        self.row['window_id'] = '8'
        self.row['image'] = '../outside.png'
        with self.assertRaisesRegex(ValueError, 'escapes repository'):
            self.verify()

    def test_altered_capture_metadata_and_foreign_image_are_rejected(self):
        (self.root / 'capture-metadata.txt').write_text('source_fingerprint=foreign\n')
        with self.assertRaisesRegex(ValueError, 'metadata mismatch'):
            self.verify()
        (self.root / 'capture-metadata.txt').write_text(f"source_fingerprint=source\nbinary_sha256={self.row['binary_sha256']}\n")
        (self.root / 'foreign.png').write_bytes((self.root / self.row['image']).read_bytes())
        self.row['image'] = 'foreign.png'
        with self.assertRaisesRegex(ValueError, 'another capture'):
            self.verify()

    def test_appended_marker_and_metadata_cannot_reuse_existing_hashes(self):
        marker = self.root / self.row['marker']
        original = marker.read_text()
        marker.write_text(original + 'forged geometry bounds=0,0,100,100\n')
        with self.assertRaisesRegex(ValueError, 'altered activation marker'):
            self.verify()
        marker.write_text(original)
        metadata = self.root / 'capture-metadata.txt'
        metadata.write_text(metadata.read_text() + 'forged_capture=true\n')
        with self.assertRaisesRegex(ValueError, 'altered capture metadata'):
            self.verify()


    def test_missing_bounds_or_outside_bounds_cannot_be_geometry_evidence(self):
        for value in ('', 'bounds=0,0,400,900', 'bounds=0,0,0,40', 'bounds=nan,0,100,100'):
            with self.subTest(value=value), self.assertRaises(ValueError):
                interaction_bounds(f'scenario=confirm activated=true {value}', 'confirm', 'standard')

    def test_changed_interaction_region_cannot_reuse_the_pixel_receipt(self):
        self.row['interaction_bounds'] = [0,0,200,80]
        with self.assertRaisesRegex(ValueError, 'native geometry'):
            self.verify()


class InteractionPixelTests(unittest.TestCase):
    def test_a_nonblank_page_with_a_blank_interaction_region_is_rejected(self):
        from PIL import Image, ImageDraw
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'page.png'
            image = Image.new('RGB', (720,480), '#333333')
            ImageDraw.Draw(image).rectangle((0,0,80,80), fill='#ffffff')
            image.save(path)
            self.assertGreater(pixel_signature(path)['luma_stddev'], 2)
            with self.assertRaisesRegex(ValueError, 'blank capture pixels'):
                interaction_signature(path, [100,100,200,80])


if __name__ == '__main__':
    unittest.main()
