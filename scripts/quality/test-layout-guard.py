#!/usr/bin/env python3
"""Test layout guard for crates/infiltrator-iced.

Modeled after taskmanager's scripts/quality/test_layout_guard.py, simplified
to this crate's rules (standalone-runnable, python3 stdlib only):

1. `src/` holds production code only. The single permitted test marker is a
   short path-mounted declaration::

       #[cfg(test)]
       #[path = "../tests/gui/xxx_tests.rs"]
       mod xxx;

   Inline `mod tests { ... }` bodies, `#[test]` / `#[tokio::test]`
   attributes and stray `src/**/tests/` directories are violations.
2. Under `tests/`, only the directories `common/`, `headless/` and `gui/`
   are allowed, plus the optional entry files `common.rs`, `headless.rs`
   and `gui.rs` at the top level (the `foo.rs + foo/` shape). `mod.rs` is
   banned everywhere below `tests/`.
3. Every `#[path = ...]` mount in `src/` must point at an existing file.

Usage:
    python3 scripts/quality/test-layout-guard.py [--mode report|enforce]
    python3 scripts/quality/test-layout-guard.py --self-test

Exit status is 0 when no violations are found (or in report mode), 1 when
violations exist in enforce mode.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path

CRATE_NAME = "infiltrator-iced"
ALLOWED_TEST_DIRS = {"common", "headless", "gui"}
ALLOWED_TEST_ENTRIES = {"common.rs", "headless.rs", "gui.rs"}

# A bare test attribute: #[test], #[tokio::test], #[my::path::test].
TEST_ATTR = re.compile(r"^\s*#\[\s*(?:[\w:]+::)?test\s*(?:\(|\])")
# A cfg(test) line on its own (mount declarations); cfg_attr is NOT matched.
CFG_TEST = re.compile(r"^\s*#\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*$")
# Any module declaration, inline or file-backed.
MOD_DECL = re.compile(r"^\s*mod\s+\w+\s*(\{|;)")
PATH_ATTR = re.compile(r'^\s*#\[\s*path\s*=\s*"([^"]+)"\s*\]')
MOUNT_TARGET = re.compile(r'\.\./tests/')
# cfg(test) gate on a production item (never a mount): #[cfg(test)] mod ... on one line
INLINE_CFG_MOD = re.compile(r"^\s*#\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\b")


@dataclass(frozen=True)
class Violation:
    path: str
    line: int
    reason: str


def crate_root(repo: Path) -> Path:
    crate = repo / "crates" / CRATE_NAME
    if not (crate / "Cargo.toml").is_file():
        raise SystemExit(f"crate not found: {crate}")
    return crate


def scan_sources(repo: Path, crate: Path) -> list[Violation]:
    """No test bodies in src/: only short #[path] mounts into ../tests/."""
    violations: list[Violation] = []
    src = crate / "src"
    for path in sorted(src.rglob("*.rs")):
        relative = path.relative_to(repo).as_posix()
        parts = path.relative_to(src).parts
        if "tests" in parts:
            violations.append(
                Violation(relative, 1, "test source directory is under src/")
            )
            continue
        lines = path.read_text(encoding="utf-8").splitlines()
        for index, raw in enumerate(lines):
            number = index + 1
            if INLINE_CFG_MOD.match(raw):
                violations.append(
                    Violation(relative, number, "inline cfg(test) module in production source")
                )
                continue
            if TEST_ATTR.match(raw):
                violations.append(
                    Violation(relative, number, "inline test attribute in production source")
                )
                continue
            path_attr = PATH_ATTR.match(raw)
            if path_attr and not MOUNT_TARGET.search(path_attr.group(1)):
                violations.append(
                    Violation(relative, number, "#[path] mount must target ../tests/")
                )
                continue
            if not MOD_DECL.match(raw):
                continue
            # A module declaration. Only test-gated ones concern this guard:
            # a valid test mount pairs cfg(test) with a #[path = "../tests/…"]
            # attribute in the two lines above; the sanctioned exception is
            # the cfg(test)-gated `test_mounts` hub module that carries those
            # mounts. Anything else test-gated without a mount is a violation;
            # plain private production modules are not this guard's business.
            above = lines[max(0, index - 2) : index]
            mounted = any(
                (attr := PATH_ATTR.match(line)) and MOUNT_TARGET.search(attr.group(1))
                for line in above
            )
            if mounted:
                target_line = next(
                    PATH_ATTR.match(line) for line in above if PATH_ATTR.match(line)
                )
                target = path.parent / target_line.group(1)
                if not target.is_file():
                    violations.append(
                        Violation(relative, number, f"mount target missing: {target}")
                    )
            elif raw.strip() == "mod test_mounts;" and any(
                CFG_TEST.match(line) for line in above
            ):
                continue  # sanctioned hub: holds the ../tests/ mounts
            elif any(CFG_TEST.match(line) for line in above):
                violations.append(
                    Violation(relative, number, "test module must be path-mounted from tests/")
                )
    return violations


