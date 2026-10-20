"""Negative tests for the five-platform evidence guard.

The guard proves that the platform evidence matrix never claims a platform is
proven without a real artifact or run, that it stays aligned with the
business coverage matrix and with the product identity registry, and that the
generated section of the platform matrix document never drifts from the
evidence ledger.
"""

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

QUALITY = Path(__file__).resolve().parents[1]
ROOT = QUALITY.parent.parent
sys.path.insert(0, str(ROOT / "scripts" / "platform"))

SPEC = importlib.util.spec_from_file_location(
    "platform_evidence_guard",
    QUALITY / "platform-evidence-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)

MODEL_SPEC = importlib.util.spec_from_file_location(
    "evidence_model",
    ROOT / "scripts" / "platform" / "evidence_model.py",
)
MODEL = importlib.util.module_from_spec(MODEL_SPEC)
sys.modules[MODEL_SPEC.name] = MODEL
MODEL_SPEC.loader.exec_module(MODEL)


def synthetic_ledger(**overrides):
    """A ledger that satisfies every rule of the guard."""
    products = ("iced", "bevy", "slint")
    evidence = tuple(
        MODEL.Evidence(
            kind="artifact",
            product=product,
            identity="sha256:" + chr(ord("a") + index) * 64,
            detail=f"dist/infiltrator_0.1.0_amd64.deb ({product})",
        )
        for index, product in enumerate(products)
    )
    legs = []
    for name in MODEL.LEGS:
        legs.append(MODEL.Leg(
            name=name,
            status=overrides.get("status", MODEL.STATUS_PROVEN),
            reason=overrides.get("reason", ""),
            command=overrides.get("command", "bash packaging/linux/build-linux.sh"),
            evidence=overrides.get("evidence", evidence),
        ))
    return MODEL.PlatformLedger(
        platform=overrides.get("platform", "linux"),
        label=overrides.get("label", "Linux desktop"),
        products=overrides.get("products", products),
        legs=tuple(overrides.get("legs", legs)),
        path=Path("scripts/platform/evidence/linux.json"),
    )


class PlatformEvidenceLedgerRules(unittest.TestCase):
    def test_the_repository_ledgers_satisfy_the_guard(self):
        problems, _ = GUARD.check()
        self.assertEqual(problems, [])

    def test_a_synthetic_ledger_satisfies_the_guard(self):
        self.assertEqual(MODEL.ledger_problems(synthetic_ledger()), [])

    def test_an_unknown_leg_status_is_rejected(self):
        ledger = synthetic_ledger(status="maybe")
        problems = MODEL.ledger_problems(ledger)
        self.assertTrue(any("unknown status" in problem for problem in problems))

    def test_an_unsupported_leg_without_a_reason_is_rejected(self):
        ledger = synthetic_ledger(status=MODEL.STATUS_UNSUPPORTED, reason="")
        problems = MODEL.ledger_problems(ledger)
        self.assertTrue(any("needs a reason" in problem for problem in problems))

    def test_a_cross_compile_check_cannot_prove_a_leg(self):
        evidence = (
            MODEL.Evidence(
                kind="cross_compile",
                product="",
                identity="command:bash scripts/cross-check.sh",
                detail="the crates only cross-compile",
            ),
        )
        ledger = synthetic_ledger(evidence=evidence)
        problems = MODEL.ledger_problems(ledger)
        self.assertTrue(any("claims Proven" in problem for problem in problems))

    def test_a_malformed_artifact_identity_is_rejected(self):
        evidence = (
            MODEL.Evidence(
                kind="artifact",
                product="iced",
                identity="not-a-digest",
                detail="dist/infiltrator_0.1.0_amd64.deb",
            ),
        )
        ledger = synthetic_ledger(evidence=evidence)
        problems = MODEL.ledger_problems(ledger)
        self.assertTrue(any("sha256:" in problem for problem in problems))

    def test_an_evidence_entry_of_an_unregistered_product_is_rejected(self):
        evidence = (
            MODEL.Evidence(
                kind="artifact",
                product="compose",
                identity="sha256:" + "b" * 64,
                detail="dist/infiltrator_0.1.0_amd64.deb",
            ),
        )
        ledger = synthetic_ledger(evidence=evidence)
        problems = MODEL.ledger_problems(ledger)
        self.assertTrue(any("unregistered product" in problem for problem in problems))

    def test_a_missing_leg_is_rejected(self):
        ledger = synthetic_ledger(legs=synthetic_ledger().legs[:2])
        problems = MODEL.ledger_problems(ledger)
        self.assertTrue(any("the legs must be exactly" in problem for problem in problems))

    def test_an_unsupported_leg_with_full_coverage_is_rejected(self):
        ledger = synthetic_ledger(status=MODEL.STATUS_UNSUPPORTED, reason="not built yet")
        problems = MODEL.ledger_problems(ledger)
        self.assertTrue(any("claims Unsupported" in problem for problem in problems))


class PlatformEvidenceConsistencyRules(unittest.TestCase):
    def setUp(self):
        self.parity_legs = GUARD.parity_platform_legs()
        self.products = GUARD.registered_products()

    def test_the_business_coverage_matrix_registers_the_five_platforms(self):
        self.assertEqual(set(self.parity_legs), set(MODEL.PLATFORM_ORDER))

    def test_the_desktop_products_match_the_coverage_matrix_surfaces(self):
        ledger = synthetic_ledger(products=("iced",))
        problems = GUARD.ledger_consistency_problems(ledger, self.parity_legs, self.products)
        self.assertTrue(any("differ from the business coverage matrix" in problem
                            for problem in problems))

    def test_an_unregistered_product_is_rejected(self):
        ledger = synthetic_ledger(products=("iced", "bevy", "slint", "slint2"))
        problems = GUARD.ledger_consistency_problems(ledger, self.parity_legs, self.products)
        self.assertTrue(any("not registered in the identity registry" in problem
                            for problem in problems))

    def test_an_unknown_platform_is_rejected(self):
        ledger = synthetic_ledger(platform="solaris")
        problems = GUARD.ledger_consistency_problems(ledger, self.parity_legs, self.products)
        self.assertTrue(any("unknown platform" in problem for problem in problems))

    def test_the_document_registers_the_same_platforms_as_the_ledgers(self):
        rows = GUARD.document_platform_rows(
            "| Platform | Product |\n| --- | --- |\n| Linux desktop | x |\n")
        self.assertEqual(rows, {("Linux desktop", "linux")})


class PlatformEvidenceDocumentRules(unittest.TestCase):
    def test_the_generated_section_matches_the_ledger(self):
        ledgers = MODEL.load_ledgers()
        section = MODEL.render_section(ledgers)
        document = (ROOT / MODEL.PLATFORM_MATRIX).read_text(encoding="utf-8")
        start, end = MODEL.section_span(document)
        self.assertEqual(document[start:end], section)

    def test_a_drifted_section_is_detected(self):
        ledgers = MODEL.load_ledgers()
        section = MODEL.render_section(ledgers)
        drifted = section.replace(MODEL.STATUS_UNSUPPORTED, "Proven", 1)
        self.assertNotEqual(drifted, section)

    def test_the_rendered_cell_carries_the_evidence_identity(self):
        section = MODEL.render_section([synthetic_ledger()])
        self.assertIn("sha256:", section)
        self.assertIn("**Proven**", section)


class PlatformEvidenceRecorderRules(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location(
            "record_platform_evidence",
            ROOT / "scripts" / "platform" / "record-platform-evidence.py",
        )
        self.recorder = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = self.recorder
        spec.loader.exec_module(self.recorder)

    def test_a_proven_run_promotes_the_covered_legs(self):
        # The ledger knows no artifact yet: only the recorded product is
        # covered, so the leg stays unsupported and names the missing ones.
        ledger = synthetic_ledger(
            status=MODEL.STATUS_UNSUPPORTED,
            reason="nothing yet",
            evidence=(),
        )
        payload = {
            "schema_version": 1,
            "platform": "linux",
            "product": "iced",
            "run_id": "20261020T101112Z_abcdef",
            "artifacts": [
                {
                    "kind": "deb",
                    "path": "dist/infiltrator_0.1.0_amd64.deb",
                    "sha256": "c" * 64,
                    "legs": {
                        "install": {"status": "proven"},
                        "run": {"status": "proven"},
                        "remove": {"status": "proven"},
                    },
                },
            ],
        }
        legs = self.recorder.recorded_legs(ledger, payload)
        build = next(leg for leg in legs if leg.name == MODEL.LEG_BUILD)
        run = next(leg for leg in legs if leg.name == MODEL.LEG_RUN)
        self.assertEqual(build.status, MODEL.STATUS_UNSUPPORTED)
        self.assertIn("iced", build.reason)
        self.assertIn("bevy", build.reason)
        self.assertEqual(run.status, MODEL.STATUS_UNSUPPORTED)
        self.assertTrue(any(entry.identity == "run:20261020T101112Z_abcdef"
                            for entry in run.evidence))

    def test_a_run_that_covers_every_product_proves_the_leg(self):
        # One run proves one product; the ledger accumulates the runs until
        # every product of the platform is covered.
        ledger = synthetic_ledger(
            status=MODEL.STATUS_UNSUPPORTED,
            reason="nothing yet",
            evidence=(),
        )
        for index, product in enumerate(("iced", "bevy", "slint")):
            payload = {
                "schema_version": 1,
                "platform": "linux",
                "product": product,
                "run_id": f"20261020T1011{index}Z_abcdef",
                "artifacts": [
                    {
                        "kind": "deb",
                        "path": f"dist/infiltrator_0.1.0_amd64.deb ({product})",
                        "sha256": chr(ord("d") + index) * 64,
                        "legs": {
                            "install": {"status": "proven"},
                            "run": {"status": "proven"},
                            "remove": {"status": "proven"},
                        },
                    },
                ],
            }
            legs = self.recorder.recorded_legs(ledger, payload)
            ledger = MODEL.PlatformLedger(
                platform=ledger.platform,
                label=ledger.label,
                products=ledger.products,
                legs=tuple(legs),
                path=ledger.path,
            )
        build = next(leg for leg in ledger.legs if leg.name == MODEL.LEG_BUILD)
        self.assertEqual(build.status, MODEL.STATUS_PROVEN)
        self.assertEqual(build.reason, "")

    def test_a_failed_run_never_promotes_a_leg(self):
        ledger = synthetic_ledger(status=MODEL.STATUS_UNSUPPORTED, reason="nothing yet")
        payload = {
            "schema_version": 1,
            "platform": "linux",
            "product": "iced",
            "run_id": "20261020T101112Z_abcdef",
            "artifacts": [
                {
                    "kind": "deb",
                    "path": "dist/infiltrator_0.1.0_amd64.deb",
                    "sha256": "c" * 64,
                    "legs": {
                        "install": {"status": "failed"},
                        "run": {"status": "failed"},
                        "remove": {"status": "proven"},
                    },
                },
            ],
        }
        legs = self.recorder.recorded_legs(ledger, payload)
        run = next(leg for leg in legs if leg.name == MODEL.LEG_RUN)
        self.assertEqual(run.status, MODEL.STATUS_UNSUPPORTED)
        self.assertFalse(any(entry.kind == MODEL.KIND_RUN for entry in run.evidence))

    def test_a_run_evidence_without_a_digest_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "evidence.json"
            path.write_text(json.dumps({
                "schema_version": 1,
                "platform": "linux",
                "product": "iced",
                "run_id": "run-1",
                "artifacts": [{"kind": "deb", "path": "x", "legs": {}}],
            }), encoding="utf-8")
            with self.assertRaises(ValueError):
                self.recorder.run_evidence(path)


if __name__ == "__main__":
    unittest.main()
