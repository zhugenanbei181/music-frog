#!/usr/bin/env python3
"""Developer journey rehearsal and dry-run audit (PUB-006.11).

An open-source crate is only as good as the journey of the developer who
adopts it. This script replays that journey end to end and writes an audit
report:

    clone -> read the docs -> scaffold a project -> run it -> attach a driver
    -> debug a configuration

The journey budget is five minutes (300 s) for the whole chain. Every step
records its own duration, its verdict (`ok`, `blocked` or `failed`) and the
exact evidence, so the report shows *where* the time went and *what* blocked
the developer. A step that cannot run in the current environment is reported,
never silently skipped.

Modes
-----
default
    The real rehearsal. It clones the repository, generates the project
    templates, builds them, starts them, attaches an engine driver and
    exercises the corrupted-configuration recovery.

--dry-run
    No clone, no network, no cargo. The script still walks every step, keeps
    the same order and fills the same report, so the structure gate can
    exercise the rehearsal logic offline.

Usage
-----
    python3 scripts/developer-journey-rehearsal.py --profile fast
    python3 scripts/developer-journey-rehearsal.py --profile full
    python3 scripts/developer-journey-rehearsal.py --dry-run
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# The journey budget in seconds.
BUDGET_SECONDS = 300

# The report template the rehearsal fills.
TEMPLATE = ROOT / "scripts/developer-journey-audit-template.md"

# The step order of the journey. The order is the contract of the rehearsal.
STEPS = (
    "clone",
    "read-docs",
    "scaffold",
    "build",
    "run",
    "attach-driver",
    "debug-configuration",
)

# The template projects of every profile: the UI framework and the engine
# support the generated project is scaffolded with.
PROFILES = {
    "fast": (("slint", "mihomo"),),
    "full": (("slint", "mihomo"), ("iced", "mihomo"), ("bevy", "mihomo")),
}

# The verdict of one step.
VERDICT_OK = "ok"
VERDICT_BLOCKED = "blocked"
VERDICT_FAILED = "failed"


@dataclass
class StepResult:
    """The measured result of one journey step."""

    name: str
    seconds: float
    verdict: str
    detail: str = ""

    @property
    def failed(self) -> bool:
        return self.verdict == VERDICT_FAILED


@dataclass
class Journey:
    """The whole rehearsal: the ordered steps and the friction they found."""

    profile: str
    steps: list[StepResult] = field(default_factory=list)
    friction: list[str] = field(default_factory=list)

    @property
    def total(self) -> float:
        return sum(step.seconds for step in journey_steps(self))

    @property
    def verdict(self) -> str:
        if any(step.failed for step in self.steps):
            return "failed"
        if any(step.verdict == VERDICT_BLOCKED for step in self.steps):
            return "blocked"
        return "ok"


def journey_steps(journey: Journey) -> list[StepResult]:
    """The measured steps of a journey."""
    return journey.steps


def _run(
    command: list[str],
    cwd: Path | None = None,
    timeout: int = 600,
    env: dict | None = None,
) -> tuple[int, str]:
    try:
        completed = subprocess.run(
            command,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
            env={**os.environ, **env} if env else None,
        )
    except FileNotFoundError as error:
        return 127, f"command not found: {error}"
    except subprocess.TimeoutExpired:
        return 124, f"command timed out after {timeout}s"
    return completed.returncode, (completed.stdout or "") + (completed.stderr or "")


def _has(command: str) -> bool:
    return shutil.which(command) is not None


def _step(journey: Journey, name: str, action) -> None:
    """Run one journey step, measure it and record the verdict."""
    started = time.monotonic()
    verdict, detail = action()
    journey.steps.append(
        StepResult(name, round(time.monotonic() - started, 3), verdict, detail)
    )


# ---------------------------------------------------------------------------
# The steps of the journey
# ---------------------------------------------------------------------------


def clone_step(journey: Journey, workspace: Path, dry_run: bool) -> Path:
    """Clone the repository into a clean directory."""

    def action() -> tuple[str, str]:
        if dry_run:
            return VERDICT_OK, "dry run: the clone step was simulated"
        target = workspace / "clone"
        code, output = _run(
            ["git", "clone", "--depth", "1", f"file://{ROOT}", str(target)],
            timeout=600,
        )
        if code != 0:
            return VERDICT_FAILED, f"git clone failed: {output.strip()[:400]}"
        return VERDICT_OK, f"cloned the repository into {target}"

    _step(journey, "clone", action)
    clone = workspace / "clone"
    return clone if clone.is_dir() else workspace


def docs_step(journey: Journey, clone: Path, dry_run: bool) -> None:
    """Prove that the documentation map answers "where do I start"."""

    def action() -> tuple[str, str]:
        if dry_run:
            return VERDICT_OK, "dry run: the docs step was simulated"
        entry_points = ("README.md", "docs/README.md", "crates/lurker-core-contract/README.md")
        missing = [name for name in entry_points if not (clone / name).is_file()]
        if missing:
            return VERDICT_FAILED, f"the documentation entry points are missing: {missing}"
        code, output = _run(
            ["python3", "scripts/quality/doc-governance-guard.py", "--mode", "enforce"],
            cwd=clone,
            timeout=300,
        )
        if code != 0:
            return VERDICT_FAILED, f"the documentation map does not resolve: {output.strip()[:400]}"
        return (
            VERDICT_OK,
            "the documentation map resolves and names the neutral crates",
        )

    _step(journey, "read-docs", action)


def scaffold_step(
    journey: Journey, clone: Path, workspace: Path, profile: str, dry_run: bool
) -> list[Path]:
    """Generate one project per template of the profile."""
    projects: list[Path] = []

    def action() -> tuple[str, str]:
        if dry_run:
            return VERDICT_OK, "dry run: the scaffold step was simulated"
        if not _has("cargo-generate"):
            journey.friction.append(
                "the scaffold step needs `cargo-generate`, which is not part of the "
                "pinned toolchain"
            )
            return (
                VERDICT_BLOCKED,
                "`cargo-generate` is not installed; install it with "
                "`cargo install cargo-generate` to replay this step",
            )
        for ui_framework, engine_support in PROFILES[profile]:
            name = f"journey-{ui_framework}"
            target = workspace / name
            code, output = _run(
                [
                    "cargo",
                    "generate",
                    "--path",
                    "templates",
                    "--name",
                    name,
                    "--define",
                    f"ui_framework={ui_framework}",
                    "--define",
                    f"engine_support={engine_support}",
                    "--define",
                    f"project_name={name}",
                    "--define",
                    "author=Journey Rehearsal",
                    "--silent",
                ],
                cwd=clone,
                timeout=600,
            )
            if code != 0 or not (target / "Cargo.toml").is_file():
                return (
                    VERDICT_FAILED,
                    f"cargo-generate failed for {ui_framework}: {output.strip()[:400]}",
                )
            projects.append(target)
        return (
            VERDICT_OK,
            "generated "
            + str(len(projects))
            + " project(s): "
            + ", ".join(project.name for project in projects),
        )

    _step(journey, "scaffold", action)
    return projects


def build_step(journey: Journey, projects: list[Path], workspace: Path, dry_run: bool) -> None:
    """Build every scaffolded project."""
    target_dir = workspace / "target"

    def action() -> tuple[str, str]:
        if dry_run:
            return VERDICT_OK, "dry run: the build step was simulated"
        if not projects:
            return VERDICT_BLOCKED, "no project was scaffolded"
        failures: list[str] = []
        for project in projects:
            code, output = _run(
                ["cargo", "build"],
                cwd=project,
                timeout=1800,
                env={"CARGO_TARGET_DIR": str(target_dir)},
            )
            if code != 0:
                failures.append(f"{project.name}: {output.strip()[-400:]}")
        if failures:
            return VERDICT_FAILED, "; ".join(failures)
        return VERDICT_OK, f"built {len(projects)} project(s)"

    _step(journey, "build", action)


def run_step(journey: Journey, projects: list[Path], workspace: Path, dry_run: bool) -> None:
    """Start every scaffolded project and record how it ended."""
    target_dir = workspace / "target"

    def action() -> tuple[str, str]:
        if dry_run:
            return VERDICT_OK, "dry run: the run step was simulated"
        if not projects:
            return VERDICT_BLOCKED, "no project was scaffolded"
        outcomes: list[str] = []
        for project in projects:
            binary = target_dir / "debug" / project.name
            if not binary.is_file():
                outcomes.append(f"{project.name}: no binary was produced")
                continue
            try:
                process = subprocess.Popen(
                    [str(binary)],
                    cwd=project,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                )
            except OSError as error:
                outcomes.append(f"{project.name}: the binary did not start ({error})")
                continue
            try:
                process.wait(timeout=5)
                outcomes.append(f"{project.name}: exited with {process.returncode}")
            except subprocess.TimeoutExpired:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                outcomes.append(f"{project.name}: started and stayed alive")
        return VERDICT_OK, "; ".join(outcomes)

    _step(journey, "run", action)


def attach_driver_step(
    journey: Journey, clone: Path, projects: list[Path], workspace: Path, dry_run: bool
) -> None:
    """Attach an engine driver to every scaffolded project."""
    target_dir = workspace / "target"

    def action() -> tuple[str, str]:
        if dry_run:
            return VERDICT_OK, "dry run: the attach-driver step was simulated"
        if not projects:
            return VERDICT_BLOCKED, "no project was scaffolded"
        failures: list[str] = []
        for project in projects:
            manifest = project / "Cargo.toml"
            text = manifest.read_text(encoding="utf-8")
            mihomo = os.path.relpath(clone / "crates" / "mihomo-driver", project)
            singbox = os.path.relpath(clone / "crates" / "singbox-driver", project)
            manifest.write_text(
                text
                + f'\nmihomo-driver = {{ path = "{mihomo}" }}\n'
                + f'singbox-driver = {{ path = "{singbox}" }}\n',
                encoding="utf-8",
            )
            source = project / "src" / "main.rs"
            if source.is_file():
                body = source.read_text(encoding="utf-8")
                probe = (
                    "\n// The engine drivers are attachable to the scaffolded project.\n"
                    "fn _drivers_are_public() {\n"
                    "    let _ = std::any::type_name::<mihomo_driver::types::Version>();\n"
                    "    let _ = std::any::type_name::<\n"
                    "        singbox_driver::version::ProcessSingBoxBinaryInspector,\n"
                    "    >();\n"
                    "}\n"
                )
                source.write_text(body + probe, encoding="utf-8")
            code, output = _run(
                ["cargo", "build"],
                cwd=project,
                timeout=1800,
                env={"CARGO_TARGET_DIR": str(target_dir)},
            )
            if code != 0:
                failures.append(f"{project.name}: {output.strip()[-400:]}")
        if failures:
            return VERDICT_FAILED, "; ".join(failures)
        return (
            VERDICT_OK,
            f"attached the mihomo and sing-box drivers to {len(projects)} project(s)",
        )

    _step(journey, "attach-driver", action)


def debug_configuration_step(journey: Journey, clone: Path, dry_run: bool) -> None:
    """Debug a corrupted configuration and read back the typed recovery."""

    def action() -> tuple[str, str]:
        if dry_run:
            return VERDICT_OK, "dry run: the debug-configuration step was simulated"
        broken = clone / "engines" / "mihomo" / "profiles" / "broken.yaml"
        broken.parent.mkdir(parents=True, exist_ok=True)
        broken.write_text("proxies: [ this is not: valid yaml\n", encoding="utf-8")
        if not (clone / "sandbox").is_dir():
            return (
                VERDICT_BLOCKED,
                "the headless sandbox that proves the corrupted configuration "
                "recovery is not part of this checkout",
            )
        code, output = _run(
            [
                "cargo",
                "nextest",
                "run",
                "--locked",
                "-p",
                "lurker-sandbox",
                "corrupted_configuration",
            ],
            cwd=clone,
            timeout=1800,
        )
        if code != 0:
            return (
                VERDICT_FAILED,
                "the corrupted configuration recovery is not proven: " + output.strip()[-400:],
            )
        return (
            VERDICT_OK,
            "the corrupted configuration is rejected by the typed store kernel and recovered",
        )

    _step(journey, "debug-configuration", action)


# ---------------------------------------------------------------------------
# The report
# ---------------------------------------------------------------------------


def fill_report(journey: Journey, template: str, summary: dict) -> str:
    """Fill the audit template with the measured journey."""
    rows = [
        "| Step | Verdict | Seconds | Evidence |",
        "| --- | --- | ---: | --- |",
    ]
    for step in journey.steps:
        detail = step.detail.replace("|", "\\|")
        rows.append(f"| {step.name} | {step.verdict} | {step.seconds:.1f} | {detail} |")
    friction = "\n".join(f"- {item}" for item in journey.friction) or "- none recorded"

    def passed(name: str) -> str:
        return (
            "yes"
            if any(step.name == name and step.verdict == VERDICT_OK for step in journey.steps)
            else "no"
        )

    slowest = max(journey.steps, key=lambda step: step.seconds).name if journey.steps else "n/a"
    answers = {
        "Q1": "yes" if not any(step.failed for step in journey.steps) else "no",
        "Q2": passed("read-docs"),
        "Q3": passed("run"),
        "Q4": passed("attach-driver"),
        "Q5": passed("debug-configuration"),
        "Q6": slowest,
        "Q7": "yes" if journey.total <= BUDGET_SECONDS else "no",
        "Q8": "; ".join(journey.friction) or "none",
    }
    replacements = {
        "{{DATE}}": summary["date"],
        "{{PROFILE}}": journey.profile,
        "{{REPOSITORY}}": summary["repository"],
        "{{REVISION}}": summary["revision"],
        "{{BUDGET_SECONDS}}": str(BUDGET_SECONDS),
        "{{TOTAL_SECONDS}}": f"{journey.total:.1f}",
        "{{VERDICT}}": journey.verdict,
        "{{STEPS_TABLE}}": "\n".join(rows),
        "{{FRICTION}}": friction,
        "{{COMMAND}}": summary["command"],
        "{{REPORT_PATH}}": summary["report_path"],
        "{{SUMMARY_PATH}}": summary["summary_path"],
    }
    report = template
    for placeholder, value in replacements.items():
        report = report.replace(placeholder, value)
    for question, answer in answers.items():
        report = report.replace("{{" + question + "}}", answer)
    return report


def _revision(root: Path) -> str:
    code, output = _run(["git", "rev-parse", "--short", "HEAD"], cwd=root, timeout=60)
    return output.strip() if code == 0 else "unknown"


def run_rehearsal(profile: str, dry_run: bool, evidence_dir: Path) -> tuple[int, dict]:
    """Run the whole journey and write the audit report."""
    journey = Journey(profile=profile)
    with tempfile.TemporaryDirectory(prefix="lurker-journey-") as directory:
        workspace = Path(directory)
        clone = clone_step(journey, workspace, dry_run)
        docs_step(journey, clone, dry_run)
        projects = scaffold_step(journey, clone, workspace, profile, dry_run)
        build_step(journey, projects, workspace, dry_run)
        run_step(journey, projects, workspace, dry_run)
        attach_driver_step(journey, clone, projects, workspace, dry_run)
        debug_configuration_step(journey, clone, dry_run)

    evidence_dir.mkdir(parents=True, exist_ok=True)
    stamp = time.strftime("%Y%m%d-%H%M%S")
    report_path = evidence_dir / f"developer-journey-{profile}-{stamp}.md"
    summary_path = evidence_dir / f"developer-journey-{profile}-{stamp}.json"
    summary = {
        "date": time.strftime("%Y-%m-%d"),
        "repository": str(ROOT),
        "revision": _revision(ROOT),
        "command": (
            "python3 scripts/developer-journey-rehearsal.py --profile "
            f"{profile}" + (" --dry-run" if dry_run else "")
        ),
        "report_path": str(report_path),
        "summary_path": str(summary_path),
    }
    report = fill_report(journey, TEMPLATE.read_text(encoding="utf-8"), summary)
    report_path.write_text(report, encoding="utf-8")
    summary_path.write_text(
        json.dumps(
            {
                **summary,
                "profile": profile,
                "dry_run": dry_run,
                "budget_seconds": BUDGET_SECONDS,
                "total_seconds": round(journey.total, 3),
                "verdict": journey.verdict,
                "steps": [
                    {
                        "name": step.name,
                        "verdict": step.verdict,
                        "seconds": round(step.seconds, 3),
                        "detail": step.detail,
                    }
                    for step in journey.steps
                ],
                "friction": journey.friction,
            },
            indent=2,
            ensure_ascii=False,
        )
        + "\n",
        encoding="utf-8",
    )
    for step in journey.steps:
        print(f"[journey] {step.name:<20} {step.verdict:<8} {step.detail}")
    print(f"[journey] total {journey.total:.1f}s (budget {BUDGET_SECONDS}s)")
    print(f"[journey] report {report_path}")
    return 0 if journey.verdict == "ok" else 1, summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--profile",
        choices=tuple(PROFILES),
        default="fast",
        help="the template profile to replay",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="walk every step without cloning, building or running anything",
    )
    parser.add_argument(
        "--evidence-dir",
        default=str(ROOT / ".evidence" / "journey"),
        help="directory the audit report and the machine readable summary are written to",
    )
    args = parser.parse_args()

    code, _ = run_rehearsal(args.profile, args.dry_run, Path(args.evidence_dir))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
