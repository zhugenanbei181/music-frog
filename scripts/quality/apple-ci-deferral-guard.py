#!/usr/bin/env python3
"""Keep Apple-target builds out of automated workflows while deferred."""

from __future__ import annotations

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
APPLE_TARGET = re.compile(
    r"\b(?:aarch64|arm64|x86_64|x86_64h)-apple-(?:darwin|ios(?:-sim)?)\b",
    re.IGNORECASE,
)
APPLE_RUNNER = re.compile(
    r"^\s*(?:runs-on|os)\s*:\s*['\"]?macos-[a-z0-9._-]+",
    re.IGNORECASE,
)


def source_problems(root: Path = ROOT) -> list[str]:
    workflows = root / ".github/workflows"
    if not workflows.is_dir():
        return [f"GitHub workflow directory is missing: {workflows}"]

    problems: list[str] = []
    workflow_files = sorted((*workflows.glob("*.yml"), *workflows.glob("*.yaml")))
    for path in workflow_files:
        for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if APPLE_TARGET.search(line):
                problems.append(
                    f"{path.relative_to(root)}:{line_number} schedules an Apple target while Apple CI is deferred"
                )
            if APPLE_RUNNER.search(line):
                problems.append(
                    f"{path.relative_to(root)}:{line_number} schedules a macOS runner while Apple CI is deferred"
                )
    return problems


def main() -> int:
    problems = source_problems()
    if problems:
        print("Apple CI deferral guard: FAILED", file=sys.stderr)
        for problem in problems:
            print(f"- {problem}", file=sys.stderr)
        return 1
    print("Apple CI deferral guard: no macOS runner or Apple target is scheduled")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
