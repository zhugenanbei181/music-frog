#!/usr/bin/env python3
"""Bevy UI rem-channel guard for the product's own sizing law (BEVY-REM-001..004).

The capability spec (`crates/infiltrator-bevy-ui/docs/BEVY_020_UI_CAPABILITIES.md`
①) makes the root font size (`RemSize`) the **only** scaling entry of the whole
UI: dimensions travel through the rem channel (`rem(...)`), type travels through
`FontSize::Rem`, and physical pixels are a closed whitelist (hairlines, the
caret, viewport breakpoints, the minimum touch target, host inset physical
pixels, and GPU/measurement modules).

The law has two halves with two owners, and they compose into the closed
whitelist without two baselines for the same sites:

* the **shared-kit scene gate** (`zhuge-ui-kit`'s `check_bevy_ui.py`, vendored as
  `scripts/quality/bevy_scene_gate.py`, rule `UIK-REM-001/002`) is the upstream
  lower bound for the same law — hardcoded px literals and `FontSize::Px`;
* this guard is the **product's blocking authority** and covers the whole law
  (hardcoded literals, named and derived px values, `em` and type), so the
  product never depends on a vendored copy being in place to be enforced.

Rules:

* **BEVY-REM-001** — a `Val::Px` dimension (literal, named or derived) that is
  neither on the physical whitelist nor registered in the legacy debt
  inventory. A new px dimension is a node the user font scale cannot reach.
* **BEVY-REM-002** — a font size that is not `FontSize::Rem` (`Px`, `Vw`, `Vh`,
  `VMin`, `VMax`). The root font size is the single type-scaling entry.
* **BEVY-REM-003** — a `Val::Em` dimension written as a bare literal. `em` is
  relative to the surrounding text size, so it must derive from a measured or
  named value, never from a magic number.
* **BEVY-REM-004** — the legacy debt inventory is missing or references a file
  that no longer exists. The inventory is an exact inventory, not an allowlist.
* **BEVY-REM-005** — a rem size written as a bare number (`rem(560.0)`). Every
  size must be a *named* quantity: a shared token or a named constant, never a
  magic number at the call site.

The legacy debt inventory (`scripts/quality/bevy_rem_inventory.json`) records
every named px site that predates this guard. It is seeded once, may only shrink
(`--prune-inventory` refuses to register new debt), and every unregistered site
is a violation — so the debt cannot grow.

Usage:
    python3 scripts/quality/bevy_rem_guard.py [--mode report|enforce] [--root PATH]
    python3 scripts/quality/bevy_rem_guard.py --prune-inventory
    python3 scripts/quality/bevy_rem_guard.py --self-test

Exit status is 0 when no violations are found (or in report mode), 1 when
violations exist in enforce mode.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
import re
import sys
import tempfile

from rust_syntax import mask_noncode

ROOTS = ("crates/infiltrator-bevy-widgets/src", "crates/infiltrator-bevy-ui/src")
INVENTORY = Path("scripts/quality/bevy_rem_inventory.json")

#: The capability spec's px whitelist: hairlines, the caret, viewport
#: breakpoints, the minimum touch target, host inset physical pixels and the
#: measured/physical quantities the product keeps in pixels on purpose (raster
#: plates, virtual-list row metrics, measured spacers).
PHYSICAL_WHITELIST = re.compile(
    r"HAIRLINE"
    r"|CARET_WIDTH"
    r"|hairline_px"
    r"|caret_width_px"
    r"|MIN_TOUCH_TARGET"
    r"|TOUCH_TARGET"
    r"|COMPACT_MAX_PX"
    r"|MEDIUM_MAX_PX"
    r"|EXPANDED_MAX_PX"
    r"|breakpoint"
    r"|inset"
    r"|safe_area"
    r"|raster_"
    r"|HEIGHT_PX"
    r"|spacer_px"
    r"|item_width_px"
    r"|display_",
    re.IGNORECASE,
)

#: GPU and measurement modules: sizes there are measured values, not tokens.
MEASUREMENT_SOURCES = (
    "crates/infiltrator-bevy-widgets/src/chart.rs",
    "crates/infiltrator-bevy-widgets/src/chart/",
    "crates/infiltrator-bevy-widgets/src/responsive.rs",
    "crates/infiltrator-bevy-widgets/src/shader_fx.rs",
    "crates/infiltrator-bevy-widgets/src/telemetry_gpu.rs",
    "crates/infiltrator-bevy-ui/src/host_insets.rs",
)

#: A font size that is not relative to the root font size.
NON_REM_FONT_SIZE = re.compile(r"\bFontSize::(?:Px|Vw|Vh|VMin|VMax)\b")

#: `em` is text-relative; a bare literal cannot be.
EM_LITERAL = re.compile(r"\bVal::Em\s*\(\s*[-+]?\d+(?:\.\d+)?\s*[,)]")

#: A rem size must be a *named* quantity: a shared token or a named constant.
#: A bare number inside `rem(...)` is an unnamed size (总纲③).
REM_LITERAL = re.compile(r"\brem\s*\(\s*(?P<value>[-+]?\d+(?:\.\d+)?)\s*[,)]")

PX_SIZE = re.compile(r"\bVal::Px\s*\(")

#: A hardcoded px literal belongs to the shared-kit scene gate (`UIK-REM-001`).
PX_LITERAL = re.compile(r"[-+]?\d+(?:\.\d+)?")


@dataclass(frozen=True)
class Violation:
    path: str
    line: int
    code: str
    message: str


@dataclass(frozen=True)
class Notice:
    path: str
    code: str
    message: str


def is_excluded(path: Path) -> bool:
    """Dedicated test trees and `#[path]`-mounted test modules sit outside the
    production authoring contract (same exclusion shape as the bsn guard)."""
    return (
        "target" in path.parts
        or "tests" in path.parts
        or path.stem.endswith(("_test", "_tests"))
    )


def rust_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for relative in ROOTS:
        source = root / relative
        if not source.is_dir():
            continue
        files.extend(
            path
            for path in sorted(source.rglob("*.rs"))
            if path.is_file() and not is_excluded(path.relative_to(root))
        )
    return files


def matching_paren(masked: str, opening: int) -> int:
    depth = 0
    for index in range(opening, len(masked)):
        if masked[index] == "(":
            depth += 1
        elif masked[index] == ")":
            depth -= 1
            if depth == 0:
                return index
    return len(masked)


def normalize(argument: str) -> str:
    """Whitespace-insensitive form of a size expression (the debt key)."""
    text = " ".join(argument.split())
    return text[:-1].rstrip() if text.endswith(",") else text


def is_zero(argument: str) -> bool:
    """A zero dimension has no size to scale, so the rem channel cannot miss it."""
    try:
        return float(argument) == 0.0
    except ValueError:
        return False


def is_literal(argument: str) -> bool:
    """A hardcoded px literal (the upstream law's lower bound, also a violation)."""
    return bool(PX_LITERAL.fullmatch(argument))


def px_sites(text: str) -> list[tuple[int, str]]:
    """`(offset, argument)` of every `Val::Px(...)` size in production code."""
    masked = mask_noncode(text)
    sites: list[tuple[int, str]] = []
    for match in PX_SIZE.finditer(masked):
        opening = masked.index("(", match.start(), match.end())
        closing = matching_paren(masked, opening)
        sites.append((match.start(), normalize(text[opening + 1 : closing])))
    return sites


def font_size_sites(text: str) -> list[tuple[int, str]]:
    masked = mask_noncode(text)
    return [
        (match.start(), match.group(0))
        for match in NON_REM_FONT_SIZE.finditer(masked)
    ]


def em_literal_sites(text: str) -> list[int]:
    masked = mask_noncode(text)
    return [match.start() for match in EM_LITERAL.finditer(masked)]


def rem_literal_sites(text: str) -> list[tuple[int, str]]:
    """`rem(<number>)` call sites: an unnamed size."""
    masked = mask_noncode(text)
    return [
        (match.start(), match.group("value"))
        for match in REM_LITERAL.finditer(masked)
    ]


def is_measurement_source(relative: str) -> bool:
    return any(
        relative == entry or relative.startswith(entry)
        for entry in MEASUREMENT_SOURCES
    )


def is_physical(relative: str, argument: str) -> bool:
    """True when the size is one of the physical quantities the spec allows."""
    return (
        is_measurement_source(relative)
        or is_zero(argument)
        or bool(PHYSICAL_WHITELIST.search(argument))
    )


def read_inventory(root: Path) -> tuple[dict[str, dict[str, int]], list[str]]:
    """The registered legacy debt, keyed by file and size expression."""
    path = root / INVENTORY
    if not path.is_file():
        return {}, [f"{INVENTORY}: the legacy debt inventory is missing"]
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return {}, [f"{INVENTORY}: cannot read the legacy debt inventory: {error}"]
    if not isinstance(document, dict) or document.get("version") != 1:
        return {}, [f"{INVENTORY}: expected an inventory object with version 1"]
    debt = document.get("debt")
    if not isinstance(debt, dict):
        return {}, [f"{INVENTORY}: `debt` must be an object"]
    inventory: dict[str, dict[str, int]] = {}
    problems: list[str] = []
    for name, values in debt.items():
        if not isinstance(values, dict):
            problems.append(f"{INVENTORY}: debt[{name!r}] must be an object")
            continue
        entry: dict[str, int] = {}
        for argument, count in values.items():
            if not isinstance(count, int) or count < 1:
                problems.append(
                    f"{INVENTORY}: debt[{name!r}][{argument!r}] must be a positive count"
                )
                continue
            entry[argument] = count
        inventory[name] = entry
    return inventory, problems


def collect_debt(root: Path) -> dict[str, dict[str, int]]:
    """The named px sites the tree currently writes outside the whitelist."""
    debt: dict[str, dict[str, int]] = {}
    for path in rust_files(root):
        relative = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8", errors="replace")
        for _, argument in px_sites(text):
            if is_physical(relative, argument):
                continue
            values = debt.setdefault(relative, {})
            values[argument] = values.get(argument, 0) + 1
    return debt


def scan(root: Path) -> tuple[list[Violation], list[Notice]]:
    """Violations and notices for the production trees of the two Bevy crates."""
    violations: list[Violation] = []
    notices: list[Notice] = []
    inventory, problems = read_inventory(root)
    for problem in problems:
        violations.append(Violation(INVENTORY.as_posix(), 0, "BEVY-REM-004", problem))

    remaining: dict[str, dict[str, int]] = {
        name: dict(values) for name, values in inventory.items()
    }

    for path in rust_files(root):
        relative = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8", errors="replace")
        for offset, variant in font_size_sites(text):
            violations.append(
                Violation(
                    relative,
                    text.count("\n", 0, offset) + 1,
                    "BEVY-REM-002",
                    f"{variant} ignores the root font size; use FontSize::Rem",
                )
            )
        for offset in em_literal_sites(text):
            violations.append(
                Violation(
                    relative,
                    text.count("\n", 0, offset) + 1,
                    "BEVY-REM-003",
                    "Val::Em must derive from a measured or named value,"
                    " not from a bare literal",
                )
            )
        for offset, value in rem_literal_sites(text):
            violations.append(
                Violation(
                    relative,
                    text.count("\n", 0, offset) + 1,
                    "BEVY-REM-005",
                    f"rem({value}) is an unnamed size; use a shared token"
                    " (space::/radius::/metrics::) or a named constant",
                )
            )
        for offset, argument in px_sites(text):
            if is_physical(relative, argument):
                continue
            values = remaining.get(relative, {})
            left = values.get(argument, 0)
            if left > 0:
                values[argument] = left - 1
                continue
            kind = "a hardcoded px literal" if is_literal(argument) else "a named px value"
            violations.append(
                Violation(
                    relative,
                    text.count("\n", 0, offset) + 1,
                    "BEVY-REM-001",
                    f"Val::Px({argument}) is {kind} outside the physical whitelist"
                    " and not registered legacy debt; use rem(...)",
                )
            )

    for name, values in sorted(remaining.items()):
        if not (root / name).is_file():
            violations.append(
                Violation(
                    name,
                    0,
                    "BEVY-REM-004",
                    "the debt inventory references a file that no longer exists",
                )
            )
            continue
        for argument, left in sorted(values.items()):
            if left > 0:
                notices.append(
                    Notice(
                        name,
                        "BEVY-REM-DRIFT",
                        f"Val::Px({argument}) is registered {left} time(s) but the"
                        " tree no longer writes it; prune the inventory",
                    )
                )
    return violations, notices


def write_inventory(root: Path, debt: dict[str, dict[str, int]]) -> None:
    path = root / INVENTORY
    path.parent.mkdir(parents=True, exist_ok=True)
    document = {
        "version": 1,
        "debt": {
            name: dict(sorted(values.items()))
            for name, values in sorted(debt.items())
        },
    }
    path.write_text(
        json.dumps(document, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def prune_inventory(root: Path) -> int:
    """Refresh the registered debt to the current tree; never register new debt."""
    inventory, problems = read_inventory(root)
    if problems:
        for problem in problems:
            print(problem, file=sys.stderr)
        return 1
    current = collect_debt(root)
    added = [
        (name, argument)
        for name, values in current.items()
        for argument in values
        if argument not in inventory.get(name, {})
    ]
    if added:
        for name, argument in sorted(added):
            print(
                f"NOT PRUNED: {name}: Val::Px({argument}) is new debt;"
                " migrate it to the rem channel instead of registering it",
                file=sys.stderr,
            )
        return 1
    write_inventory(root, current)
    total = sum(sum(values.values()) for values in current.values())
    print(f"bevy rem guard: inventory pruned, {total} registered legacy px site(s)")
    return 0


def seed_inventory(root: Path) -> int:
    """One-time inventory of the legacy named px sites (refuses to re-seed)."""
    path = root / INVENTORY
    if path.is_file():
        print(
            f"{INVENTORY}: the inventory already exists; use --prune-inventory",
            file=sys.stderr,
        )
        return 1
    debt = collect_debt(root)
    write_inventory(root, debt)
    total = sum(sum(values.values()) for values in debt.values())
    print(f"bevy rem guard: inventory seeded, {total} legacy px site(s)")
    return 0


def self_test() -> int:
    """Positive and negative cases inline, plus the real production trees."""
    ok = True

    def expect(label: str, text: str, codes: list[str]) -> None:
        nonlocal ok
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / ROOTS[0]
            source.mkdir(parents=True)
            (source / "sample.rs").write_text(text, encoding="utf-8")
            inventory = root / INVENTORY
            inventory.parent.mkdir(parents=True, exist_ok=True)
            inventory.write_text(
                json.dumps({"version": 1, "debt": {}}) + "\n", encoding="utf-8"
            )
            violations, _ = scan(root)
        got = sorted(v.code for v in violations)
        want = sorted(codes)
        if got == want:
            print(f"  [ok] {label}")
        else:
            ok = False
            print(f"  [FAIL] {label}: expected {want}, found {got}")

    expect(
        "the rem channel is the sanctioned route",
        """
        fn scene(palette: &UiPalette) -> impl Scene {
            bsn! {
                Node { width: rem(space::S8) height: Val::Rem(1.5) }
                TextFont { font_size: FontSize::Rem(1.0) }
            }
        }
        """,
        [],
    )
    expect(
        "a physical whitelist size is allowed",
        """
        fn hairline(palette: &UiPalette) {
            let border = Val::Px(palette.hairline_px);
            let caret = Val::Px(metrics::CARET_WIDTH);
            let touch = Val::Px(metrics::MIN_TOUCH_TARGET);
            let breakpoint = Val::Px(breakpoint::COMPACT_MAX_PX);
            let inset = Val::Px(insets_left);
        }
        """,
        [],
    )
    expect(
        "a named px dimension outside the whitelist is rejected",
        """
        fn card(gap: f32) {
            let radius = Val::Px(palette.control_radius_px);
            let spacing = Val::Px(gap);
        }
        """,
        ["BEVY-REM-001", "BEVY-REM-001"],
    )
    expect(
        "a hardcoded px literal is forbidden as well",
        "fn card() { let radius = Val::Px(4.0); }",
        ["BEVY-REM-001"],
    )
    expect(
        "a non-rem font size is rejected",
        """
        fn type_scale() {
            let size = FontSize::Px(15.0);
            let viewport = FontSize::Vw(2.0);
        }
        """,
        ["BEVY-REM-002", "BEVY-REM-002"],
    )
    expect(
        "a literal em size is rejected",
        "fn inline_icon() { let size = Val::Em(1.5); }",
        ["BEVY-REM-003"],
    )
    expect(
        "an unnamed rem size is rejected",
        "fn panel() { let width = rem(560.0); }",
        ["BEVY-REM-005"],
    )
    expect(
        "a named rem size is allowed",
        "fn panel() { let width = rem(space::S12); let other = rem(PANEL_WIDTH_PX); }",
        [],
    )
    expect(
        "comments and string literals cannot hide or fake a px site",
        """
        // Val::Px(palette.control_radius_px)
        /// let radius = Val::Px(6.0);
        let doc = "Val::Px(8.0)";
        """,
        [],
    )
    expect(
        "a qualified px path is the same bypass",
        "fn card() { let radius = bevy::ui::Val::Px(palette.control_radius_px); }",
        ["BEVY-REM-001"],
    )

    repo_root = Path(__file__).resolve().parents[2]
    violations, notices = scan(repo_root)
    if violations:
        ok = False
        for violation in violations:
            print(
                f"  [FAIL] real file violates: {violation.path}:{violation.line}:"
                f" {violation.code}: {violation.message}"
            )
    else:
        print(f"  [ok] real production files pass ({len(notices)} drift notice(s))")

    print("self-test:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[2],
        help="repository root that holds the two Bevy crates",
    )
    parser.add_argument(
        "--prune-inventory",
        action="store_true",
        help="refresh the registered legacy debt to the current tree",
    )
    parser.add_argument(
        "--seed-inventory",
        action="store_true",
        help="one-time inventory of the legacy named px sites",
    )
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    root = arguments.root
    if not root.is_absolute():
        root = Path(__file__).resolve().parents[2] / root
    if arguments.self_test:
        return self_test()
    if arguments.seed_inventory:
        return seed_inventory(root)
    if arguments.prune_inventory:
        return prune_inventory(root)

    violations, notices = scan(root)
    for violation in violations:
        print(
            f"{violation.path}:{violation.line}: {violation.code}: {violation.message}",
            file=sys.stderr,
        )
    for notice in notices:
        print(f"NOTICE: {notice.path}: {notice.code}: {notice.message}")
    print(
        f"bevy rem guard: violations={len(violations)} drift_notices={len(notices)}"
    )
    return int(bool(violations) and arguments.mode == "enforce")


if __name__ == "__main__":
    raise SystemExit(main())
