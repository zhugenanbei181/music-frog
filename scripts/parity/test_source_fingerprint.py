"""The legacy repository snapshot is audit provenance, never an acceptance freshness gate."""
from pathlib import Path
import subprocess
import tempfile
import unittest

from visual_receipts import source_fingerprint


class CaptureSourceTests(unittest.TestCase):
    def test_repository_provenance_records_changes_without_claiming_receipt_invalidation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(['git','init','--quiet',str(root)], check=True)
            icon = root / 'crates/widgets/assets/icon.png'
            icon.parent.mkdir(parents=True)
            icon.write_bytes(b'original icon')
            matrix = root / 'scripts/scenarios.tsv'
            matrix.parent.mkdir(parents=True)
            matrix.write_text('first scene\n')
            initial = source_fingerprint(root)
            icon.write_bytes(b'changed runtime icon')
            changed = source_fingerprint(root)
            self.assertNotEqual(initial, changed)
            matrix.write_text('different scene\n')
            self.assertNotEqual(changed, source_fingerprint(root))
