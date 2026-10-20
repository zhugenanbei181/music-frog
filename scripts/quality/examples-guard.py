#!/usr/bin/env python3
"""Examples gallery guard: structure, line budget and CI compile gate (PUB-006.9).

The gallery under `examples/` is the public face of the neutral Lurker crates:
four standalone minimal programs (`minimal-cli`, `minimal-slint`,
`minimal-iced`, `minimal-bevy`) that a third party can read, copy and run.

The guard enforces the four properties the gallery promises.

1. Every gallery entry exists, is a publishable-free workspace package and
   declares exactly one example target with the name of its directory.
2. No example carries a dependency it does not need: the allowed set is the
   published neutral crates plus the one UI toolkit the example demonstrates
   (`slint-build` is the Slint build-time companion). The toolkit requirement
   must be byte-identical to the one the matching product template uses, so
   the version and the feature set have exactly one source of truth.
3. Every example source stays inside the 200-line budget of the gallery.
4. The CI compile gate exists and obeys the tag-only trigger policy.

Modes
-----
structure (default, offline)
    The four audits above. No cargo, no network.

compile
    Builds every gallery example with `cargo build --example <name>`. The
    build is the CI compile guard: it proves that each example still compiles
    against the neutral crates and its toolkit.

smoke
    Runs the headless CLI example and checks its output. The three UI
    examples need a display, so only the terminal example is executed.

Usage
-----
    python3 scripts/quality/examples-guard.py --mode structure
    python3 scripts/quality/examples-guard.py --mode compile
    python3 scripts/quality/examples-guard.py --mode smoke
    python3 scripts/quality/examples-guard.py --self-test
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# The gallery entries: directory name -> (workspace package, UI toolkit).
GALLERY = (
    ("minimal-cli", "lurker-example-minimal-cli", None),
    ("minimal-slint", "lurker-example-minimal-slint", "slint"),
    ("minimal-iced", "lurker-example-minimal-iced", "iced"),
    ("minimal-bevy", "lurker-example-minimal-bevy", "bevy"),
)

# The published neutral crates an example may depend on.
NEUTRAL_CRATES = (
    "lurker-core-contract",
    "lurker-core-ports",
    "lurker-engine-runtime",
    "lurker-profile-store",
    "lurker-ui-presentation",
    "mihomo-driver",
    "singbox-driver",
)

# The build-time companion of the Slint toolkit.
BUILD_COMPANIONS = {"slint-build": "slint"}

# The product template that owns the toolkit requirement of each gallery entry.
TEMPLATE_OF = {
    "slint": "templates/slint",
    "iced": "templates/iced",
    "bevy": "templates/bevy",
}

# The line budget of one gallery example source file.
LINE_BUDGET = 200

# The CI compile gate that compiles the gallery.
CI_WORKFLOW = Path(".github/workflows/ecosystem-gate.yml")

# The trigger policy every non-main-gate workflow job must carry.
TAG_JOB_CONDITION = "if: github.ref_type == 'tag' && startsWith(github.ref_name, 'v')"

EXAMPLE_TARGET_RE = re.compile(r"\[\[example\]\](.*?)(?=\n\[|\Z)", re.DOTALL)


def _read_manifest(path: Path) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def _dependency_sections(document: dict) -> dict[str, dict]:
    sections: dict[str, dict] = {}
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        value = document.get(section)
        if isinstance(value, dict):
            sections[section] = value
    return sections


def _normalized_spec(spec) -> str:
    """A stable, comparable rendering of one dependency requirement."""
    if isinstance(spec, str):
        return spec
    if not isinstance(spec, dict):
        return repr(spec)
    parts: list[str] = []
    if "package" in spec:
        parts.append(f"package={spec['package']}")
    if "version" in spec:
        parts.append(f"version={spec['version']}")
    if "workspace" in spec:
        parts.append(f"workspace={spec['workspace']}")
    if "default-features" in spec:
        parts.append(f"default-features={spec['default-features']}")
    features = spec.get("features")
    if features is not None:
        parts.append("features=" + ",".join(sorted(features)))
    return "|".join(parts)


def example_sources(root: Path, directory: str) -> list[Path]:
    """Every source file of one gallery entry."""
    base = root / "examples" / directory
    if not base.is_dir():
        return []
    return sorted(
        path
        for path in base.rglob("*")
        if path.is_file()
        and path.suffix in {".rs", ".slint"}
        and "target" not in path.parts
    )


def manifest_problems(root: Path) -> list[str]:
    """Every gallery entry must be a minimal, publishable-free example package."""
    problems: list[str] = []
    for directory, package, toolkit in GALLERY:
        manifest_path = root / "examples" / directory / "Cargo.toml"
        if not manifest_path.is_file():
            problems.append(f"examples/{directory}: the gallery entry is missing")
            continue
        document = _read_manifest(manifest_path)
        meta = document.get("package", {})
        if meta.get("name") != package:
            problems.append(
                f"examples/{directory}: package name is {meta.get('name')!r}, expected {package!r}"
            )
        if meta.get("publish") is not False:
            problems.append(
                f"examples/{directory}: an example package must set `publish = false`"
            )
        if meta.get("license") != "MIT OR Apache-2.0":
            problems.append(
                f"examples/{directory}: licence must be `MIT OR Apache-2.0`"
            )

        sections = _dependency_sections(document)
        if "dev-dependencies" in sections:
            problems.append(
                f"examples/{directory}: a gallery example must not carry dev-dependencies"
            )
        for section, dependencies in sections.items():
            for name in dependencies:
                if name in NEUTRAL_CRATES:
                    continue
                if name == toolkit:
                    continue
                if section == "build-dependencies" and BUILD_COMPANIONS.get(name) == toolkit:
                    continue
                problems.append(
                    f"examples/{directory}: dependency `{name}` is not part of the "
                    "gallery allowlist (neutral crates plus the demonstrated toolkit)"
                )

        targets = document.get("example", [])
        if not isinstance(targets, list) or len(targets) != 1:
            problems.append(
                f"examples/{directory}: exactly one `[[example]]` target is required"
            )
            continue
        target = targets[0]
        if target.get("name") != directory:
            problems.append(
                f"examples/{directory}: the example target is named "
                f"{target.get('name')!r}, expected {directory!r}"
            )
        path = target.get("path")
        if not isinstance(path, str) or not (root / "examples" / directory / path).is_file():
            problems.append(
                f"examples/{directory}: the example target path {path!r} does not exist"
            )
        if target.get("required-features"):
            problems.append(
                f"examples/{directory}: a gallery example must build without extra "
                "features, so `cargo run --example` works out of the box"
            )

    return problems


def toolkit_problems(root: Path) -> list[str]:
    """The toolkit requirement of an example must equal the template's."""
    problems: list[str] = []
    for directory, _package, toolkit in GALLERY:
        if toolkit is None:
            continue
        template = root / TEMPLATE_OF[toolkit] / "Cargo.toml"
        example = root / "examples" / directory / "Cargo.toml"
        if not template.is_file() or not example.is_file():
            problems.append(
                f"examples/{directory}: the {toolkit} template or the example manifest is missing"
            )
            continue
        template_spec = _read_manifest(template).get("dependencies", {}).get(toolkit)
        example_spec = _read_manifest(example).get("dependencies", {}).get(toolkit)
        if template_spec is None or example_spec is None:
            problems.append(
                f"examples/{directory}: the {toolkit} dependency must be declared in "
                "the example and in the product template"
            )
            continue
        if _normalized_spec(template_spec) != _normalized_spec(example_spec):
            problems.append(
                f"examples/{directory}: the {toolkit} requirement "
                f"({_normalized_spec(example_spec)}) drifted from the product "
                f"template ({_normalized_spec(template_spec)})"
            )
    return problems


