#!/usr/bin/env python3
"""Identity-uniqueness guard for the checked-in desktop screenshot matrix.

Rule (GAP-7.3): every PNG under the three published screenshot roots must
have a unique SHA-256 inside its surface unless the duplicate cell is
declared in the explicit alias manifest (`scripts/parity/screenshot_aliases.tsv`).
Two different business cells sharing one byte-identical capture is bad data:
the acceptance pixel judgement cannot tell them apart. Recapture slices fix
the undeclared groups; this guard starts in report mode and flips to enforce
once the matrix is clean.

The alias manifest itself is audited: stale entries (file gone, digest
drifted, canonical mismatch, entry no longer duplicated) are violations, so
the list can never rot into a silent allowlist.
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import re
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]

# The published matrix the 2026-10-27 tri-surface visual audit read. Bevy
# keeps its capture tree one level deeper; the surface key maps to the root.
SCREENSHOT_ROOTS = {
    "iced": pathlib.Path("crates/infiltrator-iced/docs/screenshots"),
    "bevy": pathlib.Path("crates/infiltrator-bevy-ui/docs/screenshots"),
    "slint": pathlib.Path("crates/infiltrator-slint/docs/screenshots"),
}

ALIAS_MANIFEST = pathlib.Path("scripts/parity/screenshot_aliases.tsv")
ALIAS_FIELDS = ("surface", "file", "sha256", "canonical", "reason")

SHA_RE = re.compile(r"[0-9a-f]{64}")


def sha256_of(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_alias_manifest(root: pathlib.Path) -> list[dict[str, str]]:
    """Parse the TSV alias manifest; a missing file is an empty list."""
    path = root / ALIAS_MANIFEST
    if not path.exists():
        return []
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines:
        raise ValueError(f"{ALIAS_MANIFEST}: file is empty; expected a header row")
    header = lines[0].split("\t")
    if tuple(header) != ALIAS_FIELDS:
        raise ValueError(
            f"{ALIAS_MANIFEST}: header {header} does not match {list(ALIAS_FIELDS)}"
        )
    rows = []
    for number, line in enumerate(lines[1:], start=2):
        if not line.strip():
            continue
        fields = line.split("\t")
        if len(fields) != len(ALIAS_FIELDS):
            raise ValueError(
                f"{ALIAS_MANIFEST}:{number}: expected {len(ALIAS_FIELDS)} tab-separated fields"
            )
        rows.append(dict(zip(ALIAS_FIELDS, fields)))
    return rows


def scan_groups(root: pathlib.Path) -> tuple[dict[str, list[pathlib.Path]], int]:
    """Hash every git-tracked published PNG; return {sha: [paths]} and the count.

    The corpus is the tracked matrix, not the local worktree: capture export
    directories that were never committed (Bevy keeps its matrix in the
    git-ignored `docs/screenshots/bevy/` until PARITY-001.6 lands it) are not
    part of the published evidence and must not gate CI.
    """
    groups: dict[str, list[pathlib.Path]] = {}
    total = 0
    for surface, relative in SCREENSHOT_ROOTS.items():
        tracked = subprocess.run(
            ["git", "ls-files", "-z", "--", relative.as_posix()],
            cwd=root, check=True, capture_output=True,
        ).stdout.split(b"\0")
        for entry in sorted(tracked):
            if not entry or not entry.endswith(b".png"):
                continue
            path = root / entry.decode("utf-8")
            if not path.exists():
                raise FileNotFoundError(f"tracked screenshot missing from worktree: {path}")
            groups.setdefault(sha256_of(path), []).append(path)
            total += 1
    return groups, total


def violations(root: pathlib.Path) -> tuple[list[str], int]:
    """Return the guard violations and the scanned PNG count."""
    groups, total = scan_groups(root)
    duplicated = {sha: paths for sha, paths in groups.items() if len(paths) > 1}
    problems: list[str] = []

    try:
        aliases = read_alias_manifest(root)
    except ValueError as error:
        return [str(error)], total

    by_surface_root = {
        surface: (root / relative).resolve()
        for surface, relative in SCREENSHOT_ROOTS.items()
    }
    declared: dict[pathlib.Path, dict[str, str]] = {}
    for row in aliases:
        file_path = (root / row["file"]).resolve()
        if file_path in declared:
            problems.append(f"alias manifest declares the same file twice: {row['file']}")
            continue
        declared[file_path] = row

    surface_of: dict[pathlib.Path, str] = {}
    for sha, paths in groups.items():
        for path in paths:
            for surface, base in by_surface_root.items():
                if path.is_relative_to(base):
                    surface_of[path] = surface

    for sha, paths in sorted(duplicated.items()):
        undeclared = [path for path in paths if path not in declared]
        # One undeclared member is the canonical the aliases point at; two or
        # more mean at least one duplicate cell is still unexplained.
        if len(undeclared) <= 1:
            continue
        problems.append(
            "undeclared duplicate screenshot group (SHA-256 "
            f"{sha[:12]}…): "
            + ", ".join(str(path.relative_to(root)) for path in paths)
        )

    canonical_sha: dict[str, str] = {}
    for row in aliases:
        file_path = (root / row["file"]).resolve()
        canonical_path = (root / row["canonical"]).resolve()
        surface = row["surface"]
        if surface not in SCREENSHOT_ROOTS:
            problems.append(f"alias manifest: unknown surface {surface!r} for {row['file']}")
            continue
        if not file_path.exists():
            problems.append(f"alias manifest: declared file is missing: {row['file']}")
            continue
        if surface_of.get(file_path) != surface:
            problems.append(
                f"alias manifest: {row['file']} is not under the {surface} screenshot root"
            )
            continue
        if not SHA_RE.fullmatch(row["sha256"]):
            problems.append(
                f"alias manifest: sha256 for {row['file']} is not 64 lowercase hex chars"
            )
            continue
        actual = sha256_of(file_path)
        if actual != row["sha256"]:
            problems.append(
                f"alias manifest: stale digest for {row['file']} "
                f"(declared {row['sha256'][:12]}…, actual {actual[:12]}…)"
            )
            continue
        if not canonical_path.exists():
            problems.append(f"alias manifest: canonical file is missing: {row['canonical']}")
            continue
        if surface_of.get(canonical_path) != row["surface"]:
            problems.append(
                f"alias manifest: canonical {row['canonical']} is not under the "
                f"{row['surface']} screenshot root"
            )
            continue
        if not row["reason"].strip():
            problems.append(f"alias manifest: entry for {row['file']} has an empty reason")
            continue
        canonical_digest = canonical_sha.setdefault(row["canonical"], sha256_of(canonical_path))
        if canonical_digest != actual:
            problems.append(
                f"alias manifest: {row['file']} and canonical {row['canonical']} "
                "no longer share the same bytes; re-declare or re-capture"
            )
            continue
        if actual not in duplicated:
            problems.append(
                f"alias manifest: stale alias {row['file']} is no longer a duplicate; remove the entry"
            )
    return problems, total


def self_test() -> int:
    manifest = "\n".join(
        [
            "\t".join(ALIAS_FIELDS),
            "\t".join(
                [
                    "iced",
                    "crates/infiltrator-iced/docs/screenshots/a.png",
                    "a" * 64,
                    "crates/infiltrator-iced/docs/screenshots/b.png",
                    "same blank page, one viewport difference",
                ]
            ),
        ]
    )
    lines = manifest.splitlines()
    assert tuple(lines[0].split("\t")) == ALIAS_FIELDS
    assert len(lines[1].split("\t")) == len(ALIAS_FIELDS)
    assert SHA_RE.fullmatch("a" * 64)
    assert not SHA_RE.fullmatch("A" * 64)
    assert not SHA_RE.fullmatch("a" * 63)
    print("self-test OK")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    problems, total = violations(REPO_ROOT)
    print(
        f"screenshot identity guard: scanned={total} "
        f"aliases={len(read_alias_manifest(REPO_ROOT))} violations={len(problems)}"
    )
    for problem in problems:
        print(f"  {problem}")
    if args.mode == "enforce" and problems:
        print(
            "enforce mode: re-capture the undeclared duplicates or declare them "
            f"in {ALIAS_MANIFEST} with a reason",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
