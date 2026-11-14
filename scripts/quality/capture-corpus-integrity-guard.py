#!/usr/bin/env python3
"""Every published screenshot corpus must be described by its own index.

A screenshot tree is product evidence only while `manifest.tsv` and the PNGs
beside it are the same batch. Two directions of divergence were measured in
this repository, both invisible to every existing leg:

  * **uncited pixels** — a PNG the index does not name. The Bevy corpus shipped
    191 PNGs behind an index with one row: 190 files could be cited by nothing,
    and no file could be traced to a capture run. Reconstructing the index from
    git history was measured to be impossible (only 2 of 191 digests appear in
    any historical index), and the newest evidence run attests just 170 of the
    173 rows it holds, so the tree is a patchwork of several batches.
  * **matrix rows with no receipt** — a scenario the canonical matrix declares
    that the published index does not name, i.e. business behaviour with no
    pixel behind it.

The rules that can only ever be satisfied by a real recapture are held as a
count ratchet (`scripts/quality/capture_corpus_debt.json`): the number of
uncited pixels may only shrink, so a corpus can never drift further and the
entry dies only when a full batch actually publishes. The ratchet bounds a
count rather than the index bytes because a divergent index belongs to whoever
is recapturing it, while the pixels that have no receipt are the fact under
audit. Everything else is a hard failure: an index row whose pixel is missing,
or whose recorded digest, size or window dimensions disagree with the bytes on
disk, means the published evidence is lying about itself.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import pathlib
import re
import shutil
import struct
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts" / "parity"))
from corpus_layout import SURFACE_CORPORA

LEDGER = pathlib.Path("scripts/quality/capture_corpus_debt.json")
MANIFEST_FIELDS = ("scenario", "page", "skin", "requested_window", "app_pid",
                   "window_id", "width", "height", "bytes", "sha256", "status")


class Surface:
    """One published corpus and the canonical matrix that must cover it."""

    def __init__(self, name: str, out_dir: pathlib.Path, matrix: pathlib.Path,
                 matrix_name_column: int) -> None:
        self.name = name
        self.out_dir = out_dir
        self.matrix = matrix
        self.matrix_name_column = matrix_name_column


SURFACES = (
    Surface("slint", SURFACE_CORPORA["slint"],
            pathlib.Path("scripts/capture_slint_scenarios.tsv"), 0),
    Surface("iced", SURFACE_CORPORA["iced"],
            pathlib.Path("scripts/capture_iced_scenarios.tsv"), 0),
    Surface("bevy", SURFACE_CORPORA["bevy"],
            pathlib.Path("scripts/capture_bevy_scenarios.tsv"), 0),
)
CAPTURE_HARNESS = pathlib.Path("scripts/parity/capture_attempt.sh")
CAPTURE_MATRICES = (
    pathlib.Path("scripts/capture-slint-matrix.sh"),
    pathlib.Path("scripts/capture-iced-matrix.sh"),
    pathlib.Path("scripts/capture-bevy-matrix.sh"),
)
# The corpus path has one Python truth (`scripts/parity/corpus_layout.py`) and
# three shell literals. The Python readers must import that truth instead of
# keeping their own copy, and the shell writers must carry the same path.
CORPUS_LAYOUT_MODULE = pathlib.Path("scripts/parity/corpus_layout.py")
CORPUS_LAYOUT_CONSUMERS = (
    pathlib.Path("scripts/quality/capture-corpus-integrity-guard.py"),
    pathlib.Path("scripts/quality/screenshot-identity-guard.py"),
    pathlib.Path("scripts/quality/receipt-freshness-guard.py"),
)
CORPUS_PATH_LITERAL = ".evidence/corpora"
# Readers that must agree on how many scenarios a full batch owes. Each of them
# used to keep its own copy of the number - two typed `= 211` literals and one
# inference from "the largest count ever published" - so growing the matrix
# would have made every one of them wrong in a different direction.
MATRIX_SIZE_CONSUMERS = (
    pathlib.Path("scripts/parity/space_utilization.py"),
    pathlib.Path("scripts/quality/receipt-freshness-guard.py"),
    pathlib.Path("scripts/quality/slint-dock-material-guard.py"),
)
TYPED_MATRIX_SIZE = re.compile(r"^(FULL_MATRIX_SIZE|LEGACY_MATRIX_SIZE)\s*=\s*\d+", re.M)


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def png_dimensions(path: pathlib.Path) -> tuple[int, int] | None:
    """Width and height from the PNG IHDR, or None when the file is not a PNG."""
    try:
        with path.open("rb") as handle:
            head = handle.read(24)
    except OSError:
        return None
    if head[:8] != b"\x89PNG\r\n\x1a\n" or head[12:16] != b"IHDR":
        return None
    return struct.unpack(">II", head[16:24])


def read_index(path: pathlib.Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        if tuple(reader.fieldnames or ())[:len(MANIFEST_FIELDS)] != MANIFEST_FIELDS:
            raise ValueError(f"{path}: expected leading columns {MANIFEST_FIELDS}")
        return [row for row in reader if row.get("scenario")]


def read_matrix_names(surface: Surface, root: pathlib.Path) -> list[str]:
    with (root / surface.matrix).open(encoding="utf-8", newline="") as handle:
        reader = csv.reader(handle, delimiter="\t")
        rows = list(reader)
    column = surface.matrix_name_column
    return [row[column] for row in rows[1:] if len(row) > column and row[column]]


def load_debt(root: pathlib.Path) -> dict[str, dict[str, object]]:
    """Divergent corpora bounded by a count that may only shrink."""
    path = root / LEDGER
    if not path.is_file():
        return {}
    payload = json.loads(path.read_text(encoding="utf-8"))
    if payload.get("version") != 1:
        raise ValueError(f"{LEDGER}: unsupported version {payload.get('version')!r}")
    entries = payload.get("divergent_corpora")
    if not isinstance(entries, dict):
        raise ValueError(f"{LEDGER}: divergent_corpora must be an object")
    for surface, entry in entries.items():
        if not isinstance(entry, dict):
            raise ValueError(f"{LEDGER}: {surface} entry must be an object")
        for key in ("uncited_max", "receipt_gap_max"):
            if not isinstance(entry.get(key), int) or entry[key] < 0:
                raise ValueError(f"{LEDGER}: {surface} entry needs an integer {key}")
        if not entry.get("reason"):
            raise ValueError(f"{LEDGER}: {surface} entry needs a reason")
    return {str(key): dict(value) for key, value in entries.items()}


def corpus_findings(surface: Surface, root: pathlib.Path,
                    debt: dict[str, dict[str, object]]) -> list[str]:
    """Findings for one published corpus, against its index and its matrix.

    A corpus has two halves: the pixels (local evidence, never in git) and the
    index that names them (tracked). A checkout that never captured has the
    index alone, and the index is still enough to hold the surface to its
    canonical matrix — so the matrix legs run everywhere, while the pixel legs
    (digest, size, dimensions, uncited files) run only where the pixels are.
    A tree without pixels must never be judged as a corpus with broken pixels.
    """
    out = root / surface.out_dir
    index_path = out / "manifest.tsv"
    pixels = {path.stem: path for path in out.glob("*.png")}
    if not index_path.is_file():
        if pixels:
            return [f"{surface.name}: published pixels without an index at "
                    f"{surface.out_dir}/manifest.tsv"]
        # The index is tracked, so a missing one means this tree lost a tracked
        # file — never a verdict about the corpus.
        return [f"{surface.name}: no published index at {surface.out_dir}/manifest.tsv"]
    rows = read_index(index_path)
    indexed = {row["scenario"]: row for row in rows if row.get("status") == "ok"}
    problems: list[str] = []

    matrix_names = set(read_matrix_names(surface, root))
    # Every canonical matrix scenario needs a receipt in the published index.
    missing = sorted(matrix_names - set(indexed))

    # A published scenario may not leave the canonical matrix silently. The
    # matrix is the source of truth for what a batch owes, so deleting a row
    # from it would make the deletion invisible: the receipt would still be on
    # disk and still indexed, and the "missing receipt" direction would only
    # get shorter. The curated portrait rows of the Bevy matrix were deleted
    # exactly that way once, by a generator run, and the corpus stayed green
    # while eight published scenarios quietly stopped being owed.
    retired = sorted(set(indexed) - matrix_names)
    for name in retired:
        problems.append(
            f"{surface.name}: published scenario {name} is no longer in the canonical "
            f"matrix; a published scenario may not leave the matrix without a real "
            f"full batch and an explicit retirement record")

    if not pixels:
        # Index-only tree: the matrix coverage is judgeable, the pixels are not
        # in this checkout at all, so no pixel leg may be claimed of it.
        for name in missing:
            problems.append(f"{surface.name}: matrix scenario {name} has no published receipt")
        return problems

    uncited = sorted(set(pixels) - set(indexed))
    unindexed = sorted(set(indexed) - set(pixels))

    # Hard: an index row is a claim about a file that must exist and match.
    for name in unindexed:
        problems.append(f"{surface.name}: index names {name}.png which is not on disk")
    for name, row in sorted(indexed.items()):
        path = pixels.get(name)
        if path is None:
            continue
        digest = sha256(path)
        if row.get("sha256") != digest:
            problems.append(f"{surface.name}: {name}.png digest {digest[:12]} != "
                            f"indexed {str(row.get('sha256'))[:12]}")
            continue
        if row.get("bytes") and int(row["bytes"]) != path.stat().st_size:
            problems.append(f"{surface.name}: {name}.png is {path.stat().st_size} B, "
                            f"index says {row['bytes']} B")
        dimensions = png_dimensions(path)
        if dimensions is None:
            problems.append(f"{surface.name}: {name}.png is not a readable PNG")
            continue
        expected = str(row.get("requested_window") or "")
        actual = f"{dimensions[0]}x{dimensions[1]}"
        if expected and actual != expected:
            problems.append(f"{surface.name}: {name}.png is {actual}, "
                            f"index requested {expected}")

    # Ratcheted: an index that under-describes its own corpus can only be fixed
    # by a real full batch, so the two counts may shrink and never grow.
    entry = debt.get(surface.name)
    if uncited or missing:
        if entry is None:
            if uncited:
                problems.append(
                    f"{surface.name}: {len(uncited)} published screenshot(s) have no index row "
                    f"(first: {uncited[0]}.png) and no bounded divergence entry")
            for name in missing:
                problems.append(
                    f"{surface.name}: matrix scenario {name} has no published receipt")
        else:
            if len(uncited) > int(entry["uncited_max"]):
                problems.append(
                    f"{surface.name}: {len(uncited)} uncited screenshot(s) exceed the "
                    f"{entry['uncited_max']} bounded by {LEDGER.name}; a divergence may only shrink")
            if len(missing) > int(entry["receipt_gap_max"]):
                problems.append(
                    f"{surface.name}: {len(missing)} matrix scenario(s) have no receipt, over the "
                    f"{entry['receipt_gap_max']} bounded by {LEDGER.name}")
    if entry is not None and not uncited and not missing:
        problems.append(
            f"{surface.name}: {LEDGER} still bounds a divergence this corpus no longer has; "
            f"drop the entry")
    return problems


def harness_findings(root: pathlib.Path) -> list[str]:
    """The publish rule must exist once, in the harness all three matrices use."""
    problems: list[str] = []
    harness = (root / CAPTURE_HARNESS).read_text(encoding="utf-8")
    if "sweep_unindexed_pixels" not in harness:
        problems.append(f"{CAPTURE_HARNESS}: no shared unindexed-pixel sweep")
    for script in CAPTURE_MATRICES:
        text = (root / script).read_text(encoding="utf-8")
        label = script.name
        if f'source "$REPO/{CAPTURE_HARNESS.as_posix()}"' not in text:
            problems.append(f"{label}: does not source the shared capture harness")
        if "sweep_unindexed_pixels" not in text:
            problems.append(f"{label}: never reconciles published pixels with its index")
        if 'install -m 644 "$image" "$OUT_DIR/$name.png"' in text:
            problems.append(f"{label}: writes published pixels mid-run, so an interrupted "
                            f"batch leaves fresh pixels behind a stale index")
        if 'rm -f "$OUT_DIR/$name.png"' in text:
            problems.append(f"{label}: deletes published pixels before the run succeeds")
        if 'install -m 644 "$MANIFEST" "$MANIFEST_PUBLISHED"' not in text:
            problems.append(f"{label}: publishes no index")
    return problems


def matrix_size_findings(root: pathlib.Path) -> list[str]:
    """What a full batch owes has one source: the canonical matrix itself."""
    problems: list[str] = []
    shared = root / "scripts" / "parity" / "matrix_size.py"
    if not shared.is_file():
        return ["scripts/parity/matrix_size.py: the shared matrix count is missing"]
    if str(shared.parent) not in sys.path:
        sys.path.insert(0, str(shared.parent))
    import matrix_size

    declared = dict(matrix_size.CANONICAL_MATRIX)
    for surface in SURFACES:
        if declared.get(surface.name) != surface.matrix.as_posix():
            problems.append(
                f"matrix_size.py: surface {surface.name} must resolve to "
                f"{surface.matrix.as_posix()}, found {declared.get(surface.name)!r}")
    for script in MATRIX_SIZE_CONSUMERS:
        text = (root / script).read_text(encoding="utf-8")
        label = script.name
        typed = TYPED_MATRIX_SIZE.findall(text)
        if typed:
            problems.append(
                f"{label}: pins a matrix size ({', '.join(sorted(set(typed)))}) instead of "
                f"deriving it from the canonical matrix")
        if "matrix_scenario_count" not in text:
            problems.append(f"{label}: does not read the shared matrix size")
    return problems


def layout_findings(root: pathlib.Path) -> list[str]:
    """One truth for the corpus path: readers import it, writers carry it."""
    problems: list[str] = []
    shared = root / CORPUS_LAYOUT_MODULE
    if not shared.is_file():
        return [f"{CORPUS_LAYOUT_MODULE}: the shared corpus layout is missing"]
    for consumer in CORPUS_LAYOUT_CONSUMERS:
        text = (root / consumer).read_text(encoding="utf-8")
        if "corpus_layout" not in text:
            problems.append(f"{consumer}: does not read the shared corpus layout")
    for script in CAPTURE_MATRICES:
        text = (root / script).read_text(encoding="utf-8")
        label = script.name
        if CORPUS_PATH_LITERAL not in text:
            problems.append(f"{label}: does not publish into {CORPUS_PATH_LITERAL}")
        if "docs/screenshots" in text:
            problems.append(f"{label}: still publishes into a docs/ corpus directory")
    return problems


def violations(root: pathlib.Path) -> list[str]:
    debt = load_debt(root)
    problems = (harness_findings(root) + matrix_size_findings(root)
                + layout_findings(root))
    for surface in SURFACES:
        problems.extend(corpus_findings(surface, root, debt))
    return problems


def corpus_counts(root: pathlib.Path) -> dict[str, tuple[int, int]]:
    """Per-surface (uncited pixels, matrix scenarios without a receipt)."""
    counts = {}
    for surface in SURFACES:
        out = root / surface.out_dir
        index_path = out / "manifest.tsv"
        if not index_path.is_file():
            counts[surface.name] = (0, 0)
            continue
        indexed = {row["scenario"] for row in read_index(index_path)
                   if row.get("status") == "ok"}
        pixels = {path.stem for path in out.glob("*.png")}
        missing = set(read_matrix_names(surface, root)) - indexed
        counts[surface.name] = (len(pixels - indexed), len(missing))
    return counts


def tighten_debt(root: pathlib.Path) -> int:
    """Rewrite the ledger to the current divergence, never a larger one.

    Whoever recaptures a corpus must not have to hand-edit a bound, and must not
    be able to loosen one either: a surface may only move down, and a surface
    that reaches zero on both counts loses its entry.
    """
    path = root / LEDGER
    debt = load_debt(root)
    counts = corpus_counts(root)
    out_dirs = {surface.name: root / surface.out_dir for surface in SURFACES}
    changed: list[str] = []
    for surface, (uncited, gap) in counts.items():
        out = out_dirs[surface]
        if not any(out.glob("*.png")):
            # No local pixels: the divergence cannot be measured here, so the
            # bound must not move. A tree without the corpus says nothing about
            # how far the corpus has healed.
            print(f"capture-corpus-integrity: {surface} has no local pixels; "
                  f"bound untouched")
            continue
        entry = debt.get(surface)
        if not uncited and not gap:
            if entry is not None:
                del debt[surface]
                changed.append(f"{surface}: retired (index and corpus agree)")
            continue
        if entry is None:
            raise SystemExit(f"{LEDGER}: refusing to open a new divergence bound for {surface} "
                             f"({uncited} uncited, {gap} scenarios without a receipt); "
                             f"a divergence must be fixed by a recapture, not registered")
        for key, value in (("uncited_max", uncited), ("receipt_gap_max", gap)):
            if value < int(entry[key]):
                changed.append(f"{surface}: {key} {entry[key]} -> {value}")
                entry[key] = value
    payload = {"divergent_corpora": dict(sorted(debt.items())), "version": 1}
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    for line in changed:
        print(f"capture-corpus-integrity: tightened {line}")
    if not changed:
        print("capture-corpus-integrity: nothing tightened")
    return 0


def summarize(root: pathlib.Path) -> list[str]:
    """Per-surface counts, for the operator's report and the ledger entry."""
    lines = []
    debt = load_debt(root)
    for surface in SURFACES:
        out = root / surface.out_dir
        index_path = out / "manifest.tsv"
        if not index_path.is_file():
            lines.append(f"{surface.name}: no index")
            continue
        rows = read_index(index_path)
        indexed = {r["scenario"] for r in rows if r.get("status") == "ok"}
        pixels = {p.stem for p in out.glob("*.png")}
        matrix = set(read_matrix_names(surface, root))
        lines.append(
            f"{surface.name}: index={len(indexed)} png={len(pixels)} "
            f"uncited={len(pixels - indexed)} matrix={len(matrix)} "
            f"matrix_without_receipt={len(matrix - indexed)} "
            f"pinned={'yes' if surface.name in debt else 'no'}")
    return lines


