#!/usr/bin/env python3
"""Guard the freshness of published Slint screenshot receipts.

The published tree must describe exactly one **full** capture run: the most
recent one that published all 199 scenarios. A name-filtered diagnostic run
produces fresh pixels in `.evidence/captures/` for a handful of scenarios but
explicitly never touches the published corpus, so treating it as the latest
published batch would report 197 perfectly fresh receipts as stale and force
a misleading recapture. Only a full-batch manifest is a publication.

Freshness is bound to the capture run, not to the live source tree: another
session editing an unrelated crate must not invalidate receipts that the
latest run already published. The run's own source fingerprint and binary
digest stay in `.evidence/captures/*/capture-metadata.txt`.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT_DIR = pathlib.Path("crates/infiltrator-slint/docs/screenshots")
EVIDENCE_ROOT = pathlib.Path(".evidence/captures/slint")
FIELDS = ("scenario", "page", "skin", "requested_window", "app_pid", "window_id",
          "width", "height", "bytes", "sha256", "status")
FULL_MATRIX_SIZE = 199


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def read_manifest(path: pathlib.Path) -> dict[str, dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        if tuple(reader.fieldnames or ()) != FIELDS:
            raise ValueError(f"{path}: expected columns {FIELDS}")
        return {row["scenario"]: row for row in reader}


def latest_run(root: pathlib.Path) -> pathlib.Path | None:
    """The most recent full capture run; diagnostics never publish."""
    candidates = []
    for metadata in (root / EVIDENCE_ROOT).glob("*/capture-metadata.txt"):
        run = metadata.parent
        manifest = run / "capture-manifest.tsv"
        if not manifest.is_file():
            continue
        # The declared `scenario_count` is written when the run starts, so an
        # interrupted run declares the full matrix while its manifest carries
        # only the scenarios that finished. Selection must read what the run
        # actually published, otherwise a killed batch masquerades as the
        # publication and marks the intact corpus stale.
        rows = read_manifest(manifest)
        published = sum(1 for row in rows.values() if row["status"] == "ok")
        if published != FULL_MATRIX_SIZE:
            continue
        candidates.append((manifest.stat().st_mtime_ns, run))
    return max(candidates, default=(0, None))[1]


def violations(root: pathlib.Path) -> list[str]:
    out = root / OUT_DIR
    published = read_manifest(out / "manifest.tsv")
    run = latest_run(root)
    if run is None:
        return [f"{OUT_DIR}: no capture run has published a manifest"]
    expected = read_manifest(run / "capture-manifest.tsv")
    expected = {name: row for name, row in expected.items() if row.get("status") == "ok"}
    problems = []
    for name in sorted(set(published) - set(expected)):
        problems.append(f"{OUT_DIR}: stale manifest row {name}")
    for name, row in expected.items():
        image = out / f"{name}.png"
        if not image.is_file() or sha256(image) != row.get("sha256"):
            problems.append(f"{OUT_DIR}: stale or missing receipt {name}.png")
    for name, row in published.items():
        if name not in expected:
            continue
        if row.get("sha256") != expected[name].get("sha256"):
            problems.append(f"{OUT_DIR}: published row {name} does not match the latest run")
    for image in sorted(out.glob("*.png")):
        if image.stem not in expected:
            problems.append(f"{OUT_DIR}: stale screenshot {image.name}")
    return problems


class GuardTests(unittest.TestCase):
    def test_manifest_columns_are_stable(self) -> None:
        self.assertEqual(FIELDS[0], "scenario")
        self.assertEqual(FIELDS[-1], "status")

    def test_diagnostic_runs_are_not_publication_batches(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            full = root / EVIDENCE_ROOT / "full"
            diagnostic = root / EVIDENCE_ROOT / "diagnostic"
            full.mkdir(parents=True)
            diagnostic.mkdir(parents=True)
            rows = "\t".join(FIELDS) + "\n" + "".join(
                f"s{i}\tpage\tskin\t1180x780\t1\t2\t1180\t780\t1\tabc\tok\n"
                for i in range(FULL_MATRIX_SIZE))
            (full / "capture-manifest.tsv").write_text(rows)
            (full / "capture-metadata.txt").write_text(
                f"scenario_count={FULL_MATRIX_SIZE}\n")
            (diagnostic / "capture-manifest.tsv").write_text("\t".join(FIELDS) + "\n")
            (diagnostic / "capture-metadata.txt").write_text("scenario_count=6\n")
            later_ns = (full / "capture-manifest.tsv").stat().st_mtime_ns + 1000
            import os
            os.utime(diagnostic / "capture-manifest.tsv", ns=(later_ns, later_ns))
            self.assertEqual(latest_run(root), full)

    def test_sha256_is_stable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "pixels"
            path.write_bytes(b"pixels")
            self.assertEqual(sha256(path),
                             hashlib.sha256(b"pixels").hexdigest())

    def test_an_interrupted_run_is_not_a_publication(self) -> None:
        """A killed batch declares the full matrix but published a fraction."""
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            complete = root / EVIDENCE_ROOT / "complete"
            killed = root / EVIDENCE_ROOT / "killed"
            for run in (complete, killed):
                run.mkdir(parents=True)
            header = "\t".join(FIELDS) + "\n"
            ok_rows = header + "".join(
                f"s{i}\tpage\tskin\t1180x780\t1\t2\t1180\t780\t1\tabc\tok\n"
                for i in range(FULL_MATRIX_SIZE))
            (complete / "capture-manifest.tsv").write_text(ok_rows)
            (complete / "capture-metadata.txt").write_text(
                f"scenario_count={FULL_MATRIX_SIZE}\n")
            # The interrupted run wrote the same declared count at start and
            # only got through 72 scenarios before it was killed.
            (killed / "capture-manifest.tsv").write_text(header + "".join(
                f"s{i}\tpage\tskin\t1180x780\t1\t2\t1180\t780\t1\tabc\tok\n"
                for i in range(72)))
            (killed / "capture-metadata.txt").write_text(
                f"scenario_count={FULL_MATRIX_SIZE}\n")
            import os
            later_ns = (complete / "capture-manifest.tsv").stat().st_mtime_ns + 1000
            os.utime(killed / "capture-manifest.tsv", ns=(later_ns, later_ns))
            self.assertEqual(latest_run(root), complete)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardTests)
        result = unittest.TextTestRunner(verbosity=0).run(suite)
        return 0 if result.wasSuccessful() else 1
    problems = violations(ROOT)
    for problem in problems:
        print(problem)
    print(f"receipt freshness guard: violations={len(problems)}")
    if args.mode == "enforce" and problems:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
