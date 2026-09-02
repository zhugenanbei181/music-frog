#!/usr/bin/env python3
"""Quality & Doc Link Guard Script for MusicFrog.

Scans all markdown files in the workspace and reports:
1. Broken relative links.
2. Unindexed task codes (verifies they exist in TODO.md).
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import urllib.parse

# Task code regex: 2-10 uppercase letters, hyphen, 3-4 digits.
TASK_CODE_RE = re.compile(r'\b([A-Z]{2,10}-\d{3,4})\b')
# Markdown link regex: [label](path)
LINK_RE = re.compile(r'\[[^\]]*\]\(([^)]+)\)')

def get_all_md_files(repo_root: pathlib.Path) -> list[pathlib.Path]:
    # Vendored dependency trees and hidden/temporary directories are ignored.
    return sorted(
        p
        for p in repo_root.rglob("*.md")
        if "target" not in p.parts
        and "node_modules" not in p.parts
        and not any(part.startswith(".") for part in p.relative_to(repo_root).parts[:-1])
    )

def extract_valid_tasks(repo_root: pathlib.Path) -> set[str]:
    todo_path = repo_root / "TODO.md"
    if not todo_path.exists():
        return set()
    content = todo_path.read_text(encoding="utf-8")
    # We define valid tasks as any task code present in TODO.md
    return set(TASK_CODE_RE.findall(content))

IGNORED_CODES = {"SHA-256", "AES-256", "SHA-512", "UTF-8", "UTF-16", "ISO-8859"}

def check_file(path: pathlib.Path, repo_root: pathlib.Path, valid_tasks: set[str]) -> list[str]:
    content = path.read_text(encoding="utf-8")
    violations = []

    # Check task codes
    # We skip TODO.md for task code checks because it defines the tasks
    if path.name != "TODO.md":
        for match in TASK_CODE_RE.finditer(content):
            code = match.group(1)
            if code not in valid_tasks and code not in IGNORED_CODES:
                violations.append(f"Invalid task code '{code}'")

    # Check links
    for match in LINK_RE.finditer(content):
        link = match.group(1).strip()
        # Ignore external links, mailto, absolute paths, and empty or pure anchor links
        if (
            link.startswith(("http://", "https://", "mailto:", "/"))
            or not link
            or link.startswith("#")
        ):
            continue
        
        # Remove anchor for file existence check
        link_path = link.split("#", 1)[0]
        if not link_path:
            continue
            
        link_path = urllib.parse.unquote(link_path)
        target = (path.parent / link_path).resolve()
        
        # We also need to be careful with paths traversing out of repo if we want,
        # but just checking exists() is enough for broken link detection.
        if not target.exists():
            violations.append(f"Broken relative link '{link}' (resolved to {target})")

    return violations

def self_test() -> int:
    """Internal unit tests validating the parser, link extraction, and task code extraction."""
    # Test task code extraction
    assert TASK_CODE_RE.findall("This is a CORE-001 task") == ["CORE-001"]
    assert TASK_CODE_RE.findall("Invalid ABC-12 task") == [] # only 2 digits
    assert TASK_CODE_RE.findall("Valid CORE-1000 task") == ["CORE-1000"] # 4 digits
    
    # Test link extraction
    links = LINK_RE.findall("[label](relative/path.md) and [another](http://google.com)")
    assert links == ["relative/path.md", "http://google.com"]
    
    print("self-test OK")
    return 0

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    repo_root = pathlib.Path(__file__).resolve().parents[2]
    valid_tasks = extract_valid_tasks(repo_root)
    md_files = get_all_md_files(repo_root)
    
    all_violations = {}
    for md_file in md_files:
        violations = check_file(md_file, repo_root, valid_tasks)
        if violations:
            all_violations[md_file] = violations

    if not all_violations:
        print("doc-link-guard: OK")
        return 0

    print("doc-link-guard: Found violations:")
    for path, violations in all_violations.items():
        rel_path = path.relative_to(repo_root)
        print(f"  {rel_path}:")
        for v in violations:
            print(f"    - {v}")

    if args.mode == "enforce":
        return 1
    return 0

if __name__ == "__main__":
    sys.exit(main())
