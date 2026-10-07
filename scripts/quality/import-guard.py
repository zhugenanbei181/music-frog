#!/usr/bin/env python3
"""Import hygiene guard for the MusicFrog Rust workspace.

Enforces `docs/CODE_QUALITY_BASELINE.md` across production, tests and examples:
explicit imports from defining modules, no inline long paths, named aliases,
re-exports, pure type forwarding or source include! concatenation. Anonymous
trait imports and documented FFI/dependency convergence remain supported.

The shared Rust lexer masks nested comments and literals before scanning. This
checks source structure only; behavior and parity require independent evidence.

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
from rust_syntax import long_paths, mask_noncode

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


def find_violations(rel_path: str, text: str) -> list[str]:
    """Return human-readable violations for one file."""
    problems: list[str] = []
    clean = mask_noncode(text)
    for match in long_paths(text):
        line_no = clean[:match.start()].count("\n") + 1
        problems.append(f"{rel_path}:{line_no}: inline long path `{match.group()}` is banned; use the defining module")
    forwarding = re.compile(r"\bpub(?:\s*\([^()]*\))?\s+type\s+\w+\s*=\s*\(*\s*(?:::)?[A-Za-z_]\w*(?:\s*::\s*\w+)*\s*\)*\s*;")
    selector = rel_path.replace("\\", "/") in {"crates/infiltrator-http/src/lib.rs", "crates/mihomo-platform/src/defaults.rs"}
    if not selector:
        primitives = {"u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64", "bool", "str"}
        for match in forwarding.finditer(clean):
            rhs = re.sub(r"[\s()]", "", match.group().split("=", 1)[1].rstrip(";"))
            if rhs not in primitives:
                line_no = clean[:match.start()].count("\n") + 1
                problems.append(f"{rel_path}:{line_no}: pure type forwarding is banned; import its definition")
    for match in re.finditer(r"\binclude\s*!\s*\(", clean):
        line_no = clean[:match.start()].count("\n") + 1
        problems.append(f"{rel_path}:{line_no}: source include! concatenation is banned; use real modules")
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
    expect("inline type path is banned", "crates/x/src/a.rs", "fn f(_: crate::model::Thing) {}", True)
    expect("quoted comment cannot hide an inline path", "crates/x/src/a.rs", '// next "caption"\nfn f(_: crate::model::Thing) {}', True)
    expect("absolute inline path is banned", "crates/x/src/a.rs", "fn f(_: ::std::path::PathBuf) {}", True)
    expect("parenthesized type forwarding is banned", "crates/x/src/a.rs", "pub type Copy = (model :: Thing);", True)
    expect("inline function path is banned", "crates/x/src/a.rs", "fn f() { std::fs::read(path); }", True)
    expect("short imported module is fine", "crates/x/src/a.rs", "use std::fs; fn f() { fs::read(path); }", False)
    expect("pure type forwarding is banned", "crates/x/src/a.rs", "pub type Thing = model::Thing;", True)
    expect("semantic scalar alias is fine", "crates/x/src/a.rs", "pub type RequestId = u64;", False)
    expect("composed future alias is fine", "crates/x/src/a.rs", "pub type Work = Pin<Box<dyn Future<Output = ()>>>;", False)
    expect("source include is banned", "crates/x/src/a.rs", 'include!("other.rs");', True)
    expect("resource include is fine", "crates/x/src/a.rs", 'include_str!("fixture.yaml");', False)
    expect("raw literals are not code", "crates/x/src/a.rs", 'let text = r##"crate::model::Thing pub use x;"##;', False)
    expect("nested comments are not code", "crates/x/src/a.rs", '/* outer /* crate::model::Thing */ pub use x; */', False)
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
