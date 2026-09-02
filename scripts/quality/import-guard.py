#!/usr/bin/env python3
"""Import hygiene guard for the MusicFrog Rust workspace.

Enforces the two import rules documented in `docs/ARCHITECTURE.md`
(导入规范：单一权威路径):

1. **No import aliases.** Every `use` statement must import items under
   their real names: `use foo::Bar as Baz;` and `use foo::{Bar as B2};`
   are rejected. `use foo::Trait as _;` (anonymous trait import — binds
   no name) is allowed.

2. **No re-exports.** Every `pub use` / `pub(...) use` forwarding layer is
   rejected: a fact may have exactly one Rust path, reached through the
   module that defines it. Exactly two whitelisted exceptions exist (see
   `WHITELIST`): the `infiltrator_http::reqwest` dependency-version
   convergence point and the UniFFI FFI export surface of
   `infiltrator-android`.

Both rules apply to the whole tree (business code, tests, and `#[path]`
mounted test modules alike). Comments are stripped before scanning, so
prose that mentions `pub use` is never flagged; string literals are not
tokenized (accepted false-positive cost, same trade-off as
`line-guard.py`).

Usage:
    python3 scripts/quality/import-guard.py [--mode report|enforce]
    python3 scripts/quality/import-guard.py --self-test

Exit status is 0 when no violations are found (or in report mode), 1 when
violations exist in enforce mode.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

SCAN_ROOTS = ("crates",)

# Re-exports allowed at exactly these paths, each tied to a documented
# architectural exception (docs/ARCHITECTURE.md, 导入规范).
WHITELIST = {
    "crates/infiltrator-http/src/lib.rs",    # reqwest 版本收敛点
    "crates/infiltrator-android/src/lib.rs",  # UniFFI FFI 导出面
    "crates/infiltrator-android/src/uniffi_api.rs",  # UniFFI FFI 导出面
}

USE_STMT = re.compile(r"\buse\s+[^;]+;", re.S)
ALIAS_IN_USE = re.compile(r"\bas\s+([A-Za-z_][A-Za-z0-9_]*)\b")
REEXPORT = re.compile(r"\bpub(?:\s*\([^()]*\))?\s+use\b")


def strip_comments(text: str) -> str:
    """Blank out `//` line comments (incl. `///` / `//!`) and `/* */` blocks.

    Block comments do not nest (the codebase contains none); contents inside
    string literals are not tokenized and may lose a `//` — harmless for a
    detector that only looks for `use` statements afterwards.
    """
    out: list[str] = []
    in_block = False
    for line in text.splitlines():
        if in_block:
            if "*/" in line:
                line = line.split("*/", 1)[1]
                in_block = False
            else:
                out.append("")
                continue
        if "//" in line:
            head, _, _tail = line.partition("//")
            line = head
        # a block comment may open after code on the same line
        while "/*" in line:
            _head, _, rest = line.partition("/*")
            if "*/" in rest:
                line = _head + rest.split("*/", 1)[1]
            else:
                line = _head
                in_block = True
                break
        out.append(line)
    return "\n".join(out)


def find_violations(rel_path: str, text: str) -> list[str]:
    """Return human-readable violations for one file."""
    problems: list[str] = []
    clean = strip_comments(text)
    for m in USE_STMT.finditer(clean):
        stmt = " ".join(m.group(0).split())
        for a in ALIAS_IN_USE.finditer(stmt):
            if a.group(1) != "_":
                problems.append(
                    f"{rel_path}: import alias `as {a.group(1)}` is banned: {stmt}"
                )
                break
    if rel_path.replace("\\", "/") not in WHITELIST:
        for m in REEXPORT.finditer(clean):
            line_no = clean[: m.start()].count("\n") + 1
            line = clean.splitlines()[line_no - 1].strip()
            problems.append(
                f"{rel_path}:{line_no}: re-export (`{line}`) is banned; "
                "import from the defining module's canonical path"
            )
    return problems


def rs_files(repo_root: pathlib.Path) -> list[pathlib.Path]:
    files: list[pathlib.Path] = []
    for root in SCAN_ROOTS:
        base = repo_root / root
        if not base.is_dir():
            continue
        files.extend(p for p in base.rglob("*.rs") if "/target/" not in str(p))
    return sorted(files)


def run(repo_root: pathlib.Path, enforce: bool) -> int:
    violations: list[str] = []
    scanned = 0
    for f in rs_files(repo_root):
        scanned += 1
        rel = str(f.relative_to(repo_root))
        violations.extend(find_violations(rel, f.read_text(encoding="utf-8")))
    status = "enforce" if enforce else "report"
    if violations:
        for v in violations:
            print(f"VIOLATION [{status}]: {v}", file=sys.stderr)
        print(
            f"import hygiene guard: scanned={scanned} violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if enforce else 0
    print(f"import hygiene guard: scanned={scanned} violations=0")
    return 0


def self_test() -> int:
    ok = True

    def expect(label: str, rel: str, text: str, should_flag: bool) -> None:
        nonlocal ok
        got = bool(find_violations(rel, text))
        mark = "ok" if got == should_flag else "FAIL"
        if got != should_flag:
            ok = False
        print(f"  [{mark}] {label}")

    expect("plain use is fine", "crates/x/src/a.rs", "use std::io::Write;", False)
    expect("alias is banned", "crates/x/src/a.rs", "use foo::Bar as Baz;", True)
    expect("group alias is banned", "crates/x/src/a.rs", "use chrono::{Duration as D, Utc};", True)
    expect("as _ is allowed", "crates/x/src/a.rs", "use base64::Engine as _;", False)
    expect(
        "multiline group alias is banned",
        "crates/x/src/a.rs",
        "use foo::{\n    Bar as B,\n    Qux,\n};",
        True,
    )
    expect("pub use is banned", "crates/x/src/a.rs", "pub use inner::Thing;", True)
    expect(
        "pub(crate) use is banned",
        "crates/x/src/a.rs",
        "pub(crate) use inner::Thing;",
        True,
    )
    expect(
        "whitelisted http lib allows pub use",
        "crates/infiltrator-http/src/lib.rs",
        "pub use reqwest;",
        False,
    )
    expect(
        "comments mentioning pub use are ignored",
        "crates/x/src/a.rs",
        "// the old `pub use` forwarding layer is gone\nuse std::fmt;",
        False,
    )
    expect(
        "doc comment mentioning alias is ignored",
        "crates/x/src/a.rs",
        "/// never write `use x as y`\nuse std::fmt;",
        False,
    )
    print("self-test:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    repo_root = pathlib.Path(__file__).resolve().parents[2]
    return run(repo_root, enforce=args.mode == "enforce")


if __name__ == "__main__":
    sys.exit(main())