class Harness(unittest.TestCase):
    """Planted negatives: each rule must bite on a synthetic corpus."""

    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self._temporary.name)
        self.surface = Surface("slint", pathlib.Path("out"), pathlib.Path("matrix.tsv"), 0)
        (self.root / "out").mkdir()
        (self.root / "matrix.tsv").write_text(
            "name\tpage\nalpha\toverview\nbeta\tlogs\n", encoding="utf-8")

    def tearDown(self) -> None:
        self._temporary.cleanup()

    def index(self, rows: list[tuple[str, str, int, str]]) -> pathlib.Path:
        lines = ["\t".join(MANIFEST_FIELDS)]
        for name, window, size, digest in rows:
            width, height = window.split("x")
            lines.append("\t".join([name, "overview", "dark", window, "1", "2",
                                    width, height, str(size), digest, "ok"]))
        path = self.root / "out" / "manifest.tsv"
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        return path

    def pixel(self, name: str, body: bytes = b"pixels") -> pathlib.Path:
        path = self.root / "out" / f"{name}.png"
        header = (b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR"
                  + struct.pack(">II", 1180, 780) + bytes(9))
        path.write_bytes(header + body)
        return path

    @staticmethod
    def digest(path: pathlib.Path) -> str:
        return sha256(path)

    def test_a_consistent_corpus_and_harness_pass(self) -> None:
        alpha = self.pixel("alpha")
        beta = self.pixel("beta")
        self.index([("alpha", "1180x780", alpha.stat().st_size, self.digest(alpha)),
                    ("beta", "1180x780", beta.stat().st_size, self.digest(beta))])
        self.assertEqual(corpus_findings(self.surface, self.root, {}), [])
        self.assertEqual(harness_findings(ROOT), [], "the shipped harness must pass")

    def test_an_uncited_pixel_without_a_pinned_entry_is_rejected(self) -> None:
        alpha = self.pixel("alpha")
        self.pixel("ghost")
        self.index([("alpha", "1180x780", alpha.stat().st_size, self.digest(alpha))])
        problems = corpus_findings(self.surface, self.root, {})
        self.assertTrue(any("have no index row" in problem for problem in problems), problems)

    def test_a_bounded_divergence_may_shrink_but_never_grow(self) -> None:
        alpha = self.pixel("alpha")
        self.pixel("ghost")
        self.index([("alpha", "1180x780", alpha.stat().st_size, self.digest(alpha))])
        debt = {"slint": {"uncited_max": 0, "receipt_gap_max": 0, "reason": "cannot happen yet"}}
        self.assertTrue(any("may only shrink" in problem
                            for problem in corpus_findings(self.surface, self.root, debt)))
        debt = {"slint": {"uncited_max": 1, "receipt_gap_max": 1, "reason": "one frame short"}}
        beta = self.pixel("beta")
        rows = [("alpha", "1180x780", alpha.stat().st_size, self.digest(alpha))]
        self.index(rows)
        problems = corpus_findings(self.surface, self.root, debt)
        self.assertTrue(any("may only shrink" in problem for problem in problems), problems)
        self.assertTrue(any("over the 1 bounded" not in problem for problem in problems),
                        "beta is a matrix scenario, so one receipt gap is inside the bound")
        rows.append(("beta", "1180x780", beta.stat().st_size, self.digest(beta)))
        self.index(rows)
        (self.root / "out" / "ghost.png").unlink()
        problems = corpus_findings(self.surface, self.root, debt)
        self.assertTrue(any("no longer has" in problem for problem in problems), problems)

    def test_a_bounded_receipt_gap_is_reported_when_it_grows(self) -> None:
        alpha = self.pixel("alpha")
        self.pixel("beta")
        self.index([("alpha", "1180x780", alpha.stat().st_size, self.digest(alpha))])
        debt = {"slint": {"uncited_max": 1, "receipt_gap_max": 0, "reason": "tight"}}
        problems = corpus_findings(self.surface, self.root, debt)
        self.assertTrue(any("matrix scenario(s) have no receipt" in problem
                            for problem in problems), problems)

    def test_a_ledger_entry_must_be_justified(self) -> None:
        path = self.root / LEDGER
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"version": 1, "divergent_corpora": {
            "slint": {"uncited_max": 5, "receipt_gap_max": 5}}}), encoding="utf-8")
        with self.assertRaises(ValueError) as caught:
            load_debt(self.root)
        self.assertIn("reason", str(caught.exception))
        path.write_text(json.dumps({"version": 2, "divergent_corpora": {}}), encoding="utf-8")
        with self.assertRaises(ValueError):
            load_debt(self.root)

    def test_a_lie_in_the_index_is_rejected_however_it_is_pinned(self) -> None:
        alpha = self.pixel("alpha")
        self.index([("alpha", "1180x780", alpha.stat().st_size, "f" * 64)])
        self.assertTrue(any("digest" in problem
                            for problem in corpus_findings(self.surface, self.root, {})))
        self.index([("alpha", "1180x780", 999, self.digest(alpha))])
        self.assertTrue(any("index says" in problem
                            for problem in corpus_findings(self.surface, self.root, {})))
        self.index([("alpha", "800x600", alpha.stat().st_size, self.digest(alpha))])
        self.assertTrue(any("index requested" in problem
                            for problem in corpus_findings(self.surface, self.root, {})))

    def test_an_index_row_without_a_pixel_is_rejected(self) -> None:
        alpha = self.pixel("alpha")
        self.pixel("beta")
        self.index([("alpha", "1180x780", alpha.stat().st_size, self.digest(alpha)),
                    ("missing", "1180x780", alpha.stat().st_size, self.digest(alpha))])
        alpha.unlink()
        problems = corpus_findings(self.surface, self.root, {})
        self.assertTrue(any("not on disk" in problem for problem in problems), problems)

    def test_a_tree_without_pixels_is_judged_on_its_index_only(self) -> None:
        """A checkout that never captured holds the index but no pixels.

        The index is the tracked half of the corpus, so the matrix legs still
        run there. The pixel legs must not claim anything about pixels that
        are not in this tree at all: a corpus that was never published is not
        a corpus with broken pixels.
        """
        alpha = self.pixel("alpha")
        self.index([("alpha", "1180x780", alpha.stat().st_size, self.digest(alpha))])
        alpha.unlink()
        problems = corpus_findings(self.surface, self.root, {})
        self.assertFalse(any("not on disk" in problem for problem in problems), problems)
        self.assertTrue(any("matrix scenario beta has no published receipt" in problem
                            for problem in problems), problems)

    def test_a_matrix_scenario_without_a_receipt_is_rejected(self) -> None:
        alpha = self.pixel("alpha")
        self.index([("alpha", "1180x780", alpha.stat().st_size, self.digest(alpha))])
        problems = corpus_findings(self.surface, self.root, {})
        self.assertTrue(any("matrix scenario beta has no published receipt" in problem
                            for problem in problems), problems)

    def test_a_published_scenario_that_leaves_the_matrix_is_rejected(self) -> None:
        """The matrix may not shrink a published scenario away silently.

        The matrix is the source of truth for what a batch owes, so a deleted
        row used to make the deletion invisible: the receipt stayed on disk
        and stayed indexed, and the "missing receipt" direction only got
        shorter. Eight curated Bevy portrait rows were deleted exactly that
        way by a generator run while the corpus stayed green.
        """
        alpha = self.pixel("alpha")
        self.index([("alpha", "1180x780", alpha.stat().st_size, self.digest(alpha))])
        (self.root / "matrix.tsv").write_text(
            "name\tpage\nbeta\tlogs\n", encoding="utf-8")
        problems = corpus_findings(self.surface, self.root, {})
        self.assertTrue(any("no longer in the canonical" in problem
                            for problem in problems), problems)

    def test_a_reader_that_re_types_the_matrix_size_is_rejected(self) -> None:
        """One source for what a full batch owes.

        Three readers kept their own copy of 211 (two literals and one
        inference from the largest count ever published), so growing the matrix
        made each wrong in a different direction: two silently refused to
        recognise the new publication, one let a single interrupted run
        redefine "full" forever.
        """
        real = pathlib.Path(__file__).resolve().parents[2]
        for relative in [pathlib.Path("scripts/parity/matrix_size.py"),
                         *MATRIX_SIZE_CONSUMERS]:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(real / relative, target)
        culprit = self.root / MATRIX_SIZE_CONSUMERS[1]
        culprit.write_text('FULL_MATRIX_SIZE = 211\n', encoding="utf-8")
        problems = matrix_size_findings(self.root)
        self.assertTrue(any("pins a matrix size" in problem for problem in problems),
                        problems)
        # A reader that stopped consulting the shared source is caught even when
        # it no longer types a number of its own.
        culprit.write_text('REQUIRED = len(open("x").readlines())\n', encoding="utf-8")
        problems = matrix_size_findings(self.root)
        self.assertTrue(any("does not read the shared matrix size" in problem
                            for problem in problems), problems)
        culprit.write_text('FULL_MATRIX_SIZE = matrix_scenario_count("slint", ROOT)\n'
                           'matrix_scenario_count\n', encoding="utf-8")
        self.assertEqual(matrix_size_findings(self.root), [])

    def test_a_malformed_index_header_is_rejected(self) -> None:
        (self.root / "out" / "manifest.tsv").write_text("scenario\tpage\n", encoding="utf-8")
        with self.assertRaises(ValueError):
            corpus_findings(self.surface, self.root, {})


def self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(Harness)
    result = unittest.TextTestRunner(verbosity=0).run(suite)
    print(f"capture-corpus-integrity: self-test {result.testsRun - len(result.failures) - len(result.errors)}"
          f"/{result.testsRun} passed")
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=("enforce", "report"), default="enforce")
    parser.add_argument("--summarize", action="store_true")
    parser.add_argument("--tighten-debt", action="store_true",
                        help="lower the bounded divergence to what is on disk now")
    parser.add_argument("--root", type=pathlib.Path, default=ROOT)
    args = parser.parse_args(argv)

    if args.self_test:
        return self_test()
    if args.tighten_debt:
        return tighten_debt(args.root)
    problems = violations(args.root)
    if args.summarize:
        for line in summarize(args.root):
            print(line)
    for problem in problems:
        print(f"capture-corpus-integrity: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(f"capture-corpus-integrity: findings={len(problems)} "
          f"across {len(SURFACES)} published corpora")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