def scan_tests_dir(repo: Path, crate: Path) -> list[Violation]:
    """tests/ admits only common|headless|gui dirs and their entries."""
    violations: list[Violation] = []
    tests = crate / "tests"
    if not tests.is_dir():
        return violations
    for path in sorted(tests.rglob("*.rs")):
        relative = path.relative_to(repo).as_posix()
        rel_to_tests = path.relative_to(tests)
        if path.name == "mod.rs":
            violations.append(Violation(relative, 1, "mod.rs is banned under tests/"))
            continue
        if len(rel_to_tests.parts) == 1:
            if rel_to_tests.name not in ALLOWED_TEST_ENTRIES:
                violations.append(
                    Violation(
                        relative,
                        1,
                        "top-level tests/ file must be common.rs, headless.rs, or gui.rs",
                    )
                )
        elif rel_to_tests.parts[0] not in ALLOWED_TEST_DIRS:
            violations.append(
                Violation(
                    relative,
                    1,
                    "test source must be under tests/common, tests/headless, or tests/gui",
                )
            )
    return violations


def scan(repo: Path) -> list[Violation]:
    crate = crate_root(repo)
    return scan_sources(repo, crate) + scan_tests_dir(repo, crate)


def self_test() -> None:
    """Run the real scanners against a synthetic crate in a temp dir."""
    import tempfile

    good_mount = '#[cfg(test)]\n#[path = "../tests/gui/x_tests.rs"]\nmod x;\n'
    samples: dict[str, tuple[str, bool]] = {
        # path, (content, should be clean)
        "mount.rs": (good_mount, True),
        "hub.rs": ("#[cfg(test)]\nmod test_mounts;\n", True),
        "plain_mod.rs": ("mod ksni_backend;\n", True),
        "pub_mod.rs": ("pub mod spec;\n", True),
        "cfg_no_mount.rs": ("#[cfg(test)]\nmod tests;\n", False),
        "inline_mod.rs": ("#[cfg(test)]\nmod tests {\n    #[test]\n    fn a() {}\n}\n", False),
        "one_line_cfg_mod.rs": ("#[cfg(test)] mod tests { }\n", False),
        "test_attr.rs": ("#[test]\nfn a() {}\n", False),
        "tokio_attr.rs": ("#[tokio::test]\nasync fn a() {}\n", False),
        "cfg_attr_ok.rs": ("#[cfg_attr(test, allow(dead_code))]\nmod native;\n", True),
    }
    tests_samples: dict[str, tuple[str, bool]] = {
        # relative-to-tests path, (should be clean)
        "gui/x_tests.rs": (True,),
        "headless/y_tests.rs": (True,),
        "common/support.rs": (True,),
        "headless.rs": (True,),
        "common.rs": (True,),
        "gui.rs": (True,),
        "stray_tests.rs": (False,),
        "other/mod.rs": (False,),
        "other/z_tests.rs": (False,),
    }
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp)
        crate = repo / "crates" / CRATE_NAME
        (crate / "src").mkdir(parents=True)
        (crate / "Cargo.toml").write_text("[package]\n", encoding="utf-8")
        for name, (content, clean) in samples.items():
            (crate / "src" / name).write_text(content, encoding="utf-8")
            if name == "mount.rs":
                target = crate / "tests" / "gui" / "x_tests.rs"
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("#[test]\nfn x() {}\n", encoding="utf-8")
        for rel, (clean,) in tests_samples.items():
            target = crate / "tests" / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text("// probe\n", encoding="utf-8")
        violations = scan(repo)
        flagged = {Path(v.path).name for v in violations}
        for name, (_, clean) in samples.items():
            was = name in flagged
            if was == clean:
                raise RuntimeError(f"self-test failed for src sample {name!r}")
        for rel, (clean,) in tests_samples.items():
            was = Path(rel).name in flagged
            if was == clean:
                raise RuntimeError(f"self-test failed for tests sample {rel!r}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--mode",
        choices=("report", "enforce"),
        default="enforce",
        help="report lists violations without failing; enforce exits non-zero",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[2],
        help="repository root (default: two levels above this script)",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run the guard's internal marker sanity checks and exit",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        self_test()
        print("Test layout guard self-test: PASS")
        return 0
    violations = scan(args.repo_root.resolve())
    print(
        f"Test layout guard ({CRATE_NAME}): violations={len(violations)} mode={args.mode}"
    )
    for violation in violations:
        print(f" - {violation.path}:{violation.line}: {violation.reason}")
    if violations and args.mode == "enforce":
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
