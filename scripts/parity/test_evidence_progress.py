"""A candidate revalidation gap must never reset accumulated successful acceptance records."""
import json
import tempfile
import unittest
from pathlib import Path
from evidence_progress import historical_coverage, progress


class ProgressTests(unittest.TestCase):
    def test_distinct_builds_preserve_history_without_becoming_selected_release_coverage(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, cells in [('old-iced', [['confirm', 'iced']]), ('new-bevy', [['confirm', 'bevy']])]:
                run = root / name
                run.mkdir()
                (run / 'acceptance.json').write_text(json.dumps({
                    'status':'pass','registry_verified':True,'discovery_verified':True,'aligned':cells}))
            manifest = [{'feature_id':'confirm','surface':surface,'status':'implemented'} for surface in ['iced','bevy']]
            evidence = [{'level':level,'status':'anchored'} for level in ['contract','scenario'] for _ in range(2)]
            report = {'acceptance_scope':'baseline-coverage','aligned':[],'release_complete':False}
            observed = progress(manifest, evidence, root, {'confirm'}, report)
            self.assertEqual(observed['historical_baselines']['accepted_peer_scenarios'], 1)
            self.assertEqual(observed['historical_baselines']['accepted_surface_cells'], 2)
            self.assertIsNone(observed['selected_candidate']['aligned_surface_cells'])
            self.assertFalse(observed['selected_candidate']['explicitly_selected'])
            self.assertFalse(observed['selected_candidate']['release_complete'])
            self.assertEqual(observed['targets']['visual_receipts'], 4)

    def test_failed_unknown_duplicate_and_undiscovered_reports_cannot_inflate_progress(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for number, report in enumerate([
                {'status':'pass','registry_verified':True,'discovery_verified':True,'aligned':[['confirm','iced'],['confirm','iced'],['foreign','bevy']]},
                {'status':'fail','registry_verified':True,'discovery_verified':True,'aligned':[['confirm','bevy']]},
                {'status':'pass','registry_verified':False,'discovery_verified':False,'aligned':[['confirm','bevy']]}]):
                run = root / str(number); run.mkdir()
                (run / 'acceptance.json').write_text(json.dumps(report))
            result = historical_coverage(root, {'confirm'})
            self.assertEqual(result['accepted_surface_cells'], 1)
            self.assertEqual(result['accepted_peer_scenarios'], 0)
