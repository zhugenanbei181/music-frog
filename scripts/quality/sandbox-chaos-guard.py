#!/usr/bin/env python3
"""Headless sandbox and chaos suite guard (PUB-006.7).

The headless sandbox (`sandbox/`) is the integration surface of the neutral
runtime: it runs the real runtime over adapters that inject deterministic
faults. This guard proves the properties the chaos matrix promises, without
running a single test:

1. the sandbox is a workspace package that never reaches crates.io;
2. the fault model covers the four required fault classes: a network flap,
   the sudden death of the kernel process, the contention of the write fence
   and a corrupted configuration;
3. the injection is deterministic: the suite uses no random number generator
   and derives no outcome from a wall-clock measurement;
4. every fault class owns a scenario in the matrix, so the matrix can never
   grow a fault without a scenario;
5. the sandbox depends on the neutral crates only: no product crate, no UI
   toolkit and no engine binary.

Modes
-----
structure (default, offline)
    The five audits above. No cargo, no network.

run
    Runs the chaos matrix with `cargo nextest run -p lurker-sandbox`. This is
    the entry point the closing batch and the CI job call.

Usage
-----
    python3 scripts/quality/sandbox-chaos-guard.py --mode structure
    python3 scripts/quality/sandbox-chaos-guard.py --mode run
    python3 scripts/quality/sandbox-chaos-guard.py --self-test
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tomllib
from pathlib import Path

from rust_syntax import mask_comments

ROOT = Path(__file__).resolve().parents[2]

# The sandbox package and its test entry point.
PACKAGE = Path("sandbox")
MANIFEST = PACKAGE / "Cargo.toml"
MATRIX = PACKAGE / "tests" / "chaos_matrix.rs"
FAULT_MODEL = PACKAGE / "src" / "fault.rs"

# The fault classes the chaos matrix must cover.
REQUIRED_FAULTS = (
    "NetworkFlap",
    "KernelProcessDeath",
    "LockContention",
    "CorruptedConfiguration",
)

# The neutral crates the sandbox may depend on.
NEUTRAL_CRATES = (
    "lurker-core-contract",
    "lurker-core-ports",
    "lurker-engine-runtime",
    "lurker-profile-store",
    "lurker-ui-presentation",
    "mihomo-driver",
    "singbox-driver",
)

# The non-neutral crates the sandbox needs: the async runtime, the trait
# macro of the neutral ports and the temporary directory of the tests.
ALLOWED_EXTRAS = ("async-trait", "tokio", "tempfile", "futures-executor")

# The product crates and the UI toolkits the sandbox must never depend on.
FORBIDDEN = (
    "infiltrator-",
    "iced",
    "bevy",
    "slint",
    "zhuge-ui",
    "mihomo-api",
    "singbox-api",
)

# Sources of non-determinism. A chaos suite that depends on a random number
# or on a wall-clock measurement cannot prove a typed recovery.
NONDETERMINISM = (
    r"\brand\b",
    r"\brandom\b",
    r"thread_rng",
    r"StdRng",
    r"SmallRng",
    r"tokio::time::sleep",
    r"\bsleep\(",
    r"Instant::now",
    r"SystemTime::now",
    r"elapsed\(\)",
    r"tokio::time::timeout",
)


def _read_manifest(root: Path) -> dict:
    return tomllib.loads((root / MANIFEST).read_text(encoding="utf-8"))


def manifest_problems(root: Path) -> list[str]:
    """The sandbox must be a neutral, publishable-free workspace package."""
    problems: list[str] = []
    manifest = root / MANIFEST
    if not manifest.is_file():
        return [f"{MANIFEST}: the headless sandbox package is missing"]
    document = _read_manifest(root)
    meta = document.get("package", {})
    if meta.get("name") != "lurker-sandbox":
        problems.append(
            f"{MANIFEST}: the package is named {meta.get('name')!r}, expected 'lurker-sandbox'"
        )
    if meta.get("publish") is not False:
        problems.append(f"{MANIFEST}: the sandbox must set `publish = false`")
    members = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    workspace_members = members.get("workspace", {}).get("members", [])
    if PACKAGE.as_posix() not in workspace_members:
        problems.append(f"Cargo.toml: the workspace members miss `{PACKAGE.as_posix()}`")
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        for name in document.get(section, {}):
            if name in NEUTRAL_CRATES or name in ALLOWED_EXTRAS:
                continue
            if any(name.startswith(prefix) for prefix in FORBIDDEN):
                problems.append(
                    f"{MANIFEST}: the sandbox must not depend on `{name}`"
                )
                continue
            problems.append(
                f"{MANIFEST}: the dependency `{name}` is not part of the sandbox allowlist"
            )
    return problems


def fault_model_problems(root: Path) -> list[str]:
    """The fault model must cover the four required fault classes."""
    problems: list[str] = []
    model = root / FAULT_MODEL
    if not model.is_file():
        return [f"{FAULT_MODEL}: the fault model of the sandbox is missing"]
    text = model.read_text(encoding="utf-8")
    for fault in REQUIRED_FAULTS:
        if not re.search(rf"^\s*{fault},\s*$", text, re.MULTILINE):
            problems.append(f"{FAULT_MODEL}: the fault `{fault}` is missing")
    return problems


def matrix_problems(root: Path) -> list[str]:
    """Every fault class must own a scenario in the matrix."""
    problems: list[str] = []
    matrix = root / MATRIX
    if not matrix.is_file():
        return [f"{MATRIX}: the chaos matrix of the sandbox is missing"]
    text = matrix.read_text(encoding="utf-8")
    for fault in REQUIRED_FAULTS:
        if f"Fault::{fault}" not in text:
            problems.append(f"{MATRIX}: the fault `{fault}` has no scenario")
    if "fn scenario_of(" not in text:
        problems.append(
            f"{MATRIX}: the matrix must map every fault to its scenario, so a new "
            "fault cannot appear without one"
        )
    return problems


def determinism_problems(root: Path) -> list[str]:
    """The injection must be deterministic."""
    problems: list[str] = []
    sources = sorted((root / PACKAGE).rglob("*.rs"))
    if not sources:
        return [f"{PACKAGE}: the sandbox has no Rust sources"]
    for source in sources:
        # Only the code is scanned: a doc comment may name the forbidden
        # constructs while explaining why the suite does not use them.
        text = mask_comments(source.read_text(encoding="utf-8", errors="replace"))
        for pattern in NONDETERMINISM:
            match = re.search(pattern, text)
            if match:
                problems.append(
                    f"{source.relative_to(root).as_posix()}: the chaos suite must not "
                    f"depend on `{match.group(0)}`"
                )
    return problems


def structure_problems(root: Path = ROOT) -> list[str]:
    return (
        manifest_problems(root)
        + fault_model_problems(root)
        + matrix_problems(root)
        + determinism_problems(root)
    )


def run_matrix(root: Path, timeout: int = 1800) -> list[str]:
    """Run the chaos matrix with nextest."""
    command = ["cargo", "nextest", "run", "--locked", "-p", "lurker-sandbox"]
    try:
        completed = subprocess.run(
            command,
            cwd=root,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except FileNotFoundError:
        return ["cargo nextest is not available in this environment"]
    except subprocess.TimeoutExpired:
        return [f"{' '.join(command)} timed out after {timeout}s"]
    if completed.returncode != 0:
        problems = [f"{' '.join(command)} failed with exit code {completed.returncode}"]
        for line in (completed.stdout or "").strip().splitlines()[-30:]:
            problems.append(f"  {line}")
        return problems
    return []


def self_test() -> int:
    assert len(REQUIRED_FAULTS) == 4, len(REQUIRED_FAULTS)
    assert r"\brand\b" in NONDETERMINISM
    assert "Instant::now" in NONDETERMINISM
    assert "tokio::time::sleep" in NONDETERMINISM
    assert "tokio::time::timeout" in NONDETERMINISM
    print("sandbox chaos guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mode",
        choices=("structure", "run"),
        default="structure",
        help="structure: offline audit; run: execute the chaos matrix",
    )
    parser.add_argument("--root", default=str(ROOT))
    parser.add_argument(
        "--timeout", type=int, default=1800, help="seconds the matrix may take"
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    root = Path(args.root).resolve()
    problems = structure_problems(root)
    if problems:
        for problem in problems:
            print(f"sandbox chaos guard: {problem}", file=sys.stderr)
        print(f"sandbox chaos guard: FAILED ({len(problems)} problem(s))", file=sys.stderr)
        return 1
    if args.mode == "structure":
        print(
            "sandbox chaos guard: OK (four deterministic fault classes, one scenario each)"
        )
        return 0
    problems = run_matrix(root, args.timeout)
    if problems:
        for problem in problems:
            print(f"sandbox chaos guard: {problem}", file=sys.stderr)
        print(f"sandbox chaos guard: FAILED ({len(problems)} problem(s))", file=sys.stderr)
        return 1
    print("sandbox chaos guard: OK (the chaos matrix passed)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
