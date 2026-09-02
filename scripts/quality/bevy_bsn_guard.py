#!/usr/bin/env python3
"""Bevy UI authoring guard: 100% ``bsn!`` scene composition (BEVY-004).

The two Bevy crates (`infiltrator-bevy-widgets`, `infiltrator-bevy-ui`) have
exactly one sanctioned route for declaring a UI tree: a scene built by the
``bsn!`` macro and mounted through ``Commands::spawn_scene`` (crate law,
docs/BEVY_UI_FRONTEND.md). The guard deliberately does not restrict ECS state
binding — observers restamping components in place are the sanctioned runtime
route. It rejects the imperative/parallel hierarchy routes instead:

* BEVY-BSN-001 — UI primitives (``Node { ... }``, ``Children [ ... ]``,
  ``Text( ... )``) or legacy UI bundles (``NodeBundle`` / ``TextBundle`` /
  ``ButtonBundle`` / ``ImageBundle`` / ``ChildBuilder`` /
  ``ChildSpawnerCommands``) appearing outside a ``bsn! { ... }`` scene span.
* BEVY-BSN-002 — manual child-link APIs (``with_children`` /
  ``push_children`` / ``add_child`` / ``add_children``), anywhere: inside a
  scene they are meaningless, outside one they are the imperative tree route.
* BEVY-BSN-003 — direct entity spawn via ``.spawn(`` / ``.spawn_batch(``.
  Receiver-agnostic on purpose: Commands, World, ``world_mut()``,
  EntityWorldMut and child builders are all entity-tree routes. The sanctioned
  mounting seam ``spawn_scene`` never matches (the literal ``spawn`` must be
  followed directly by ``(`` or ``_batch(``).
* BEVY-BSN-004 — an unbalanced ``bsn! {`` (fails safe: a scene the scanner
  cannot bracket is treated as no scene at all).

Exemptions are mechanical, decided from the spawn argument text (no per-file
allowlist, no line numbers to maintain):

* ``commands.spawn(Camera2d)`` — camera infrastructure, not a UI tree
  (today: crates/infiltrator-bevy-ui/src/app.rs);
* ``commands.spawn(Observer::new( ... ))`` — observer infrastructure, the
  sanctioned runtime-restamp route.

Rule: the spawn's first argument expression must *start with* the whitelisted
text, so ``spawn(Camera2d)`` passes and ``spawn((Camera2d, Marker))`` still
fails (compose that shape as a scene, or extend the whitelist deliberately).
A future TaskPool-style ``.spawn(future)`` would also need a whitelist entry —
failing safe is the design.

Comments and string/char literals are masked before scanning with offsets
preserved, so prose and doc examples can neither satisfy nor trip the rule.
Scanned scope is production code only: the ``src/`` trees of the two Bevy
crates; dedicated ``tests/`` directories and ``*_test(s).rs`` modules are
outside the authoring contract. Violation codes intentionally match
taskmanager's ``bevy_bsn_guard.py`` for cross-project greppability.

Usage:
    python3 scripts/quality/bevy_bsn_guard.py [--mode report|enforce] [--root PATH]
    python3 scripts/quality/bevy_bsn_guard.py --self-test

Exit status is 0 when no violations are found (or in report mode), 1 when
violations exist in enforce mode.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
from dataclasses import dataclass

SCAN_ROOTS = (
    "crates/infiltrator-bevy-widgets/src",
    "crates/infiltrator-bevy-ui/src",
)

BSN_START = re.compile(r"\bbsn!\s*\{")
UI_CONSTRUCTION = re.compile(
    r"\b(?:Node|Children|Text)\s*(?:\{|\[|\()"
    r"|\b(?:NodeBundle|TextBundle|ButtonBundle|ImageBundle|ChildBuilder|"
    r"ChildSpawnerCommands)\b"
)
MANUAL_CHILD_API = re.compile(
    r"\.\s*(?:with_children|push_children|add_child|add_children)\s*\("
)
DIRECT_SPAWN = re.compile(r"\.\s*spawn(?:_batch)?\s*\(")
# Spawn arguments that are infrastructure, not UI trees (see module docstring).
ALLOWED_SPAWN_ARGUMENTS = ("Camera2d", "Observer::new")


@dataclass(frozen=True)
class Violation:
    path: str
    line: int
    code: str
    detail: str


def _blank(chars: list[str], start: int, end: int) -> None:
    """Replace a non-code range with spaces while retaining line endings."""
    for index in range(start, min(end, len(chars))):
        if chars[index] not in "\r\n":
            chars[index] = " "


def mask_rust(text: str) -> str:
    """Mask comments and literals without changing offsets or line numbers."""
    chars = list(text)
    length = len(text)
    index = 0
    block_depth = 0
    while index < length:
        pair = text[index : index + 2]
        if block_depth:
            if pair == "/*":
                _blank(chars, index, index + 2)
                block_depth += 1
                index += 2
            elif pair == "*/":
                _blank(chars, index, index + 2)
                block_depth -= 1
                index += 2
            else:
                _blank(chars, index, index + 1)
                index += 1
            continue

        if pair == "//":
            end = text.find("\n", index)
            _blank(chars, index, length if end < 0 else end)
            index = length if end < 0 else end
            continue
        if pair == "/*":
            _blank(chars, index, index + 2)
            block_depth = 1
            index += 2
            continue

        raw = re.match(r"(?:br|r)(#+)?\"", text[index:])
        if raw:
            hashes = raw.group(1) or ""
            content_start = index + len(raw.group(0))
            terminator = f'"{hashes}'
            end = text.find(terminator, content_start)
            end = length if end < 0 else end + len(terminator)
            _blank(chars, index, end)
            index = end
            continue

        if text[index] == '"':
            end = index + 1
            escaped = False
            while end < length:
                current = text[end]
                if current == "\n" and not escaped:
                    break
                if current == '"' and not escaped:
                    end += 1
                    break
                if current == "\\" and not escaped:
                    escaped = True
                else:
                    escaped = False
                end += 1
            _blank(chars, index, end)
            index = end
            continue

        # A Rust character literal can contain braces or quotes. A lifetime
        # (`'name`) has no closing quote and is intentionally left as code.
        if text[index] == "'":
            end = index + 1
            escaped = False
            while end < length and text[end] not in "\r\n":
                current = text[end]
                if current == "'" and not escaped:
                    end += 1
                    _blank(chars, index, end)
                    index = end
                    break
                if current == "\\" and not escaped:
                    escaped = True
                else:
                    escaped = False
                end += 1
            else:
                index += 1
            continue

        index += 1
    return "".join(chars)


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def scene_spans(masked: str) -> tuple[list[tuple[int, int]], list[int]]:
    """Return balanced ``bsn! { ... }`` spans and unbalanced start offsets."""
    spans: list[tuple[int, int]] = []
    unbalanced: list[int] = []
    for match in BSN_START.finditer(masked):
        opening = masked.find("{", match.start(), match.end())
        depth = 0
        closing = None
        for index in range(opening, len(masked)):
            if masked[index] == "{":
                depth += 1
            elif masked[index] == "}":
                depth -= 1
                if depth == 0:
                    closing = index + 1
                    break
        if closing is None:
            unbalanced.append(match.start())
        else:
            spans.append((match.start(), closing))
    return spans, unbalanced


def inside_scene(offset: int, spans: list[tuple[int, int]]) -> bool:
    return any(start <= offset < end for start, end in spans)


def matching_paren(masked: str, opening: int) -> int | None:
    depth = 0
    for index in range(opening, len(masked)):
        if masked[index] == "(":
            depth += 1
        elif masked[index] == ")":
            depth -= 1
            if depth == 0:
                return index
    return None


def analyze(rel_path: str, text: str) -> list[Violation]:
    """Violations for one file: `text` is raw Rust, masking happens here."""
    original = text
    masked = mask_rust(text)
    spans, unbalanced = scene_spans(masked)
    violations: list[Violation] = []

    for offset in unbalanced:
        violations.append(
            Violation(
                rel_path,
                line_number(original, offset),
                "BEVY-BSN-004",
                "bsn! scene has unbalanced braces",
            )
        )

    for match in UI_CONSTRUCTION.finditer(masked):
        if not inside_scene(match.start(), spans):
            violations.append(
                Violation(
                    rel_path,
                    line_number(original, match.start()),
                    "BEVY-BSN-001",
                    "UI hierarchy construction must be inside a bsn! Scene",
                )
            )

    for match in MANUAL_CHILD_API.finditer(masked):
        violations.append(
            Violation(
                rel_path,
                line_number(original, match.start()),
                "BEVY-BSN-002",
                "manual child-link APIs are forbidden; compose a bsn! Scene",
            )
        )

    for match in DIRECT_SPAWN.finditer(masked):
        opening = masked.find("(", match.start(), match.end())
        closing = matching_paren(masked, opening)
        arguments = masked[opening + 1 : closing].lstrip() if closing else ""
        if arguments.startswith(ALLOWED_SPAWN_ARGUMENTS):
            continue
        violations.append(
            Violation(
                rel_path,
                line_number(original, match.start()),
                "BEVY-BSN-003",
                "direct entity spawn is forbidden; mount UI through spawn_scene",
            )
        )
    return violations


def is_excluded(path: pathlib.Path) -> bool:
    """Dedicated test trees and `#[path]`-mounted test modules sit outside the
    production authoring contract (same exclusion shape as line-guard.py)."""
    return (
        "target" in path.parts
        or "tests" in path.parts
        or path.stem.endswith(("_test", "_tests"))
    )


def rs_files(root: pathlib.Path) -> list[pathlib.Path]:
    if root.is_file():
        return [root] if root.suffix == ".rs" and not is_excluded(root) else []
    return sorted(
        path for path in root.rglob("*.rs") if path.is_file() and not is_excluded(path)
    )


def display_path(path: pathlib.Path, repo_root: pathlib.Path) -> str:
    try:
        return path.resolve().relative_to(repo_root).as_posix()
    except ValueError:
        return path.as_posix()


def scan(repo_root: pathlib.Path, roots: list[pathlib.Path]) -> list[Violation]:
    violations: list[Violation] = []
    for base in roots:
        for path in rs_files(base):
            rel = display_path(path, repo_root)
            text = path.read_text(encoding="utf-8", errors="replace")
            violations.extend(analyze(rel, text))
    return violations


def run(repo_root: pathlib.Path, enforce: bool, root: pathlib.Path | None) -> int:
    roots = [root] if root is not None else [repo_root / r for r in SCAN_ROOTS]
    violations = scan(repo_root, roots)
    scanned = sum(len(rs_files(base)) for base in roots)
    status = "enforce" if enforce else "report"
    if violations:
        for v in violations:
            print(
                f"VIOLATION [{status}]: {v.path}:{v.line}: {v.code}: {v.detail}",
                file=sys.stderr,
            )
        print(
            f"bevy bsn guard: scanned={scanned} violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if enforce else 0
    print(f"bevy bsn guard: scanned={scanned} violations=0")
    return 0


def self_test() -> int:
    """Positive and negative cases inline, plus: the real production trees of
    the two Bevy crates must currently pass (guards against rule drift that
    would flag the compliant tree, or exemptions that would swallow it)."""
    ok = True

    def expect(label: str, text: str, codes: list[str]) -> None:
        nonlocal ok
        got = sorted(v.code for v in analyze("self-test.rs", text))
        want = sorted(codes)
        if got == want:
            print(f"  [ok] {label}")
        else:
            ok = False
            print(f"  [FAIL] {label}: expected {want}, found {got}")

    expect(
        "bsn! scene with Node/Children/Text is the sanctioned route",
        """
        fn scene(label: String) -> impl Scene {
            bsn! {
                Node { width: percent(100) }
                BackgroundColor({ palette.surface })
                Children [ ( Text(label) TextRole(Role::Body) ) ]
            }
        }
        """,
        [],
    )
    expect(
        "camera infrastructure spawn is allowed",
        "fn camera(mut commands: Commands) { commands.spawn(Camera2d); }",
        [],
    )
    expect(
        "observer infrastructure spawn is allowed",
        "fn obs(mut commands: Commands) { commands.spawn(Observer::new(on_add)); }",
        [],
    )
    expect(
        "spawn_scene is the sanctioned mounting seam",
        "fn mount(mut commands: Commands) { commands.spawn_scene(shell_scene()); }",
        [],
    )
    expect(
        "comments and string/char/raw literals are masked",
        """
        // Node { width: px(1.0) } Children [ Text(x) ] commands.spawn(Foo)
        /// let children = Children [ ];
        let doc = "Node { } Text(x) commands.spawn(Foo)";
        let raw = r#"Children [ Node { } ]"#;
        let ch = '"';
        """,
        [],
    )
    expect(
        "type mentions without construction are fine",
        """
        use bevy::ui::widget::Text;
        use bevy::ecs::hierarchy::Children;
        #[derive(Component)]
        struct TextRole(pub Role);
        let n = Node::default();
        fn borrow<'a>(x: &'a str) -> &'a str { x }
        """,
        [],
    )
    expect(
        "Node outside bsn! is banned",
        "let n = Node { width: px(1.0) };",
        ["BEVY-BSN-001"],
    )
    expect(
        "Children outside bsn! is banned",
        "let c = Children [];",
        ["BEVY-BSN-001"],
    )
    expect(
        "Text(...) outside bsn! is banned",
        'let t = Text("hi");',
        ["BEVY-BSN-001"],
    )
    expect(
        "legacy UI bundle outside bsn! is banned",
        "fn bundle() -> NodeBundle { todo!() }",
        ["BEVY-BSN-001"],
    )
    expect(
        "manual child-link API is banned (closure body spawn caught too)",
        "commands.entity(root).with_children(|p| { p.spawn(A); });",
        ["BEVY-BSN-002", "BEVY-BSN-003"],
    )
    expect(
        "direct UI spawn is banned",
        "commands.spawn(Button);",
        ["BEVY-BSN-003"],
    )
    expect(
        "world.spawn UI tree is banned (primitive + spawn both flagged)",
        "world.spawn((Node { width: px(1.0) },));",
        ["BEVY-BSN-001", "BEVY-BSN-003"],
    )
    expect(
        "world_mut().spawn is banned",
        "world_mut().spawn(ContentSlot);",
        ["BEVY-BSN-003"],
    )
    expect(
        "spawn_batch is banned",
        "commands.spawn_batch(rows);",
        ["BEVY-BSN-003"],
    )
    expect(
        "non-whitelisted spawn argument fails even with camera-adjacent text",
        "commands.spawn(CameraMarker);",
        ["BEVY-BSN-003"],
    )
    expect(
        "unbalanced bsn! brace fails safe",
        "fn broken() { bsn! {\n",
        ["BEVY-BSN-004"],
    )

    # The real production trees must stay clean — this is the same scan CI
    # runs, so self-test also catches a repo state the rule would reject.
    repo_root = pathlib.Path(__file__).resolve().parents[2]
    roots = [repo_root / r for r in SCAN_ROOTS]
    found = scan(repo_root, roots)
    scanned = sum(len(rs_files(base)) for base in roots)
    if scanned == 0:
        ok = False
        print("  [FAIL] real production trees not found — SCAN_ROOTS drifted?")
    elif found:
        ok = False
        for v in found:
            print(f"  [FAIL] real file violates: {v.path}:{v.line}: {v.code}: {v.detail}")
    else:
        print(f"  [ok] real production files pass ({scanned} files scanned)")

    print("self-test:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument(
        "--root",
        type=pathlib.Path,
        default=None,
        help="scan this file or directory instead of the two default bevy crates",
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()

    repo_root = pathlib.Path(__file__).resolve().parents[2]
    root = args.root
    if root is not None and not root.is_absolute():
        root = repo_root / root
    return run(repo_root, enforce=args.mode == "enforce", root=root)


if __name__ == "__main__":
    sys.exit(main())
