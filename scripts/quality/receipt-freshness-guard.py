#!/usr/bin/env python3
"""Guard the freshness of published Slint screenshot receipts.

The published tree must describe exactly one **full** capture run: the most
recent one that published every scenario the canonical matrix declares. A
name-filtered diagnostic run
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
import io
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts" / "parity"))
from matrix_size import matrix_scenario_count

OUT_DIR = pathlib.Path("crates/infiltrator-slint/docs/screenshots")
EVIDENCE_ROOT = pathlib.Path(".evidence/captures/slint")
FIELDS = ("scenario", "page", "skin", "requested_window", "app_pid", "window_id",
          "width", "height", "bytes", "sha256", "status", "locale")
LEGACY_FIELDS = FIELDS[:-1]
# The size of a full batch is whatever the canonical matrix declares, read from
# the file. It used to be typed as 211 beside a second literal 203 kept for the
# previous matrix, which meant a grown matrix disqualified every new full
# publication at once: latest_run() returned None and the pixel leg reported
# "no capture run has published a manifest" about a corpus that was fine.
FULL_MATRIX_SIZE = matrix_scenario_count("slint", ROOT)
LOCALE_MATRIX = ROOT / "scripts/capture_slint_scenarios.tsv"
LOCALE_ROWS = (
    "sidebar-locale-zh-standard",
    "sidebar-locale-zh-compact",
    "sidebar-locale-en-standard",
    "sidebar-locale-en-compact",
    "page-locale-zh-standard",
    "page-locale-zh-compact",
    "page-locale-en-standard",
    "page-locale-en-compact",
    "page-locale-settings-zh-standard",
    "page-locale-settings-zh-compact",
    "page-locale-settings-en-standard",
    "page-locale-settings-en-compact",
)
EXPECTED_NAV_LABELS = {
    "zh-CN": {"overview": "核心概览", "proxies": "代理节点", "profiles": "配置管理", "settings": "系统设置"},
    "en-US": {"overview": "Overview", "proxies": "Proxies", "profiles": "Profiles", "settings": "Settings"},
}
EXPECTED_LOCALE_COPY = {
    "zh-CN": {
        "overview": {
            "desktop-title": "核心概览",
            "mobile-title": "仪表盘",
            "quality-tag": "质量未观测",
            "traffic-peak-up": "峰值 2.1 MB/s",
            "traffic-total-up": "累计 1.60 GB",
        },
        "settings": {
            "desktop-title": "系统设置",
            "mobile-title": "工具",
        },
    },
    "en-US": {
        "overview": {
            "desktop-title": "Overview",
            "mobile-title": "Dashboard",
            "quality-tag": "Quality not observed",
            "traffic-peak-up": "Peak 2.1 MB/s",
            "traffic-total-up": "Total 1.60 GB",
        },
        "settings": {
            "desktop-title": "Settings",
            "mobile-title": "Tools",
        },
    },
}
PAGE_LOCALE_ROWS = {
    name: page
    for name, page in (
        ("page-locale-zh-standard", "overview"),
        ("page-locale-zh-compact", "overview"),
        ("page-locale-en-standard", "overview"),
        ("page-locale-en-compact", "overview"),
        ("page-locale-settings-zh-standard", "settings"),
        ("page-locale-settings-zh-compact", "settings"),
        ("page-locale-settings-en-standard", "settings"),
        ("page-locale-settings-en-compact", "settings"),
    )
}


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def read_manifest(path: pathlib.Path) -> dict[str, dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        if tuple(reader.fieldnames or ()) not in (FIELDS, LEGACY_FIELDS):
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
        if published < FULL_MATRIX_SIZE:
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
    problems.extend(locale_receipt_findings(root, run))
    return problems


def locale_receipt_findings(
    root: pathlib.Path,
    run: pathlib.Path,
    *,
    matrix: str | None = None,
    published_locale: str | None = None,
    markers: dict[str, str] | None = None,
) -> list[str]:
    """The four sidebar frames must prove the labels of both locales."""
    findings: list[str] = []
    matrix_path = root / LOCALE_MATRIX
    if matrix is None and not matrix_path.is_file():
        return [f"{LOCALE_MATRIX}: missing capture matrix"]
    source = matrix if matrix is not None else matrix_path.read_text(encoding="utf-8")
    with io.StringIO(source) as handle:
        rows = {row["name"]: row for row in csv.DictReader(handle, delimiter="\t")}
    for name in LOCALE_ROWS:
        row = rows.get(name)
        if row is None:
            findings.append(f"{LOCALE_MATRIX}: missing locale receipt {name}")
            continue
        locale = row.get("locale")
        if locale not in EXPECTED_NAV_LABELS:
            findings.append(f"{LOCALE_MATRIX}: {name} has unsupported locale {locale!r}")
            continue
        published = (
            {"locale": published_locale}
            if published_locale is not None
            else read_manifest(root / OUT_DIR / "manifest.tsv").get(name, {})
        )
        if published.get("locale") != locale:
            findings.append(f"{OUT_DIR}: manifest locale for {name} is not {locale}")
        marker = run / name / "marker.log"
        marker_text = markers.get(name) if markers is not None else None
        if marker_text is None and not marker.is_file():
            findings.append(f"{run / name}: missing capture marker")
            continue
        text = marker_text if marker_text is not None else marker.read_text(encoding="utf-8")
        lines = [line for line in text.splitlines()
                 if line.startswith("CAPTURE_NAV ")]
        if len(lines) != 1:
            findings.append(f"{marker}: expected exactly one CAPTURE_NAV line")
            continue
        prefix = "CAPTURE_NAV labels="
        if not lines[0].startswith(prefix):
            findings.append(f"{marker}: malformed CAPTURE_NAV line")
            continue
        encoded = lines[0][len(prefix):]
        published_labels = {}
        try:
            for entry in filter(None, encoded.split(";")):
                page, digest = entry.split("=", 1)
                published_labels[page] = bytes.fromhex(digest).decode("utf-8")
        except (ValueError, UnicodeDecodeError) as error:
            findings.append(f"{marker}: CAPTURE_NAV labels are not hex UTF-8: {error}")
            continue
        for page, word in EXPECTED_NAV_LABELS[locale].items():
            if published_labels.get(page) != word:
                findings.append(
                    f"{marker}: {locale} navigation row {page} reads "
                    f"{published_labels.get(page)!r}, expected {word!r}"
                )
        if name in PAGE_LOCALE_ROWS:
            copy_lines = [line for line in text.splitlines()
                          if line.startswith("CAPTURE_LOCALE copy=")]
            if len(copy_lines) != 1:
                findings.append(f"{marker}: expected exactly one CAPTURE_LOCALE line")
                continue
            prefix = "CAPTURE_LOCALE copy="
            published_copy = {}
            try:
                for entry in filter(None, copy_lines[0][len(prefix):].split(";")):
                    key, digest = entry.split("=", 1)
                    published_copy[key] = bytes.fromhex(digest).decode("utf-8")
            except (ValueError, UnicodeDecodeError) as error:
                findings.append(f"{marker}: CAPTURE_LOCALE copy is not hex UTF-8: {error}")
                continue
            expected_copy = EXPECTED_LOCALE_COPY[locale][PAGE_LOCALE_ROWS[name]]
            for key, word in expected_copy.items():
                if published_copy.get(key) != word:
                    findings.append(
                        f"{marker}: {locale} page copy {key} reads "
                        f"{published_copy.get(key)!r}, expected {word!r}"
                    )
            if locale == "en-US":
                for key, word in published_copy.items():
                    if any("\u4e00" <= character <= "\u9fff" for character in word):
                        findings.append(
                            f"{marker}: en-US page copy {key} contains CJK text {word!r}"
                        )
    return findings


class GuardTests(unittest.TestCase):
    def test_manifest_columns_are_stable(self) -> None:
        self.assertEqual(FIELDS[0], "scenario")
        self.assertEqual(FIELDS[-1], "locale")

    def test_a_legacy_full_batch_is_not_the_latest_publication(self) -> None:
        # A batch that finished fewer rows than the matrix now declares is not a
        # publication, however fresh its files look: it predates the growth.
        AN_OLD_MATRIX_SIZE = FULL_MATRIX_SIZE - 8
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            legacy = root / EVIDENCE_ROOT / "legacy"
            current = root / EVIDENCE_ROOT / "current"
            legacy.mkdir(parents=True)
            current.mkdir(parents=True)
            legacy_rows = "\t".join(LEGACY_FIELDS) + "\n" + "".join(
                f"s{i}\tpage\tskin\t1180x780\t1\t2\t1180\t780\t1\tabc\tok\n"
                for i in range(AN_OLD_MATRIX_SIZE))
            current_rows = "\t".join(FIELDS) + "\n" + "".join(
                f"s{i}\tpage\tskin\t1180x780\t1\t2\t1180\t780\t1\tabc\tok\tzh-CN\n"
                for i in range(FULL_MATRIX_SIZE))
            (legacy / "capture-manifest.tsv").write_text(legacy_rows)
            (current / "capture-manifest.tsv").write_text(current_rows)
            (legacy / "capture-metadata.txt").write_text("scenario_count=199\n")
            (current / "capture-metadata.txt").write_text(
                f"scenario_count={FULL_MATRIX_SIZE}\n")
            import os
            later_ns = (current / "capture-manifest.tsv").stat().st_mtime_ns + 1000
            os.utime(legacy / "capture-manifest.tsv", ns=(later_ns, later_ns))
            self.assertEqual(latest_run(root), current)

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

    def test_a_locale_receipt_with_the_wrong_nav_word_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            labels = ";".join(
                f"{page}={word.encode('utf-8').hex()}"
                for page, word in (("overview", "Overview"), ("proxies", "代理节点"))
            ) + ";"
            matrix = (
                "\t".join(("name", "page", "skin", "window_size", "locale")) + "\n"
                + "sidebar-locale-en-standard\toverview\tdark\t1180x780\ten-US\n"
            )
            findings = locale_receipt_findings(
                root,
                pathlib.Path("/unused-run"),
                matrix=matrix,
                published_locale="en-US",
                markers={
                    "sidebar-locale-en-standard": "CAPTURE_NAV labels=" + labels + "\n"
                },
            )
            self.assertTrue(any("en-US navigation row proxies" in item for item in findings), findings)

    def test_an_english_page_receipt_with_chinese_content_copy_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            labels = ";".join(
                f"{page}={word.encode('utf-8').hex()}"
                for page, word in EXPECTED_NAV_LABELS["en-US"].items()
            ) + ";"
            copy = ";".join(
                f"{key}={word.encode('utf-8').hex()}"
                for key, word in (("desktop-title", "Overview"),
                                  ("mobile-title", "仪表盘"),
                                  ("network-mode", "TUN"),
                                  ("quality-tag", "Quality not observed"),
                                  ("traffic-peak-up", "Peak 2.1 MB/s"),
                                  ("traffic-total-up", "Total 1.60 GB"))
            ) + ";"
            matrix = (
                "\t".join(("name", "page", "skin", "window_size", "locale")) + "\n"
                + "page-locale-en-standard\toverview\tdark\t1180x780\ten-US\n"
            )
            findings = locale_receipt_findings(
                root,
                pathlib.Path("/unused-run"),
                matrix=matrix,
                published_locale="en-US",
                markers={
                    "page-locale-en-standard": (
                        "CAPTURE_NAV labels=" + labels + "\n"
                        + "CAPTURE_LOCALE copy=" + copy + "\n"
                    )
                },
            )
            self.assertTrue(
                any("en-US page copy mobile-title" in item for item in findings),
                findings,
            )

    def test_an_english_page_receipt_with_chinese_dashboard_copy_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            labels = ";".join(
                f"{page}={word.encode('utf-8').hex()}"
                for page, word in EXPECTED_NAV_LABELS["en-US"].items()
            ) + ";"
            copy = ";".join(
                f"{key}={word.encode('utf-8').hex()}"
                for key, word in EXPECTED_LOCALE_COPY["en-US"]["overview"].items()
            ) + f";traffic-peak-down={'峰值 6.9 MB/s'.encode('utf-8').hex()};"
            matrix = (
                "\t".join(("name", "page", "skin", "window_size", "locale")) + "\n"
                + "page-locale-en-standard\toverview\tdark\t1180x780\ten-US\n"
            )
            findings = locale_receipt_findings(
                root,
                pathlib.Path("/unused-run"),
                matrix=matrix,
                published_locale="en-US",
                markers={
                    "page-locale-en-standard": (
                        "CAPTURE_NAV labels=" + labels + "\n"
                        + "CAPTURE_LOCALE copy=" + copy + "\n"
                    )
                },
            )
            self.assertTrue(
                any("contains CJK text" in item for item in findings),
                findings,
            )


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
