#!/usr/bin/env python3
"""Dynamic documentation-governance guard.

Validates the documentation map by *pattern*, never by enumerating literal
document contents:

1. Every in-map document declares the authority level assigned to its scope
   (`层级：Lx`); documents cannot lower their own level to bypass link checks.
2. Relative links resolve (the ignored local archive and build `target/**`
   artifacts are out of scope).
3. Reference direction is same-or-down: links and literal Markdown path references may not cite a higher-level document.
4. When the local ignored `TODO.md` exists, referenced task IDs are registered.

Map membership and assigned authority are derived from path structure, with a
single explicit L2 scope for the shared cross-surface contract. Declared levels
are checked against that assignment before links are evaluated.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import tempfile
import urllib.parse

LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
DOC_REFERENCE_RE = re.compile(r"(?<![\w./:-])(?:\.\.?/|[\w.-]+/)*[\w.-]+\.md(?:#[\w./-]+)?")
INLINE_CODE_RE = re.compile(r"`([^`]+)`")
LEVEL_RE = re.compile(r"层级：\s*(?:L\s*)?(?P<level>\d+)|层级：\s*终端")
TASK_CODE_RE = re.compile(r"\b([A-Z]{2,10}-\d{3,4})(?!-)\b")
IGNORED_CODES = {"SHA-256", "AES-256", "SHA-512", "UTF-8", "UTF-16", "ISO-8859"}

def rel_parts(path: pathlib.Path, root: pathlib.Path) -> tuple[str, ...]:
    return path.relative_to(root).parts


def doc_level(rel: str) -> int:
    """Assigned authority by document scope; -1 means 'outside the map'."""
    if rel == "AGENTS.md":
        return 0
    if rel in ("README.md", "TESTING.md", "THIRD-PARTY-NOTICES.md"):
        return 1
    if rel == "TODO.md":
        return 4
    if rel.startswith("todos/") and rel.endswith(".md"):
        return 4
    if rel == "docs/README.md":
        return 1
    if rel.startswith("docs/archive/"):
        return -1
    if rel.startswith("docs/") and rel.count("/") == 1 and rel.endswith(".md"):
        return 2
    if rel.startswith("docs/"):
        return 3
    # The shared contract is authoritative across product surfaces despite
    # living in a crate directory. Other crate documentation stays L3.
    if rel.startswith("crates/infiltrator-contract/") and rel.endswith(".md"):
        return 2
    if rel.startswith(("crates/", "android/")) and rel.endswith(".md"):
        return 3
    if rel.startswith("scripts/") and rel.endswith(".md"):
        return 3
    return -1


def declared_doc_level(rel: str, text: str) -> int:
    """Authority declared by the document, or -1 when missing/unmapped."""
    if doc_level(rel) < 0:
        return -1
    head = "\n".join(text.splitlines()[:12])
    match = LEVEL_RE.search(head)
    if match is None:
        return -1
    level = match.group("level")
    return int(level) if level is not None else 4


def markdown_files(root: pathlib.Path) -> list[pathlib.Path]:
    result = []
    for p in root.rglob("*.md"):
        parts = rel_parts(p, root)
        if any(part in ("target", "node_modules", "vendor", "dist") for part in parts):
            continue
        if parts[:2] == ("docs", "archive"):
            continue
        if any(part.startswith(".") for part in parts[:-1]):
            continue
        result.append(p)
    return sorted(result)


def document_paths_by_name(root: pathlib.Path) -> dict[str, list[pathlib.Path]]:
    paths: dict[str, list[pathlib.Path]] = {}
    for document in markdown_files(root):
        paths.setdefault(document.name, []).append(document)
    return paths


def document_paths_by_stem(root: pathlib.Path) -> dict[str, list[pathlib.Path]]:
    paths: dict[str, list[pathlib.Path]] = {}
    for document in markdown_files(root):
        paths.setdefault(document.stem, []).append(document)
    return paths


def extract_valid_tasks(root: pathlib.Path) -> set[str]:
    """Task codes registered by the local ignored ledger: root TODO.md plus todos/*.md boards."""
    codes: set[str] = set()
    todo = root / "TODO.md"
    if todo.exists():
        codes |= set(TASK_CODE_RE.findall(todo.read_text(encoding="utf-8")))
    todos_dir = root / "todos"
    if todos_dir.is_dir():
        for board in sorted(todos_dir.glob("*.md")):
            codes |= set(TASK_CODE_RE.findall(board.read_text(encoding="utf-8")))
    return codes


def check_file(
    path: pathlib.Path,
    root: pathlib.Path,
    valid_tasks: set[str] | None = None,
    documents_by_name: dict[str, list[pathlib.Path]] | None = None,
    documents_by_stem: dict[str, list[pathlib.Path]] | None = None,
) -> list[str]:
    rel = path.relative_to(root).as_posix()
    text = path.read_text(encoding="utf-8")
    violations: list[str] = []
    source_level = doc_level(rel)
    names = documents_by_name or document_paths_by_name(root)
    stems = documents_by_stem or document_paths_by_stem(root)
    declared_level = declared_doc_level(rel, text)
    if source_level < 0:
        violations.append("unmapped Markdown document; assign it to a documentation level")
    elif declared_level < 0:
        violations.append("missing `层级：Lx` header")
    elif declared_level != source_level:
        violations.append(
            f"declares L{declared_level} but its scope is L{source_level}"
        )
    if valid_tasks is not None and path.name != "TODO.md":
        for code in TASK_CODE_RE.findall(text):
            if code not in IGNORED_CODES and code not in valid_tasks:
                violations.append(f"invalid task code '{code}'")
    for match in LINK_RE.finditer(text):
        link = match.group(1).strip()
        if link.startswith(("http://", "https://", "mailto:", "/", "#")) or not link:
            continue
        path_part = urllib.parse.unquote(link.split("#", 1)[0])
        if not path_part:
            continue
        target = (path.parent / path_part).resolve()
        try:
            target_rel = target.relative_to(root.resolve()).as_posix()
        except ValueError:
            continue
        if target_rel.startswith("target/"):
            continue
        if target_rel.split("/", 1)[0].startswith("."):
            # The local ledger links into gitignored local artifacts (.evidence/,
            # .private/, ...). Those never exist in a CI checkout, so they are not
            # broken links; markdown_files already skips the same dot-directories.
            continue
        if not target.exists():
            violations.append(f"broken relative link '{link}'")
            continue
        if source_level >= 0 and target_rel.endswith(".md"):
            target_level = doc_level(target_rel)
            if target_level >= 0 and target_level < source_level:
                violations.append(f"upward link to L{target_level} '{link}' (source L{source_level})")
    for match in DOC_REFERENCE_RE.finditer(text):
        reference = urllib.parse.unquote(match.group(0).split("#", 1)[0])
        if not reference:
            continue
        target = (path.parent / reference).resolve()
        if not target.exists():
            target = (root / reference).resolve()
        if not target.exists():
            candidates = names.get(pathlib.Path(reference).name, [])
            if len(candidates) == 1:
                target = candidates[0].resolve()
            else:
                continue
        try:
            target_rel = target.relative_to(root.resolve()).as_posix()
        except ValueError:
            continue
        if not target.exists():
            continue
        target_level = doc_level(target_rel)
        if source_level >= 0 and target_level >= 0 and target_level < source_level:
            violations.append(
                f"upward document reference to L{target_level} '{reference}' (source L{source_level})"
            )
    for match in INLINE_CODE_RE.finditer(text):
        reference = match.group(1).strip()
        if not reference or reference.endswith(".md"):
            continue
        basename = pathlib.Path(reference).name
        candidates = stems.get(basename, [])
        if len(candidates) != 1:
            continue
        target_rel = candidates[0].relative_to(root).as_posix()
        target_level = doc_level(target_rel)
        if source_level >= 0 and target_level >= 0 and target_level < source_level:
            violations.append(
                f"upward document reference to L{target_level} '{reference}' (source L{source_level})"
            )
    return violations


def self_test() -> int:
    assert LINK_RE.findall("[a](x.md) [b](http://e.com)") == ["x.md", "http://e.com"]
    assert TASK_CODE_RE.findall("CORE-001 and invalid ABC-12") == ["CORE-001"]
    assert TASK_CODE_RE.findall("CORE-1000 and compound CORE-040-01") == ["CORE-1000"]
    assert doc_level("AGENTS.md") == 0
    assert doc_level("README.md") == 1
    assert doc_level("TESTING.md") == 1
    assert doc_level("THIRD-PARTY-NOTICES.md") == 1
    assert doc_level("TODO.md") == 4
    assert doc_level("todos/CODE_TODO.md") == 4
    assert doc_level("todos/CODE_TODO_PAST.md") == 4
    assert doc_level("todos/CODE_TODO_FUTURE.md") == 4
    assert doc_level("docs/README.md") == 1
    assert doc_level("docs/ARCHITECTURE.md") == 2
    assert doc_level("crates/infiltrator-bevy-ui/docs/README.md") == 3
    assert doc_level("crates/infiltrator-bevy-ui/docs/BEVY_UI_FRONTEND.md") == 3
    assert doc_level("crates/infiltrator-contract/docs/UI_PARITY_AUDIT.md") == 2
    assert doc_level("android/docs/BEVY_ANDROID_PRODUCT.md") == 3
    assert doc_level("docs/archive/X.md") == -1
    assert doc_level("crates/infiltrator-application/README.md") == 3
    assert doc_level("crates/mihomo-dav-sync/sync-engine/README.md") == 3
    assert doc_level("android/bevy-host/driver/README.md") == 3
    assert doc_level("crates/infiltrator-bevy-widgets/assets/fonts/README.md") == 3
    assert doc_level("README.md") == 1
    assert declared_doc_level(
        "crates/infiltrator-bevy-ui/docs/BEVY_020_UI_CAPABILITIES.md",
        "# title\n\n> 层级：L3\n",
    ) == 3
    assert declared_doc_level(
        "crates/infiltrator-contract/docs/UI_PARITY_AUDIT.md",
        "# title\n\n> 层级：L2\n",
    ) == 2
    with tempfile.TemporaryDirectory() as temporary:
        root = pathlib.Path(temporary)
        low = root / "crates/pkg/README.md"
        high = root / "docs/ARCHITECTURE.md"
        low.parent.mkdir(parents=True)
        (root / "docs").mkdir(parents=True)
        low.write_text("# Low\n\n> 层级：L3\n\n[High](../../docs/ARCHITECTURE.md)\n", encoding="utf-8")
        high.write_text("# High\n\n> 层级：L2\n", encoding="utf-8")
        assert any("upward link to L2" in issue for issue in check_file(low, root))

        low.write_text("# Low\n\n> 层级：L1\n\n[High](../../docs/ARCHITECTURE.md)\n", encoding="utf-8")
        assert any("declares L1 but its scope is L3" in issue for issue in check_file(low, root))

        high.write_text("# High\n\n> 层级：L2\n\n[Low](../crates/pkg/README.md)\n", encoding="utf-8")
        assert check_file(high, root) == []

        (root / "TESTING.md").write_text("# Tests\n\n> 层级：L1\n", encoding="utf-8")
        low.write_text("# Low\n\n> 层级：L3\n\n[Tests](../../TESTING.md)\n", encoding="utf-8")
        assert any("upward link to L1" in issue for issue in check_file(low, root))

        low.write_text("# Low\n\n> 层级：L3\n\nImplementation policy is in ../../docs/ARCHITECTURE.md.\n", encoding="utf-8")
        assert any("upward document reference to L2" in issue for issue in check_file(low, root))

        low.write_text("# Low\n\n> 层级：L3\n\nSee `ARCHITECTURE.md` for implementation policy.\n", encoding="utf-8")
        assert any("upward document reference to L2" in issue for issue in check_file(low, root))

        todo = root / "TODO.md"
        todo.write_text(
            "# Local ledger\n\n> 层级：L4\n\n[Charter](docs/ARCHITECTURE.md)\n",
            encoding="utf-8",
        )
        assert any("upward link to L2" in issue for issue in check_file(todo, root))

        todo.write_text(
            "# Local ledger\n\n> 层级：L4\n\n[Evidence](.evidence/old-board.md)\n",
            encoding="utf-8",
        )
        assert check_file(todo, root) == []

        unclassified = root / "misc/NOTE.md"
        unclassified.parent.mkdir(parents=True)
        unclassified.write_text("# Note\n", encoding="utf-8")
        assert any("unmapped Markdown document" in issue for issue in check_file(unclassified, root))

        (root / "TODO.md").write_text("# TODO\n\n- CORE-100\n", encoding="utf-8")
        low.write_text("# Low\n\n> 层级：L3\n\nTask CORE-101.\n", encoding="utf-8")
        assert any("invalid task code 'CORE-101'" in issue for issue in check_file(low, root, {"CORE-100"}))
    print("self-test OK")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()

    root = pathlib.Path(__file__).resolve().parents[2]
    todo = root / "TODO.md"
    valid_tasks = extract_valid_tasks(root) if todo.exists() else None
    documents_by_name = document_paths_by_name(root)
    documents_by_stem = document_paths_by_stem(root)
    violations: dict[str, list[str]] = {}
    for md in markdown_files(root):
        found = check_file(md, root, valid_tasks, documents_by_name, documents_by_stem)
        if found:
            violations[md.relative_to(root).as_posix()] = found

    if not violations:
        print("doc-governance-guard: OK")
        return 0

    print("doc-governance-guard: Found violations:")
    for rel, found in violations.items():
        print(f"  {rel}:")
        for item in found:
            print(f"    - {item}")
    return 1 if args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
