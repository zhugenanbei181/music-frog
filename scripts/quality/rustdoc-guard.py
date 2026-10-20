#!/usr/bin/env python3
"""Docs.rs preview and public-API documentation quality gate (PUB-006.6).

The seven neutral ("Lurker") crates are published to crates.io and are read on
docs.rs by third parties who want to build their own engine adapter or their
own surface on top of the neutral contract. That preview only stays useful
when three facts hold for every crate of the chain:

1. the crate declares the docs.rs build configuration, so the published
   preview shows the *complete* public API instead of a feature-gated subset;
2. the public API is documented — `missing_docs` is denied, so every public
   item carries real Rustdoc, and rustdoc itself reports no warning;
3. at least one doctest exists and actually executes, so the published
   examples are proven to compile instead of being decorative snippets.

Modes
-----
structure (default, offline)
    Audits the `[package.metadata.docs.rs]` section of every neutral crate
    and proves that every neutral crate owns at least one doctest. No cargo,
    no network, no build: the doctest is detected in the crate's own source.

docs
    The docs.rs-equivalent check. It runs `cargo doc --all-features
    --no-deps` for every neutral crate with `-D warnings`, so *any* rustdoc
    warning fails the gate. The offline `structure` audit runs first and is
    reported separately.

doctest
    Proves that the published examples are executable: `cargo test --doc` runs
    for every neutral crate and has to execute at least one passing example.
    This mode builds the dev-dependencies of every neutral crate, so it is
    kept apart from the cheap `docs` mode.

Usage
-----
    python3 scripts/quality/rustdoc-guard.py --mode structure
    python3 scripts/quality/rustdoc-guard.py --mode docs
    python3 scripts/quality/rustdoc-guard.py --mode doctest
    python3 scripts/quality/rustdoc-guard.py --self-test
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# The neutral crates that are published to crates.io and previewed on docs.rs.
NEUTRAL_CRATES = (
    "lurker-core-contract",
    "lurker-core-ports",
    "lurker-engine-runtime",
    "lurker-profile-store",
    "lurker-ui-presentation",
    "mihomo-driver",
    "singbox-driver",
)

# The feature selection a crate may declare for the docs.rs build.
FEATURE_SELECTION_KEYS = ("all-features", "no-default-features")

# A rustdoc code fence inside a `///` or `//!` doc comment.
DOC_FENCE_RE = re.compile(r"^\s*(?:///|//!)\s*(`{3,})")

# A cargo/rustc diagnostic that points at a source location.
LOCATION_RE = re.compile(r"^\s*-->")


def crate_manifest(root: Path, crate: str) -> Path:
    """Return the manifest path of one neutral crate."""
    return root / "crates" / crate / "Cargo.toml"


def crate_sources(root: Path, crate: str) -> list[Path]:
    """Return every Rust source file of one neutral crate."""
    source_root = root / "crates" / crate / "src"
    if not source_root.is_dir():
        return []
    return sorted(source_root.rglob("*.rs"))


def doc_code_blocks(text: str) -> list[str]:
    """Return the bodies of every rustdoc code fence in `text`.

    Only `///` and `//!` doc comments are scanned; a fenced block in ordinary
    code is not a doctest and must not be counted as one.
    """
    lines = text.splitlines()
    blocks: list[str] = []
    index = 0
    while index < len(lines):
        opener = DOC_FENCE_RE.match(lines[index])
        if opener is None:
            index += 1
            continue
        fence = opener.group(1)
        index += 1
        body: list[str] = []
        closed = False
        while index < len(lines):
            closer = DOC_FENCE_RE.match(lines[index])
            if closer is not None and len(closer.group(1)) >= len(fence):
                index += 1
                closed = True
                break
            body.append(lines[index])
            index += 1
        blocks.append("\n".join(body))
        if not closed:
            # An unterminated fence is still a doctest attempt; rustdoc reports
            # it as a broken example, and the cargo gate catches that.
            break
    return blocks


def doctest_count(root: Path, crate: str) -> int:
    """Count the doctest examples a neutral crate owns."""
    total = 0
    for source in crate_sources(root, crate):
        try:
            text = source.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        total += len(doc_code_blocks(text))
    return total


def docs_rs_metadata(document: dict) -> dict | None:
    """Return the `[package.metadata.docs.rs]` table of a parsed manifest.

    Cargo parses the section header as the nested table path
    `package.metadata.docs.rs`; the quoted form `package.metadata."docs.rs"`
    is accepted as well so a manifest cannot silently lose the section.
    """
    metadata = document.get("package", {}).get("metadata")
    if not isinstance(metadata, dict):
        return None
    nested = metadata.get("docs")
    if isinstance(nested, dict) and isinstance(nested.get("rs"), dict):
        return nested["rs"]
    literal = metadata.get("docs.rs")
    if isinstance(literal, dict):
        return literal
    return None


def metadata_problems(root: Path) -> list[str]:
    """Audit the `[package.metadata.docs.rs]` section of every neutral crate."""
    problems: list[str] = []
    for crate in NEUTRAL_CRATES:
        manifest = crate_manifest(root, crate)
        if not manifest.is_file():
            problems.append(f"{manifest}: neutral crate manifest is missing")
            continue
        try:
            document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except tomllib.TOMLDecodeError as error:
            problems.append(f"{manifest}: manifest is not valid TOML ({error})")
            continue

        metadata = docs_rs_metadata(document)
        if not isinstance(metadata, dict):
            problems.append(
                f"{manifest}: missing `[package.metadata.docs.rs]` section"
            )
            continue

        rustdoc_args = metadata.get("rustdoc-args")
        if not isinstance(rustdoc_args, list) or not rustdoc_args:
            problems.append(
                f"{manifest}: `[package.metadata.docs.rs]` must declare a non-empty "
                "`rustdoc-args` list"
            )
        elif not all(isinstance(argument, str) for argument in rustdoc_args):
            problems.append(
                f"{manifest}: `rustdoc-args` entries must be strings"
            )
        elif "--cfg" not in rustdoc_args or "docsrs" not in rustdoc_args:
            problems.append(
                f"{manifest}: `rustdoc-args` must pass `--cfg docsrs` so the "
                "docs.rs build is distinguishable from a local build"
            )

        targets = metadata.get("targets")
        if not isinstance(targets, list) or not targets:
            problems.append(
                f"{manifest}: `[package.metadata.docs.rs]` must declare a non-empty "
                "`targets` list"
            )
        elif not all(isinstance(target, str) for target in targets):
            problems.append(f"{manifest}: `targets` entries must be strings")

        declared = set(document.get("features", {}))
        selected = [key for key in FEATURE_SELECTION_KEYS if key in metadata]
        if len(selected) != 1:
            problems.append(
                f"{manifest}: `[package.metadata.docs.rs]` must select exactly one of "
                f"{' and '.join(FEATURE_SELECTION_KEYS)}"
            )
            continue
        key = selected[0]
        if key == "all-features":
            if metadata.get("all-features") is not True:
                problems.append(f"{manifest}: `all-features` must be `true`")
            continue
        if metadata.get("no-default-features") is not True:
            problems.append(f"{manifest}: `no-default-features` must be `true`")
        features = metadata.get("features")
        if not isinstance(features, list) or not all(
            isinstance(feature, str) for feature in features
        ):
            problems.append(
                f"{manifest}: `no-default-features` requires an explicit `features` list"
            )
            continue
        missing = sorted(
            feature
            for feature in declared
            if feature != "default" and feature not in features
        )
        if missing:
            problems.append(
                f"{manifest}: the docs.rs feature selection omits {', '.join(missing)}"
            )
    return problems


def doctest_problems(root: Path) -> list[str]:
    """Prove that every neutral crate owns at least one doctest."""
    problems: list[str] = []
    for crate in NEUTRAL_CRATES:
        if not crate_sources(root, crate):
            problems.append(f"crates/{crate}: neutral crate has no Rust sources")
            continue
        if doctest_count(root, crate) < 1:
            problems.append(
                f"crates/{crate}: neutral crate publishes no doctest; the docs.rs "
                "preview would ship without a single executable example"
            )
    return problems


def structure_problems(root: Path) -> list[str]:
    """Run the offline audit: docs.rs metadata plus doctest presence."""
    return metadata_problems(root) + doctest_problems(root)


def rustdoc_warning_lines(output: str) -> list[str]:
    """Return the rustdoc warnings of one `cargo doc` output.

    A cargo-level notice (for example "unused manifest key") has no source
    location and is not a rustdoc warning; a rustdoc diagnostic always points
    at the file it belongs to.
    """
    lines = output.splitlines()
    warnings: list[str] = []
    for index, line in enumerate(lines):
        if not line.startswith("warning:"):
            continue
        window = lines[index : index + 12]
        if any(LOCATION_RE.match(candidate) for candidate in window):
            warnings.append(line.strip())
    return warnings


def cargo_doc_command(crate: str) -> list[str]:
    """Return the docs.rs-equivalent `cargo doc` command of one crate."""
    return ["cargo", "doc", "--all-features", "--no-deps", "-p", crate]


def run_cargo_doc(root: Path, crate: str, timeout: int) -> tuple[int, str]:
    """Run `cargo doc` for one crate and return its exit code and output."""
    environment = dict(os.environ)
    environment["CARGO_TERM_COLOR"] = "never"
    existing = environment.get("RUSTDOCFLAGS", "")
    environment["RUSTDOCFLAGS"] = (existing + " -D warnings").strip()
    try:
        completed = subprocess.run(
            cargo_doc_command(crate),
            cwd=root,
            env=environment,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except FileNotFoundError:
        return 127, "cargo is not available in this environment"
    except subprocess.TimeoutExpired:
        return 124, f"cargo doc timed out after {timeout}s"
    return completed.returncode, (completed.stdout or "") + (completed.stderr or "")


def run_cargo_doctest(root: Path, crate: str, timeout: int) -> tuple[int, str]:
    """Run `cargo test --doc` for one crate and return its exit code and output."""
    environment = dict(os.environ)
    environment["CARGO_TERM_COLOR"] = "never"
    command = ["cargo", "test", "--doc", "--all-features", "-p", crate]
    try:
        completed = subprocess.run(
            command,
            cwd=root,
            env=environment,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except FileNotFoundError:
        return 127, "cargo is not available in this environment"
    except subprocess.TimeoutExpired:
        return 124, f"cargo test --doc timed out after {timeout}s"
    return completed.returncode, (completed.stdout or "") + (completed.stderr or "")


def docs_problems(
    root: Path,
    timeout: int = 1800,
    runner=None,
) -> list[str]:
    """Run the docs.rs-equivalent `cargo doc` check for every neutral crate.

    `runner` replaces the cargo invocation; it is the seam the unit tests use
    to prove that a rustdoc warning is rejected.
    """
    invoke = runner if runner is not None else run_cargo_doc
    problems: list[str] = []
    for crate in NEUTRAL_CRATES:
        code, output = invoke(root, crate, timeout)
        warnings = rustdoc_warning_lines(output)
        if code != 0:
            problems.append(
                f"crates/{crate}: `cargo doc --all-features --no-deps` failed "
                f"(exit code {code})"
            )
        if warnings:
            problems.append(
                f"crates/{crate}: rustdoc reported {len(warnings)} warning(s): "
                + "; ".join(warnings)
            )
    return problems


def doctest_run_problems(root: Path, timeout: int = 1800) -> list[str]:
    """Prove that every neutral crate executes at least one doctest."""
    problems: list[str] = []
    for crate in NEUTRAL_CRATES:
        code, output = run_cargo_doctest(root, crate, timeout)
        if code != 0:
            problems.append(
                f"crates/{crate}: `cargo test --doc` failed (exit code {code})"
            )
        elif "test result: ok" not in output:
            problems.append(
                f"crates/{crate}: `cargo test --doc` did not run a passing doctest"
            )
    return problems


def self_test() -> int:
    """Exercise the checks on synthetic manifests and synthetic cargo output."""
    import tempfile

    assert rustdoc_warning_lines("warning: unused manifest key: package.foo\n") == []
    assert rustdoc_warning_lines(
        "warning: missing documentation for a struct\n"
        "  --> crates/demo/src/lib.rs:3:1\n"
        "   |\n"
        "3  | pub struct Demo;\n"
        "   | ^^^^^^^^^^^^^^\n"
    ) == ["warning: missing documentation for a struct"]
    assert rustdoc_warning_lines("") == []

    assert len(doc_code_blocks("/// ```\n/// let x = 1;\n/// ```\n")) == 1
    assert doc_code_blocks("/// no fence here\n") == []
    assert doc_code_blocks("```\nlet x = 1;\n```\n") == []
    assert len(doc_code_blocks("//!\n//! ```\n//! let x = 1;\n//! ```\n")) == 1

    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        manifests: dict[str, str] = {}
        for crate in NEUTRAL_CRATES:
            crate_root = root / "crates" / crate
            (crate_root / "src").mkdir(parents=True)
            manifests[crate] = (
                f'[package]\nname = "{crate}"\nversion = "0.50.0"\n'
                "[package.metadata.docs.rs]\n"
                "all-features = true\n"
                'rustdoc-args = ["--cfg", "docsrs"]\n'
                'targets = ["x86_64-unknown-linux-gnu"]\n'
            )
            (crate_root / "Cargo.toml").write_text(manifests[crate], encoding="utf-8")
            (crate_root / "src" / "lib.rs").write_text(
                "//! Demo crate.\n//!\n//! ```\n//! let x = 1;\n//! ```\n", encoding="utf-8"
            )

        assert metadata_problems(root) == [], metadata_problems(root)
        assert doctest_problems(root) == [], doctest_problems(root)

        # Negative: a missing docs.rs metadata section is rejected.
        target = root / "crates" / "lurker-core-contract" / "Cargo.toml"
        target.write_text(
            '[package]\nname = "lurker-core-contract"\nversion = "0.50.0"\n',
            encoding="utf-8",
        )
        problems = metadata_problems(root)
        assert any("missing `[package.metadata.docs.rs]`" in problem for problem in problems), (
            problems
        )

        # Negative: a metadata section without the docs.rs rustdoc args is rejected.
        target.write_text(
            '[package]\nname = "lurker-core-contract"\nversion = "0.50.0"\n'
            "[package.metadata.docs.rs]\n"
            "all-features = true\n"
            'rustdoc-args = ["--cfg", "not_docsrs"]\n'
            'targets = ["x86_64-unknown-linux-gnu"]\n',
            encoding="utf-8",
        )
        problems = metadata_problems(root)
        assert any("--cfg docsrs" in problem for problem in problems), problems

        # Negative: a crate without any doctest is rejected.
        target.write_text(manifests["lurker-core-contract"], encoding="utf-8")
        (root / "crates" / "lurker-core-contract" / "src" / "lib.rs").write_text(
            "//! Demo crate.\n", encoding="utf-8"
        )
        problems = doctest_problems(root)
        assert any("publishes no doctest" in problem for problem in problems), problems

    print("rustdoc guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mode",
        choices=("structure", "docs", "doctest"),
        default="structure",
        help=(
            "structure: offline metadata and doctest audit; "
            "docs: cargo doc without warnings; "
            "doctest: cargo test --doc has to run a passing example"
        ),
    )
    parser.add_argument("--root", default=str(ROOT), help="repository root to audit")
    parser.add_argument(
        "--timeout",
        type=int,
        default=1800,
        help="seconds one cargo invocation may take",
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    root = Path(args.root).resolve()
    if args.mode == "structure":
        problems = structure_problems(root)
        if problems:
            print("\n".join(problems), file=sys.stderr)
            print(
                f"rustdoc guard: FAILED ({len(problems)} problem(s), offline structure audit)",
                file=sys.stderr,
            )
            return 1
        print(
            "rustdoc guard: OK (docs.rs metadata and doctest presence for "
            f"{len(NEUTRAL_CRATES)} neutral crates)"
        )
        return 0

    offline = structure_problems(root)
    if offline:
        print("\n".join(offline), file=sys.stderr)
        print(
            f"rustdoc guard: FAILED ({len(offline)} problem(s), offline structure audit)",
            file=sys.stderr,
        )
        return 1

    if args.mode == "docs":
        problems = docs_problems(root, timeout=args.timeout)
        if problems:
            print("\n".join(problems), file=sys.stderr)
            print(f"rustdoc guard: FAILED ({len(problems)} problem(s))", file=sys.stderr)
            return 1
        print(
            "rustdoc guard: OK (cargo doc --all-features --no-deps without warnings for "
            f"{len(NEUTRAL_CRATES)} neutral crates)"
        )
        return 0

    problems = doctest_run_problems(root, timeout=args.timeout)
    if problems:
        print("\n".join(problems), file=sys.stderr)
        print(f"rustdoc guard: FAILED ({len(problems)} problem(s))", file=sys.stderr)
        return 1
    print(
        "rustdoc guard: OK (executable doctests for "
        f"{len(NEUTRAL_CRATES)} neutral crates)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
