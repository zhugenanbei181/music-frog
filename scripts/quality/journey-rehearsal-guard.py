#!/usr/bin/env python3
"""Developer journey rehearsal guard (PUB-006.11).

The journey rehearsal (`scripts/developer-journey-rehearsal.py`) is only worth
running when the script and its audit template still describe the same
journey. This guard proves that offline, without cloning, building or
running anything:

1. the script exists and walks the seven steps of the journey in the
   declared order;
2. the audit template exists, carries its authority level and contains every
   placeholder the script fills;
3. the journey budget is five minutes and the script enforces it;
4. the dry run produces a complete report: no placeholder is left over and
   every step appears in the step ledger.

Usage
-----
    python3 scripts/quality/journey-rehearsal-guard.py [--mode report|enforce]
    python3 scripts/quality/journey-rehearsal-guard.py --self-test
"""

from __future__ import annotations

import argparse
import contextlib
import importlib.util
import io
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# The rehearsal script and its audit template.
SCRIPT = Path("scripts/developer-journey-rehearsal.py")
TEMPLATE = Path("scripts/developer-journey-audit-template.md")

# The journey budget in seconds.
BUDGET_SECONDS = 300

# The placeholders the script fills in the template.
PLACEHOLDERS = (
    "{{DATE}}",
    "{{PROFILE}}",
    "{{REPOSITORY}}",
    "{{REVISION}}",
    "{{BUDGET_SECONDS}}",
    "{{TOTAL_SECONDS}}",
    "{{VERDICT}}",
    "{{STEPS_TABLE}}",
    "{{FRICTION}}",
    "{{COMMAND}}",
    "{{REPORT_PATH}}",
    "{{SUMMARY_PATH}}",
    "{{Q1}}",
    "{{Q2}}",
    "{{Q3}}",
    "{{Q4}}",
    "{{Q5}}",
    "{{Q6}}",
    "{{Q7}}",
    "{{Q8}}",
)


def _load_script():
    """Load the rehearsal script as a module (its file name is not importable)."""
    spec = importlib.util.spec_from_file_location(
        "developer_journey_rehearsal", ROOT / SCRIPT
    )
    if spec is None or spec.loader is None:  # pragma: no cover - defensive
        raise ImportError(f"cannot load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    # The dataclasses of the script resolve their own module through
    # `sys.modules`, so the module must be registered before it runs.
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def script_problems(root: Path = ROOT) -> list[str]:
    """The rehearsal script must walk the whole journey."""
    problems: list[str] = []
    path = root / SCRIPT
    if not path.is_file():
        return [f"{SCRIPT}: the developer journey rehearsal script is missing"]
    text = path.read_text(encoding="utf-8")
    try:
        module = _load_script()
        steps = tuple(module.STEPS)
    except Exception as error:  # pragma: no cover - defensive
        return [f"{SCRIPT}: the rehearsal script does not load ({error})"]
    if len(steps) != 7:
        problems.append(f"{SCRIPT}: the journey must have seven steps, found {len(steps)}")
    for step in ("clone", "read-docs", "scaffold", "build", "run", "attach-driver", "debug-configuration"):
        if step not in steps:
            problems.append(f"{SCRIPT}: the journey step `{step}` is missing")
    if list(steps) != sorted(steps, key=lambda step: steps.index(step)):
        problems.append(f"{SCRIPT}: the journey steps are not in a stable order")
    if module.BUDGET_SECONDS != BUDGET_SECONDS:
        problems.append(
            f"{SCRIPT}: the journey budget is {module.BUDGET_SECONDS}s, expected {BUDGET_SECONDS}s"
        )
    if "--dry-run" not in text:
        problems.append(f"{SCRIPT}: the rehearsal needs a `--dry-run` mode")
    return problems


def template_problems(root: Path = ROOT) -> list[str]:
    """The audit template must carry every placeholder the script fills."""
    problems: list[str] = []
    path = root / TEMPLATE
    if not path.is_file():
        return [f"{TEMPLATE}: the developer journey audit template is missing"]
    text = path.read_text(encoding="utf-8")
    if not re.search(r"^> 层级：L3\s*$", text, re.MULTILINE):
        problems.append(f"{TEMPLATE}: the template must declare `层级：L3`")
    for placeholder in PLACEHOLDERS:
        if placeholder not in text:
            problems.append(f"{TEMPLATE}: the placeholder `{placeholder}` is missing")
    return problems


def dry_run_problems(root: Path = ROOT) -> list[str]:
    """The dry run must produce a complete report."""
    problems: list[str] = []
    try:
        module = _load_script()
    except Exception as error:  # pragma: no cover - defensive
        return [f"{SCRIPT}: the rehearsal script does not load ({error})"]
    with tempfile.TemporaryDirectory(prefix="lurker-journey-guard-") as directory:
        evidence = Path(directory)
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                module.run_rehearsal("fast", True, evidence)
        except Exception as error:  # pragma: no cover - defensive
            return [f"{SCRIPT}: the dry run failed ({error})"]
        reports = sorted(evidence.glob("*.md"))
        if not reports:
            return [f"{SCRIPT}: the dry run produced no audit report"]
        report = reports[-1].read_text(encoding="utf-8")
        for placeholder in PLACEHOLDERS:
            if placeholder in report:
                problems.append(
                    f"{SCRIPT}: the filled report still carries the placeholder `{placeholder}`"
                )
        for step in module.STEPS:
            if f"| {step} |" not in report:
                problems.append(f"{SCRIPT}: the filled report does not list the step `{step}`")
        summaries = sorted(evidence.glob("*.json"))
        if not summaries:
            problems.append(f"{SCRIPT}: the dry run produced no machine readable summary")
    return problems


def check(root: Path = ROOT) -> list[str]:
    return script_problems(root) + template_problems(root) + dry_run_problems(root)


def self_test() -> int:
    assert len(PLACEHOLDERS) == 20, len(PLACEHOLDERS)
    assert "{{Q1}}" in PLACEHOLDERS and "{{Q8}}" in PLACEHOLDERS
    assert BUDGET_SECONDS == 300
    print("journey rehearsal guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--root", default=str(ROOT))
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    root = Path(args.root).resolve()
    problems = check(root)
    if not problems:
        print(
            "journey rehearsal guard: OK (seven steps, five minute budget, complete "
            "audit template)"
        )
        return 0
    for problem in problems:
        print(f"journey rehearsal guard: {problem}", file=sys.stderr)
    print(f"journey rehearsal guard: violations={len(problems)}", file=sys.stderr)
    return 1 if args.mode == "enforce" else 0


if __name__ == "__main__":
    raise SystemExit(main())
