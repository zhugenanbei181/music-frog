#!/usr/bin/env python3
"""Fail-closed guard for UI-04-07 cross-resolution visual regression testing pipeline."""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    for marker in markers:
        if marker not in text:
            violations.append(f"{path} missing {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    # 1. Verify Roadmap and Documentation references
    require(
        violations,
        "docs/DUAL_SURFACE_UI_UX_ROADMAP.md",
        "UI-04-07",
        "跨分辨率像素级自动化截图视觉回归测试流水线",
    )

    # 2. Verify Pipeline script existence and scenario definitions
    pipeline_script = ROOT / "scripts" / "visual-regression-pipeline.py"
    if not pipeline_script.exists():
        violations.append("scripts/visual-regression-pipeline.py missing")

    require(
        violations,
        "scripts/capture_bevy_scenarios.tsv",
        "390x800",
        "1024x768",
        "1180x760",
    )

    # 3. Execute self-test to verify algorithm correctness
    cmd = [sys.executable, str(pipeline_script), "--self-test"]
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        violations.append(f"visual-regression-pipeline self-test failed: {result.stderr.strip()}")

    if violations:
        print("visual-regression-guard: FAIL", file=sys.stderr)
        for v in violations:
            print(f"  - {v}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0

    print("visual-regression-guard: UI-04-07 pipeline verified violations=0")
    return 0


if __name__ == "__main__":
    sys.exit(main())
