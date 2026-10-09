#!/usr/bin/env python3
"""Bevy UI authoring guard: 100% ``bsn!`` scene composition (BEVY-004).

The two Bevy crates (`infiltrator-bevy-widgets`, `infiltrator-bevy-ui`) have
exactly one sanctioned route for declaring a UI tree: a scene built by the
``bsn!`` macro and mounted through ``Commands::spawn_scene`` (crate law,
docs/bevy-ui/BEVY_UI_FRONTEND.md). The dividing line the guard enforces is
**structure vs state**: structure (the tree, its layout and text primitives)
may only be declared inside a ``bsn!`` scene; state (colors, images, markers,
selection bits) may be restamped in place by observers and systems. It
rejects every imperative/parallel structure route instead:

* BEVY-BSN-001 — structure primitives (``Node { ... }``, ``Children [ ... ]``,
  ``Text( ... )``) or legacy UI bundles (``NodeBundle`` / ``TextBundle`` /
  ``ButtonBundle`` / ``ImageBundle``) appearing outside a ``bsn! { ... }``
  scene span. Visual components (``ImageNode``, ``BackgroundColor``) are
  deliberately out of this list: stamping them onto existing tree entities is
  the sanctioned observer route.
* BEVY-BSN-002 — manual child-link APIs and spawner types, anywhere: inside a
  scene they are meaningless, outside one they are the imperative tree route.
  Methods: ``with_children`` / ``push_children`` / ``add_child`` /
  ``add_children`` / ``insert_children`` / ``replace_children`` /
  ``add_related`` / ``insert_related`` (turbofish form included). Types:
  ``ChildBuilder`` / ``ChildSpawner(Commands)`` / ``RelatedSpawner(Commands)``
  — a system parameter declaring a spawner is a hierarchy route.
  ``despawn_children`` / ``remove_child`` / ``remove_children`` are removal
  routes (bounded-subtree replacement) and stay allowed.
* BEVY-BSN-003 — direct entity spawn via ``.spawn(`` / ``.spawn_batch(`` /
  ``.spawn_empty(`` / ``.spawn_empty_at(``. Receiver-agnostic on purpose:
  Commands, World, ``world_mut()``, EntityWorldMut and child builders are all
  entity-tree routes. The sanctioned mounting seam ``spawn_scene`` never
  matches (the literal ``spawn`` must be followed directly by ``(``,
  ``_batch(``, ``_empty(`` or ``_empty_at(``).
* BEVY-BSN-004 — an unbalanced ``bsn! {`` (fails safe: a scene the scanner
  cannot bracket is treated as no scene at all).
* BEVY-BSN-005 — temporary-value constructors of structure primitives outside
  a scene: ``Node::default()`` / ``Node::EMPTY`` / ``Text::new(...)`` /
  ``Children::from_*`` and friends. This is the placeholder wall: an empty or
  default structure value conjured outside the scene, then stuffed into the
  tree via ``insert`` or filled in imperatively afterwards, is the same
  bypass as BEVY-BSN-001 with different spelling. Sanctioned restamps mutate
  fields in place (``node.width = ...``); ``..Node::default()`` spreads inside
  a scene are fine. ``accesskit::Node::new`` is the one mechanical exemption —
  that ``Node`` is the accessibility tree, not UI structure.
* BEVY-ECS-007 — unrestricted World/EntityWorldMut/UnsafeWorldCell and
  runtime SystemState access are forbidden, including fields, wrappers and
  exclusive systems. DeferredWorld is also forbidden: lifecycle observers
  declare precise ECS access rather than receiving an unrestricted context.
* BEVY-ECS-008 — App world access is forbidden in production, including plugin
  assembly. Assets and captures initialize through restricted systems.
* BEVY-ECS-009 — query complexity and argument-count lint suppression is
  forbidden. Declare precise QueryData/QueryFilter and cohesive SystemParam
  access; blanket warnings/clippy suppressions cannot bypass this boundary.
* BEVY-BSN-006 — the mount seam must stay a seam: ``.insert(ChildOf(...))``
  (direct or tuple-first argument) is only legal inside the same statement as
  a ``spawn_scene`` call — the ``spawn_scene(scene).insert(ChildOf(slot))``
  chain. A standalone ``ChildOf`` insert reparents arbitrary entities without
  any scene; replace a bounded subtree instead. ``Query<&ChildOf>`` reads are
  unaffected.

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
outside the authoring contract. Violation codes 001-004 intentionally match
taskmanager's ``bevy_bsn_guard.py`` for cross-project greppability; 005 and
006 are music-frog extensions of the same family (taskmanager has no rule for
them yet).

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
from rust_syntax import mask_noncode, mask_test_items

SCAN_ROOTS = (
    "crates/infiltrator-bevy-widgets/src",
    "crates/infiltrator-bevy-ui/src",
)

BSN_START = re.compile(r"\bbsn!\s*\{")
UI_CONSTRUCTION = re.compile(
    r"\b(?:Node|Children|Text)\s*(?:\{|\[|\()"
    r"|\b(?:NodeBundle|TextBundle|ButtonBundle|ImageBundle)\b"
)
MANUAL_CHILD_API = re.compile(
    r"\.\s*(?:with_children|push_children|add_child|add_children|insert_children"
    r"|replace_children|add_related|insert_related)\s*(?:\(|::<)"
    r"|\b(?:ChildBuilder|ChildSpawner(?:Commands)?|RelatedSpawner(?:Commands)?)\b"
)
DIRECT_SPAWN = re.compile(r"\.\s*spawn(?:_empty(?:_at)?|_batch)?\s*\(")
# Spawn arguments that are infrastructure, not UI trees (see module docstring).
ALLOWED_SPAWN_ARGUMENTS = ("Camera2d", "Observer::new")
# `accesskit::Node::new` builds the accessibility tree, not UI structure — the
# fixed-width lookbehind exempts exactly that path prefix.
STRUCTURE_TEMP_CONSTRUCTOR = re.compile(
    r"(?<!accesskit::)\b(?:Node|Children|Text)::(?:new|default|EMPTY|from\w*)\b"
)
# Path-aware on purpose: `insert(ChildOf(..))` and the fully qualified
# `insert(bevy::ecs::hierarchy::ChildOf(..))` are the same bypass.
CHILDOF_MOUNT = re.compile(
    r"\.\s*insert\(\s*\(?\s*(?:\w+\s*::\s*)*ChildOf\b"
)


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


def mount_anchored_to_spawn_scene(masked: str, offset: int) -> bool:
    """True when a ``spawn_scene`` call precedes ``offset`` in its statement.

    The statement starts after the closest ``;`` or ``{``/``}`` before the
    insert — method chains spanning lines carry no ``;`` until the end.
    """
    boundaries = [masked.rfind(ch, 0, offset) for ch in ";{}"]
    statement_start = max(boundaries) + 1
    return "spawn_scene" in masked[statement_start:offset]


def balanced_end(masked: str, opening: int, left: str, right: str) -> int | None:
    depth = 0
    for index in range(opening, len(masked)):
        if masked[index] == left:
            depth += 1
        elif masked[index] == right:
            depth -= 1
            if depth == 0:
                return index + 1
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
                    "UI structure construction must be inside a bsn! Scene",
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

    for match in STRUCTURE_TEMP_CONSTRUCTOR.finditer(masked):
        if not inside_scene(match.start(), spans):
            violations.append(
                Violation(
                    rel_path,
                    line_number(original, match.start()),
                    "BEVY-BSN-005",
                    "structure temp-value constructor outside a bsn! Scene;"
                    " declare it in the scene or mutate fields in place",
                )
            )

    for match in CHILDOF_MOUNT.finditer(masked):
        if not mount_anchored_to_spawn_scene(masked, match.start()):
            violations.append(
                Violation(
                    rel_path,
                    line_number(original, match.start()),
                    "BEVY-BSN-006",
                    "ChildOf mount must chain from spawn_scene in the same"
                    " statement; replace a bounded subtree instead",
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
    runtime = mask_test_items(mask_noncode(text))
    for match in re.finditer(r"\b(?:World|DeferredWorld|EntityWorldMut|UnsafeWorldCell|SystemState)\b", runtime):
        violations.append(Violation(rel_path, line_number(original, match.start()), "BEVY-ECS-007",
            "unrestricted ECS access is forbidden in runtime code; declare Query/resources/events"))
    for match in re.finditer(r"(?:\.\s*|\bApp\s*::\s*)world(?:_mut)?\s*\(", runtime):
        violations.append(Violation(rel_path, line_number(original, match.start()), "BEVY-ECS-008",
            "App world access is forbidden in production; initialize through scoped systems"))
    suppression = re.compile(
        r"#\s*!?\s*\[\s*(?:allow|expect|cfg_attr)\b[^\]]*"
        r"(?:\bclippy\s*::\s*(?:type_complexity|too_many_arguments|all)\b|\bwarnings\b)"
    )
    for match in suppression.finditer(runtime):
        violations.append(Violation(rel_path, line_number(original, match.start()), "BEVY-ECS-009",
            "ECS access complexity cannot be suppressed; declare typed queries and scoped parameters"))
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

    expect("lifetime-bound World access is still forbidden", "fn business<'w>(world: &'w mut World) {}", ["BEVY-ECS-007"])
    expect("exclusive World system is forbidden", "fn business(world: &mut World) {}", ["BEVY-ECS-007"])
    expect("quoted comment cannot hide a World system", '// next "caption"\nfn business(world: &mut World) {}', ["BEVY-ECS-007"])
    expect("World field and query wrapper cannot disguise access", "struct Scope { world: World, state: SystemState<Res<X>> }", ["BEVY-ECS-007", "BEVY-ECS-007"])
    expect("test assembly is masked but subsequent runtime remains checked", "#[cfg(test)] fn fixture(world: &mut World) {} fn runtime(world: &World) {}", ["BEVY-ECS-007"])
    expect("restricted ECS parameters are allowed", "fn business(query: Query<&Fact>, state: Res<State>, commands: Commands) {}", [])
    expect("lifecycle observers declare restricted parameters", "fn hook(insert: On<Insert<Copy>>, query: Query<&Copy>, commands: Commands) {}", [])
    expect("business cannot substitute App for World", "fn business(app: &mut App) { app.world_mut().resource_mut::<State>(); }", ["BEVY-ECS-008"])
    expect("capture install cannot edit live world", "fn install(app: &mut App) { app.world().resource::<State>(); }", ["BEVY-ECS-008"])
    expect("a build name is not a Plugin boundary", "impl Service { fn build(app: &mut App) { app.world_mut(); } }", ["BEVY-ECS-008"])
    expect("plugin assembly cannot bypass scoped access", "impl Plugin for Skin { fn build(&self, app: &mut App) { app.world_mut().register_required_components::<A, B>(); } }", ["BEVY-ECS-008"])
    expect("framework registration uses App API", "impl Plugin for Skin { fn build(&self, app: &mut App) { app.register_required_components::<A, B>(); } }", [])
    expect("UFCS cannot acquire application World", "fn install(app: &mut App) { App::world_mut(app); }", ["BEVY-ECS-008"])
    expect("inner lint suppression is forbidden", "#![allow(clippy::type_complexity)] fn edit() {}", ["BEVY-ECS-009"])
    expect("query lint suppression is forbidden", "#[allow(clippy::type_complexity)] fn draw() {}", ["BEVY-ECS-009"])
    expect("argument lint expectation is still suppression", "#[expect(clippy::too_many_arguments)] fn edit() {}", ["BEVY-ECS-009"])
    expect("conditional suppression cannot bypass the boundary", "#[cfg_attr(feature = \"render\", allow(clippy::type_complexity))] fn edit() {}", ["BEVY-ECS-009"])
    expect("combined multiline suppression is forbidden", "#[allow(\nclippy::too_many_arguments,\nclippy::type_complexity\n)] fn edit() {}", ["BEVY-ECS-009"])
    expect("blanket warnings cannot hide access complexity", "#[allow(warnings)] fn edit() {}", ["BEVY-ECS-009"])
    expect("blanket clippy cannot hide access complexity", "#[allow(clippy::all)] fn edit() {}", ["BEVY-ECS-009"])
    expect("unrelated lint and literal text do not fake a boundary violation", "#[allow(non_upper_case_globals)] const x: &str = \"#[allow(clippy::type_complexity)]\";", [])
    expect("ordinary DeferredWorld input is forbidden", "fn business(world: DeferredWorld) {}", ["BEVY-ECS-007"])
    expect("DeferredWorld fields cannot hide access", "struct Service<'w> { access: DeferredWorld<'w> }", ["BEVY-ECS-007"])
    expect("a hook cannot forward its entire context", "fn hook(mut world: DeferredWorld, context: HookContext) { business(world); }", ["BEVY-ECS-007"])
    expect("a hook cannot borrow its entire context", "fn hook(mut world: DeferredWorld, context: HookContext) { business(&mut world); }", ["BEVY-ECS-007"])
    expect("a hook cannot obtain the whole context even for component access", "fn hook(mut world: DeferredWorld, context: HookContext) { let copy = world.get::<Copy>(context.entity); }", ["BEVY-ECS-007"])
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
        fn borrow<'a>(x: &'a str) -> &'a str { x }
        """,
        [],
    )
    expect(
        "accesskit::Node::new is the a11y tree, not UI structure",
        "fn a11y() { let mut n = accesskit::Node::new(accesskit::Role::Window); }",
        [],
    )
    expect(
        "..Default spreads inside a bsn! scene are fine",
        """
        fn scene(handle: Handle<Image>) -> impl Scene {
            bsn! { ImageNode { image: handle ..ImageNode::default() } }
        }
        """,
        [],
    )
    expect(
        "spawn_scene then ChildOf mount in one chain is the sanctioned seam",
        """
        fn mount(mut commands: Commands, scene: impl Scene) {
            commands.spawn_scene(scene).insert(ChildOf(slot));
        }
        """,
        [],
    )
    expect(
        "tuple-form ChildOf mount in the spawn_scene chain is sanctioned",
        "fn m(mut c: Commands) { c.spawn_scene(s()).insert((ChildOf(slot), Marker)); }",
        [],
    )
    expect(
        "ChildOf reads and removal routes are allowed",
        """
        fn reads(q: Query<&ChildOf>) {}
        fn teardown(mut commands: Commands, e: Entity, child: Entity) {
            commands.entity(e).despawn_children();
            commands.entity(e).remove_child(child);
            commands.entity(e).remove_children(&[]);
        }
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
        "full hierarchy API surface is banned (turbofish included)",
        """
        commands.entity(root).insert_children(0, &rows);
        commands.entity(root).replace_children(&rows);
        commands.entity(root).add_related::<ChildOf>(&rows);
        commands.entity(root).insert_related::<ChildOf>(0, &rows);
        """,
        ["BEVY-BSN-002"] * 4,
    )
    expect(
        "spawner types declared anywhere are banned",
        """
        fn a(mut spawner: RelatedSpawnerCommands<ChildOf>) {}
        fn b(mut spawner: ChildSpawner) {}
        """,
        ["BEVY-BSN-002"] * 2,
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
        "spawn_empty / spawn_empty_at are banned",
        """
        commands.spawn_empty();
        world.spawn_empty_at(entity);
        """,
        ["BEVY-BSN-003"] * 2,
    )
    expect(
        "structure temp-value constructors outside bsn! are banned",
        """
        let a = Node::default();
        let b = Node::EMPTY;
        let c = Text::new("hi");
        let d = Children::from_entity(e);
        let f = bevy::ui::Node::EMPTY;
        """,
        ["BEVY-BSN-005"] * 5,
    )
    expect(
        "standalone ChildOf insert (reparent) is banned",
        "commands.entity(e).insert(ChildOf(parent));",
        ["BEVY-BSN-006"],
    )
    expect(
        "fully qualified ChildOf insert is the same bypass",
        "commands.entity(e).insert(bevy::ecs::hierarchy::ChildOf(parent));",
        ["BEVY-BSN-006"],
    )
    expect(
        "ChildOf insert after an unrelated statement is banned (same-block escape)",
        """
        commands.spawn_scene(scene);
        commands.entity(e).insert(ChildOf(parent));
        """,
        ["BEVY-BSN-006"],
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
