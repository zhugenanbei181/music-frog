#!/usr/bin/env python3
"""Prevent expensive CI workflows from running for non-tag changes."""

from __future__ import annotations

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
EXPECTED_EVENTS = {
    "android-device.yml": {"workflow_dispatch"},
    "android.yml": {"workflow_dispatch"},
    "bevy.yml": {"workflow_dispatch"},
    "parity.yml": {"workflow_dispatch"},
    "release.yml": {"push"},
    "test.yml": {"workflow_call"},
    "main-gate.yml": {"push"},
}
TAG_JOB_CONDITION = "    if: github.ref_type == 'tag' && startsWith(github.ref_name, 'v')"


def top_level_events(text: str) -> set[str]:
    lines = text.splitlines()
    try:
        on_index = next(index for index, line in enumerate(lines) if line == "on:")
    except StopIteration:
        return set()

    events: set[str] = set()
    for line in lines[on_index + 1 :]:
        if line and not line[0].isspace() and not line.lstrip().startswith("#"):
            break
        match = re.match(r"^  ([A-Za-z][A-Za-z0-9_-]*):\s*(?:#.*)?$", line)
        if match:
            events.add(match.group(1))
    return events


def job_blocks(text: str) -> dict[str, list[str]]:
    """Return top-level job blocks without depending on a YAML package."""
    lines = text.splitlines()
    try:
        jobs_index = next(index for index, line in enumerate(lines) if line == "jobs:")
    except StopIteration:
        return {}

    jobs: dict[str, list[str]] = {}
    active_job: str | None = None
    for line in lines[jobs_index + 1 :]:
        if line and not line[0].isspace() and not line.lstrip().startswith("#"):
            break
        match = re.match(r"^  ([A-Za-z][A-Za-z0-9_-]*):\s*(?:#.*)?$", line)
        if match:
            active_job = match.group(1)
            jobs[active_job] = []
        elif active_job is not None:
            jobs[active_job].append(line)
    return jobs


def source_problems(root: Path = ROOT) -> list[str]:
    workflow_dir = root / ".github/workflows"
    if not workflow_dir.is_dir():
        return [f"GitHub workflow directory is missing: {workflow_dir}"]

    actual_files = {path.name for path in workflow_dir.glob("*.y*ml")}
    problems: list[str] = []
    for missing in sorted(set(EXPECTED_EVENTS) - actual_files):
        problems.append(f"registered workflow is missing: .github/workflows/{missing}")
    for unregistered in sorted(actual_files - set(EXPECTED_EVENTS)):
        problems.append(f"workflow trigger policy is not registered: .github/workflows/{unregistered}")

    for filename, expected in EXPECTED_EVENTS.items():
        path = workflow_dir / filename
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        events = top_level_events(text)
        if events != expected:
            problems.append(
                f".github/workflows/{filename} events are {sorted(events)}, expected {sorted(expected)}"
            )
        if filename == "main-gate.yml":
            if not re.search(r"(?m)^  push:\s*\n    branches:\s*\[main\]", text):
                problems.append(
                    ".github/workflows/main-gate.yml must trigger on push to main only"
                )
            if re.search(r"cargo\s+(?:build|check|clippy|nextest|test|run|zigbuild)\b", text):
                problems.append(
                    ".github/workflows/main-gate.yml must not compile or run tests"
                )
            continue
        if filename == "release.yml":
            if not re.search(r'(?ms)^  push:\s*\n(?P<body>(?:^    .*\n|^      .*\n|^        .*\n)*)', text):
                problems.append(".github/workflows/release.yml must declare a tag-only push trigger")
            else:
                push_body = re.search(
                    r'(?ms)^  push:\s*\n(?P<body>(?:^    .*\n|^      .*\n|^        .*\n)*)', text
                ).group("body")
                if "tags:" not in push_body or '"v*"' not in push_body or "branches:" in push_body:
                    problems.append(".github/workflows/release.yml push trigger must match v* tags only")
            jobs = job_blocks(text)
            preflight = jobs.get("validate-release-ref", [])
            test_gate = jobs.get("test-gate", [])
            if not preflight or not any(
                "android-product-identity-guard.py --release-tag \"$GITHUB_REF_NAME\"" in line
                for line in preflight
            ):
                problems.append(
                    ".github/workflows/release.yml must validate the exact product version tag before tests"
                )
            if not any("needs: [validate-release-ref]" in line for line in test_gate):
                problems.append(
                    ".github/workflows/release.yml test-gate must wait for release tag validation"
                )
            for build_job in ("build-desktop", "build-android"):
                if not any("needs: [test-gate]" in line for line in jobs.get(build_job, [])):
                    problems.append(
                        f".github/workflows/release.yml {build_job} must wait for the validated test gate"
                    )
        else:
            jobs = job_blocks(text)
            if not jobs:
                problems.append(f".github/workflows/{filename} must declare at least one job")
            for job_name, lines in jobs.items():
                if TAG_JOB_CONDITION not in lines:
                    problems.append(
                        f".github/workflows/{filename} job '{job_name}' must skip non-v* refs "
                        "before assigning a runner"
                    )

    test_callers = [
        path.name
        for path in workflow_dir.glob("*.y*ml")
        if "uses: ./.github/workflows/test.yml" in path.read_text(encoding="utf-8")
    ]
    if test_callers != ["release.yml"]:
        problems.append(
            f"test workflow must only be called by release.yml; found {sorted(test_callers)}"
        )
    return problems


def main() -> int:
    problems = source_problems()
    if problems:
        print("CI trigger guard: FAILED", file=sys.stderr)
        for problem in problems:
            print(f"- {problem}", file=sys.stderr)
        return 1
    print("CI trigger guard: expensive workflows are restricted to v* tags")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
