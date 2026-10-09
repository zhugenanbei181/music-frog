#!/usr/bin/env python3
"""Dynamic documentation-governance guard.

Validates the documentation map by *pattern*, never by enumerating literal
document contents:

1. Every in-map document declares its authority level (`层级：Lx`).
2. Relative links resolve (frozen `docs/archive/**` and build `target/**`
   artifacts are out of scope).
3. Link direction is same-or-down: a document may not cite a higher-level one.
4. Task codes match the shared format; when `TODO.md` exists, referenced codes
   must be registered there.

Levels are derived from path structure plus the documented charter set, not from
a per-document allow/deny list.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import urllib.parse

# Task code: 2-10 uppercase letters, hyphen, 3-4 digits. A trailing `-<digit>`
# marks a longer compound code (e.g. `CORE-040-01`) and is not a bare task code.
TASK_CODE_RE = re.compile(r"\b([A-Z]{2,10}-\d{3,4})(?!-)\b")
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
LEVEL_RE = re.compile(r"层级：\s*L?\d|层级：\s*终端")

IGNORED_CODES = {"SHA-256", "AES-256", "SHA-512", "UTF-8", "UTF-16", "ISO-8859"}

# Charters carry L2 authority even though they live inside a UI directory.
L2_CHARTERS = {
    "docs/bevy-ui/BEVY_UI_FRONTEND.md",
    "docs/android/BEVY_ANDROID_PRODUCT.md",
}


def rel_parts(path: pathlib.Path, root: pathlib.Path) -> tuple[str, ...]:
    return path.relative_to(root).parts


def doc_level(rel: str) -> int:
    """Authority level for an in-map doc; -1 means 'outside the map'."""
    if rel == "AGENTS.md":
        return 0
    if rel == "docs/README.md":
        return 1
    if rel.startswith("docs/archive/"):
        return 4
    if rel in L2_CHARTERS:
        return 2
    if rel.startswith("docs/") and rel.count("/") == 1 and rel.endswith(".md"):
        return 2
    if rel.startswith("docs/") and rel.endswith("/README.md"):
        return 2
    if rel.startswith("docs/"):
        return 3
    return -1


def is_in_map(rel: str) -> bool:
    return doc_level(rel) >= 0


def markdown_files(root: pathlib.Path) -> list[pathlib.Path]:
    result = []
    for p in root.rglob("*.md"):
        parts = rel_parts(p, root)
        if any(part in ("target", "node_modules", "vendor", "dist") for part in parts):
            continue
        if any(part.startswith(".") for part in parts[:-1]):
            continue
        result.append(p)
    return sorted(result)


def extract_valid_tasks(root: pathlib.Path) -> set[str]:
    todo = root / "TODO.md"
    if not todo.exists():
        return set()
    return set(TASK_CODE_RE.findall(todo.read_text(encoding="utf-8")))


def check_file(path: pathlib.Path, root: pathlib.Path, valid_tasks: set[str], todo_present: bool) -> list[str]:
    rel = path.relative_to(root).as_posix()
    text = path.read_text(encoding="utf-8")
    violations: list[str] = []
    in_map = is_in_map(rel)
    frozen = rel.startswith("docs/archive/")

    if in_map and not frozen:
        head = "\n".join(text.splitlines()[:12])
        if not LEVEL_RE.search(head):
            violations.append("missing `层级：Lx` header")

    if path.name != "TODO.md" and not frozen:
        for match in TASK_CODE_RE.finditer(text):
            code = match.group(1)
            if code in IGNORED_CODES:
                continue
            if not todo_present:
                continue
            if code not in valid_tasks:
                violations.append(f"invalid task code '{code}'")

    if frozen:
        return violations

    source_level = doc_level(rel)
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
        if not target.exists():
            violations.append(f"broken relative link '{link}'")
            continue
        if source_level >= 0 and target_rel.endswith(".md"):
            target_level = doc_level(target_rel)
            if target_level >= 0 and target_level < source_level:
                violations.append(f"upward link to L{target_level} '{link}' (source L{source_level})")
    return violations


def self_test() -> int:
    assert TASK_CODE_RE.findall("This is a CORE-001 task") == ["CORE-001"]
    assert TASK_CODE_RE.findall("Invalid ABC-12 task") == []
    assert TASK_CODE_RE.findall("Valid CORE-1000 task") == ["CORE-1000"]
    assert TASK_CODE_RE.findall("Compound CORE-040-01 stays whole") == []
    assert TASK_CODE_RE.findall("DUAL-01-01 is not a bare code") == []
    assert LINK_RE.findall("[a](x.md) [b](http://e.com)") == ["x.md", "http://e.com"]
    assert doc_level("AGENTS.md") == 0
    assert doc_level("docs/README.md") == 1
    assert doc_level("docs/ARCHITECTURE.md") == 2
    assert doc_level("docs/bevy-ui/README.md") == 2
    assert doc_level("docs/bevy-ui/BEVY_UI_FRONTEND.md") == 2
    assert doc_level("docs/bevy-ui/BEVY_CORE_MATURITY_GAPS.md") == 3
    assert doc_level("docs/archive/X.md") == 4
    assert doc_level("README.md") == -1
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
    todo_present = (root / "TODO.md").exists()
    valid_tasks = extract_valid_tasks(root)

    violations: dict[str, list[str]] = {}
    for md in markdown_files(root):
        found = check_file(md, root, valid_tasks, todo_present)
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