def budget_problems(root: Path) -> list[str]:
    """Every gallery source file stays inside the 200-line budget."""
    problems: list[str] = []
    for directory, _package, _toolkit in GALLERY:
        for source in example_sources(root, directory):
            lines = len(source.read_text(encoding="utf-8", errors="replace").splitlines())
            if lines > LINE_BUDGET:
                problems.append(
                    f"{source.relative_to(root).as_posix()}: {lines} lines exceed the "
                    f"{LINE_BUDGET}-line gallery budget"
                )
    return problems


def ci_gate_problems(root: Path) -> list[str]:
    """The CI compile gate must exist and obey the tag-only trigger policy."""
    problems: list[str] = []
    workflow = root / CI_WORKFLOW
    if not workflow.is_file():
        problems.append(
            f"{CI_WORKFLOW.as_posix()}: the CI compile gate of the gallery is missing"
        )
        return problems
    text = workflow.read_text(encoding="utf-8")
    if "scripts/quality/examples-guard.py --mode compile" not in text:
        problems.append(
            f"{CI_WORKFLOW.as_posix()}: the workflow must call the gallery compile guard"
        )
    if TAG_JOB_CONDITION not in text:
        problems.append(
            f"{CI_WORKFLOW.as_posix()}: every job must skip non-v* refs before it "
            "assigns a runner"
        )
    if "cargo build" in text and "--example" not in text:
        problems.append(
            f"{CI_WORKFLOW.as_posix()}: the compile gate must build the example targets"
        )
    return problems


def structure_problems(root: Path = ROOT) -> list[str]:
    return (
        manifest_problems(root)
        + toolkit_problems(root)
        + budget_problems(root)
        + ci_gate_problems(root)
    )


