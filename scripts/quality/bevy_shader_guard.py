#!/usr/bin/env python3
"""Structural WESL rules; linking, pixels and asset release need real execution."""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import re

from rust_syntax import mask_comments, mask_noncode, mask_test_items

ROOTS = ("crates/infiltrator-bevy-widgets/src", "crates/infiltrator-bevy-ui/src")


@dataclass(frozen=True)
class Violation:
    path: str
    line: int
    code: str
    message: str


def scan(root: Path) -> list[Violation]:
    violations: list[Violation] = []

    def report(path: Path, text: str, offset: int, code: str, message: str):
        violations.append(Violation(str(path.relative_to(root)), text.count("\n", 0, offset) + 1, code, message))

    for relative in ROOTS:
        source = root / relative
        if not source.exists():
            continue
        registered: set[str] = set()
        references: list[tuple[Path, str, re.Match]] = []
        modules: dict[Path, list[Path]] = {}
        for path in sorted(source.rglob("*")):
            if path.suffix == ".wgsl":
                report(path, "", 0, "BEVY-SHADER-001", "Project shaders must use modular WESL")
            elif path.suffix == ".wesl":
                text = mask_comments(path.read_text())
                dependencies = []
                for match in re.finditer(r"\bimport\s+package::([^;]+);", text):
                    imported = match.group(1).split("::{", 1)
                    parts = imported[0].strip().split("::")
                    if len(imported) == 1:
                        parts = parts[:-1]  # final element is the imported symbol
                    dependency = source.joinpath(*parts).with_suffix(".wesl")
                    if not dependency.is_file():
                        report(path, text, match.start(), "BEVY-SHADER-004", "Missing package WESL dependency")
                    else:
                        dependencies.append(dependency)
                modules[path] = dependencies
            elif path.suffix == ".rs" and not re.search(r"_tests?\.rs$", path.name):
                text = mask_comments(mask_test_items(path.read_text()))
                syntax = mask_noncode(text)
                for match in re.finditer(r"\bfrom_wgsl\s*\(", syntax):
                    report(path, text, match.start(), "BEVY-SHADER-001", "Construct project shaders through the WESL path")
                for match in re.finditer(r"\bforget\s*\(|\bBox\s*::\s*leak\s*\(", syntax):
                    report(path, text, match.start(), "BEVY-SHADER-003", "Explicit leaking bypasses App/entity asset ownership")
                for match in re.finditer(r'r(?P<hashes>#*)"(?P<body>.*?)"(?P=hashes)', text, re.S):
                    if re.search(r"@(vertex|fragment|group|binding)\b", match.group("body")):
                        report(path, text, match.start(), "BEVY-SHADER-002", "Move inline shader source to WESL modules")
                for match in re.finditer(r'embedded_asset!\s*\([^,]+,\s*"([^"]+\.wesl)"', text):
                    asset = path.parent / match.group(1)
                    if not asset.is_file():
                        report(path, text, match.start(), "BEVY-SHADER-005", "Embedded WESL file does not exist")
                    else:
                        registered.add(asset.relative_to(source).as_posix())
                for match in re.finditer(r'"embedded://[^/"]+/([^"]+\.wesl)"', text):
                    references.append((path, text, match))
                for match in re.finditer(r'"[^"\n]*\.wgsl"', text):
                    report(path, text, match.start(), "BEVY-SHADER-001", "Project shader paths must use WESL")
        for path, text, match in references:
            if match.group(1) not in registered:
                report(path, text, match.start(), "BEVY-SHADER-005", "Embedded WESL reference has no registration")
        visiting: set[Path] = set()
        done: set[Path] = set()

        def visit(path: Path):
            if path in visiting:
                report(path, "", 0, "BEVY-SHADER-004", "Cyclic WESL package imports")
                return
            if path in done:
                return
            visiting.add(path)
            for dependency in modules.get(path, []):
                visit(dependency)
            visiting.remove(path)
            done.add(path)

        for path in modules:
            visit(path)
    return violations


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    arguments = parser.parse_args()
    violations = scan(arguments.root)
    for violation in violations:
        print(f"{violation.path}:{violation.line}: {violation.code}: {violation.message}")
    print(f"Bevy WESL structure: {len(violations)} violations")
    return int(bool(violations) and arguments.mode == "enforce")


if __name__ == "__main__":
    raise SystemExit(main())