def _run(command: list[str], root: Path, timeout: int) -> tuple[int, str]:
    try:
        completed = subprocess.run(
            command,
            cwd=root,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except FileNotFoundError as error:
        return 127, f"cannot run {' '.join(command)}: {error}"
    except subprocess.TimeoutExpired:
        return 124, f"command timed out after {timeout}s: {' '.join(command)}"
    return completed.returncode, (completed.stdout or "") + (completed.stderr or "")


def compile_gallery(root: Path, timeout: int = 3600, locked: bool = True) -> list[str]:
    """Compile every gallery example; a broken example is a release blocker."""
    problems: list[str] = []
    for directory, package, _toolkit in GALLERY:
        command = ["cargo", "build", "--example", directory, "-p", package]
        if locked:
            command.append("--locked")
        code, output = _run(command, root, timeout)
        if code != 0:
            problems.append(
                f"examples/{directory}: `{' '.join(command)}` failed with exit code {code}"
            )
            tail = output.strip().splitlines()[-20:]
            for line in tail:
                problems.append(f"  {line}")
    return problems


def smoke_gallery(root: Path, timeout: int = 600, locked: bool = True) -> list[str]:
    """Run the headless CLI example and check its output."""
    command = ["cargo", "run", "--example", "minimal-cli", "-p", "lurker-example-minimal-cli"]
    if locked:
        command.append("--locked")
    code, output = _run(command, root, timeout)
    if code != 0:
        return [f"examples/minimal-cli: `{' '.join(command)}` failed with exit code {code}"]
    expectations = ("engine:   mihomo", "Stopped -> core stopped", "Running -> real running")
    missing = [needle for needle in expectations if needle not in output]
    if missing:
        return [
            "examples/minimal-cli: the example output does not contain "
            + ", ".join(repr(needle) for needle in missing)
        ]
    return []


def self_test() -> int:
    import tempfile

    assert _normalized_spec({"version": "0.14", "default-features": False}) == (
        "version=0.14|default-features=False"
    )
    assert _normalized_spec({"workspace": True, "features": ["b", "a"]}) == (
        "workspace=True|features=a,b"
    )
    assert _normalized_spec("1.0") == "1.0"

    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "examples/minimal-cli").mkdir(parents=True)
        (root / "examples/minimal-cli/Cargo.toml").write_text(
            '[package]\nname = "lurker-example-minimal-cli"\nversion = "0.50.0"\n'
            "publish = false\nlicense = \"MIT OR Apache-2.0\"\n"
            "[dependencies]\nlurker-core-contract = { path = \"../../crates/lurker-core-contract\" }\n"
            '[[example]]\nname = "minimal-cli"\npath = "main.rs"\n',
            encoding="utf-8",
        )
        (root / "examples/minimal-cli/main.rs").write_text("fn main() {}\n", encoding="utf-8")
        problems = manifest_problems(root)
        assert not [problem for problem in problems if "minimal-cli" in problem], problems

        # Negative: an extra dependency is rejected.
        (root / "examples/minimal-cli/Cargo.toml").write_text(
            '[package]\nname = "lurker-example-minimal-cli"\nversion = "0.50.0"\n'
            "publish = false\nlicense = \"MIT OR Apache-2.0\"\n"
            "[dependencies]\nlurker-core-contract = { path = \"../../crates/lurker-core-contract\" }\n"
            'clap = "4"\n'
            '[[example]]\nname = "minimal-cli"\npath = "main.rs"\n',
            encoding="utf-8",
        )
        problems = manifest_problems(root)
        assert any("not part of the gallery allowlist" in problem for problem in problems), (
            problems
        )

        # Negative: an oversized example is rejected.
        (root / "examples/minimal-cli/Cargo.toml").write_text(
            '[package]\nname = "lurker-example-minimal-cli"\nversion = "0.50.0"\n'
            "publish = false\nlicense = \"MIT OR Apache-2.0\"\n"
            "[dependencies]\nlurker-core-contract = { path = \"../../crates/lurker-core-contract\" }\n"
            '[[example]]\nname = "minimal-cli"\npath = "main.rs"\n',
            encoding="utf-8",
        )
        (root / "examples/minimal-cli/main.rs").write_text(
            "// filler\n" * (LINE_BUDGET + 1), encoding="utf-8"
        )
        problems = budget_problems(root)
        assert any("gallery budget" in problem for problem in problems), problems

    print("examples guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mode",
        choices=("structure", "compile", "smoke"),
        default="structure",
        help="structure: offline audit; compile: build every example; smoke: run the CLI example",
    )
    parser.add_argument("--root", default=str(ROOT), help="repository root to audit")
    parser.add_argument(
        "--timeout", type=int, default=3600, help="seconds one cargo invocation may take"
    )
    parser.add_argument(
        "--allow-lock-update",
        action="store_true",
        help="drop `--locked` from the cargo invocations (lock file repair runs)",
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    root = Path(args.root).resolve()
    problems = structure_problems(root)
    if problems:
        for problem in problems:
            print(f"examples guard: {problem}", file=sys.stderr)
        print(f"examples guard: FAILED ({len(problems)} problem(s))", file=sys.stderr)
        return 1

    if args.mode == "structure":
        print(
            "examples guard: OK ("
            + ", ".join(f"{directory} ({package})" for directory, package, _ in GALLERY)
            + ")"
        )
        return 0

    locked = not args.allow_lock_update
    if args.mode == "compile":
        problems = compile_gallery(root, args.timeout, locked)
    else:
        problems = smoke_gallery(root, min(args.timeout, 600), locked)
    if problems:
        for problem in problems:
            print(f"examples guard: {problem}", file=sys.stderr)
        print(f"examples guard: FAILED ({len(problems)} problem(s))", file=sys.stderr)
        return 1
    print(f"examples guard: OK ({args.mode})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
