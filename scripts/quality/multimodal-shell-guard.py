#!/usr/bin/env python3
"""Fail-closed guard for the group 15 multimodal-shell ledger (DUAL-15-01..15).

Group 15's closure standard matches groups 06/12/14: one shared
contract/application source, both surfaces, dual headless tests, and an honest
per-item ledger. This guard asserts the per-item ledger rows exist, that the
shared theme/shortcut/notification contracts and both surface wirings stay
present, that the widget-layer skin mirror cannot drift from the contract
vocabulary, and that the fixed defects do not regress (hard-coded two-way
theme flip, local hotkey list, index-based toast removal, zero-capacity toast
queue).
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]

LEDGER = "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md"
RESPONSIVE_LEDGER = "docs/RESPONSIVE_PARITY_LEDGER.md"
MATRIX = "docs/MULTIMODAL_SHELL_MATRIX.md"
DESIGN_TOKENS = "crates/infiltrator-contract/src/design_tokens.rs"
ICED_THEME = "crates/infiltrator-iced/src/view/theme.rs"
BEVY_THEME = "crates/infiltrator-bevy-widgets/src/theme.rs"

#: Contract core-palette field -> the Bevy widget mirror's field name.
BEVY_CORE_FIELDS = {
    "canvas": "window_bg",
    "sidebar": "sidebar",
    "card": "surface",
    "card_border": "border",
    "control_bg": "surface_elevated",
    "ink": "ink",
    "ink_dim": "ink_dim",
    "accent": "accent",
    "on_accent": "on_accent",
    "success": "success",
    "warning": "warning",
    "danger": "danger",
}

#: Contract core-palette field -> the Iced shell token field it resolves into.
ICED_CORE_FIELDS = {
    "canvas": "canvas",
    "sidebar": "sidebar",
    "card": "card_bg",
    "card_border": "card_border",
    "control_bg": "control_bg",
    "ink": "text_primary",
    "ink_dim": "text_secondary",
    "accent": "accent",
    "on_accent": "on_accent",
    "success": "success",
    "warning": "warning",
    "danger": "danger",
}

SKINS = ("Dark", "Light", "Forest", "Amoled")

#: Contract interaction-palette field -> (Iced token field, Bevy mirror field).
#: DUAL-15-14: these are the programmable interaction surfaces both toolkits
#: expose; the focus ring maps onto the Iced input border and `disabled_ink`
#: onto the Iced tertiary ink (the two surfaces use different field names for
#: the same shared value).
INTERACTION_FIELDS = {
    "scrim": ("scrim", "scrim"),
    "hover": ("hover", "hover"),
    "pressed": ("pressed", "pressed"),
    "focus_ring": ("focus_ring", "focus_ring"),
    "disabled_ink": ("text_tertiary", "disabled_ink"),
}

#: Shared shell rows the Iced surface can honestly present only as a visible
#: localized tooltip (icon-only or colour-only affordances, or readouts whose
#: meaning is a value). Each row must be referenced by a real `labelled(...)`
#: call in the listed file: removing the tooltip fails the gate instead of
#: silently dropping the shared label (DUAL-15-10).
ICED_TOOLTIP_ROWS = {
    "GlobalStatusDot": "crates/infiltrator-iced/src/view/sidebar.rs",
    "TrafficReadout": "crates/infiltrator-iced/src/view/sidebar.rs",
    "SystemProxySwitch": "crates/infiltrator-iced/src/view/sidebar.rs",
    "TunSwitch": "crates/infiltrator-iced/src/view/sidebar.rs",
    "MiniHudCard": "crates/infiltrator-iced/src/view/mini_hud.rs",
    "MiniHudSystemProxySwitch": "crates/infiltrator-iced/src/view/mini_hud.rs",
    "MiniHudTunSwitch": "crates/infiltrator-iced/src/view/mini_hud.rs",
    "ChromeMinimize": "crates/infiltrator-iced/src/view/chrome.rs",
    "ChromeMaximize": "crates/infiltrator-iced/src/view/chrome.rs",
    "ChromeClose": "crates/infiltrator-iced/src/view/chrome.rs",
    "CommandPaletteQuery": "crates/infiltrator-iced/src/view_root/command_palette.rs",
}

#: Shared shell rows the Iced surface presents as visible text already (nav
#: labels, the dialog content, toast copy), so a tooltip would be redundant.
ICED_VISIBLE_TEXT_ROWS = {
    "ShellHeader",
    "ContentRegion",
    "SidebarNav",
    "ModeSegment",
    "ToastRegion",
    "CommandPaletteDialog",
}

#: Shared shell rows Iced genuinely has no control for: the OS window itself,
#: and the header theme toggle (Iced changes the appearance from the settings
#: page and the command palette, so there is no icon to attach a label to).
#: Classifying them as absent is the honest statement, not a missing tooltip.
ICED_ABSENT_ROWS = {
    "Window",
    "ThemeToggle",
}

#: The Iced overlay backdrops that must paint the shared scrim token.
ICED_SCRIM_BACKDROPS = (
    "crates/infiltrator-iced/src/view_root/command_palette.rs",
    "crates/infiltrator-iced/src/view_root/connection_drawer.rs",
    "crates/infiltrator-iced/src/view_root/modals/card.rs",
)

#: The Bevy scenes that must paint the shared scrim token.
BEVY_SCRIM_SCENES = (
    "crates/infiltrator-bevy-widgets/src/adaptive_modal.rs",
    "crates/infiltrator-bevy-widgets/src/drawer.rs",
    "crates/infiltrator-bevy-widgets/src/menu.rs",
    "crates/infiltrator-bevy-widgets/src/modal.rs",
    "crates/infiltrator-bevy-widgets/src/popover.rs",
    "crates/infiltrator-bevy-ui/src/command_palette.rs",
)


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    compact_text = " ".join(text.split())
    for marker in markers:
        compact_marker = " ".join(marker.split())
        rustfmt_marker = compact_marker.replace(" }", ", }")
        if (
            marker not in text
            and compact_marker not in compact_text
            and rustfmt_marker not in compact_text
        ):
            violations.append(f"{path} missing {marker!r}")


def forbid(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    for marker in markers:
        if marker in text:
            violations.append(f"{path} still contains forbidden marker {marker!r}")


def skin_settings(path: str) -> set[str]:
    """Extract the quoted skin setting names from one `as_setting` arm."""
    text = read(path)
    match = re.search(
        r"pub const fn as_setting\(self\)[^{]*\{(?P<body>.*?)\n    \}",
        text,
        re.DOTALL,
    )
    if not match:
        return set()
    return set(re.findall(r'=>\s*"([a-z]+)"', match.group("body")))


def color_fields(body: str) -> dict[str, tuple[float, ...]]:
    """Extract `name: Ctor(0.1, 0.2, 0.3[, 0.4])` token tuples."""
    fields: dict[str, tuple[float, ...]] = {}
    for match in re.finditer(
        r"(\w+):\s*\w+::rgba?\(([^)]*)\)", body, re.DOTALL
    ):
        fields[match.group(1)] = tuple(
            float(value.strip()) for value in match.group(2).split(",")
        )
    return fields


def contract_cores() -> dict[str, dict[str, tuple[float, ...]]]:
    """The authoritative per-skin core palette from the shared contract."""
    text = read(DESIGN_TOKENS)
    cores: dict[str, dict[str, tuple[float, ...]]] = {}
    for skin in SKINS:
        match = re.search(
            rf"ThemeSkin::{skin} => SkinCorePalette \{{(?P<body>.*?)\n        \}},",
            text,
            re.DOTALL,
        )
        if match:
            cores[skin] = color_fields(match.group("body"))
    return cores


def bevy_cores() -> dict[str, dict[str, tuple[float, ...]]]:
    """The mirrored per-skin palettes from the Bevy widget layer."""
    text = read(BEVY_THEME)
    cores: dict[str, dict[str, tuple[float, ...]]] = {}
    for skin in SKINS:
        match = re.search(
            rf"pub fn {skin.lower()}\(\) -> Self \{{(?P<body>.*?)\n    \}}",
            text,
            re.DOTALL,
        )
        if match:
            cores[skin] = color_fields(match.group("body"))
    return cores


def contract_interactions() -> dict[str, dict[str, tuple[float, ...]]]:
    """The authoritative per-skin interaction palette from the contract."""
    text = read(DESIGN_TOKENS)
    interactions: dict[str, dict[str, tuple[float, ...]]] = {}
    for skin in SKINS:
        match = re.search(
            rf"ThemeSkin::{skin} => SkinInteractionPalette \{{(?P<body>.*?)\n        \}},",
            text,
            re.DOTALL,
        )
        if match:
            interactions[skin] = color_fields(match.group("body"))
    return interactions


def number_const(text: str, name: str) -> float | None:
    match = re.search(
        rf"pub const {name}\s*:\s*f32\s*=\s*([0-9]+(?:\.[0-9]+)?)\s*;", text
    )
    return float(match.group(1)) if match else None


def ignored_test_reason(source: str, name: str) -> str | None:
    """Why a cited test could not fail: ignored, or only mentioned in comments.

    A `path::test_name` token is evidence only if the test really runs; an
    `#[ignore]`-annotated (or commented-out) function would make the matrix
    cite a test that can never fail the suite.
    """
    declaration = re.compile(rf"\bfn\s+{re.escape(name)}\b")
    found = False
    lines = source.splitlines()
    for index, line in enumerate(lines):
        if not declaration.search(line):
            continue
        if line.strip().startswith("//"):
            continue
        found = True
        for previous in lines[max(0, index - 4) : index]:
            if previous.strip().startswith("#[ignore"):
                return "annotated #[ignore]"
            if previous.strip().startswith("//"):
                continue
    return None if found else "only mentioned in comments"


def check_matrix(violations: list[str]) -> None:
    """Verify the group 15 regression matrix against the real test sources.

    This is what makes `DUAL-15-15` a machine-checkable claim instead of a doc
    promise: every `path::test_name` evidence token must point at an existing
    file that really contains that test, closed items must carry evidence from
    both surfaces plus the shared/application source, planned items must not
    claim any, and every matrix status must equal the authoritative per-item
    status in the master plan.
    """
    text = read(MATRIX)
    evidence = re.compile(r"`(crates/[^`\s]+?\.rs)::([A-Za-z0-9_]+)`")
    rows: dict[str, str] = {}
    for line in text.splitlines():
        if not line.startswith("| `DUAL-15-"):
            continue
        item = re.match(r"\| `(DUAL-15-\d+)` \|", line)
        if not item:
            continue
        rows[item.group(1)] = line

    ledger = ledger_statuses()
    shared_roots = (
        "crates/infiltrator-contract/",
        "crates/infiltrator-application/",
        "crates/infiltrator-ports/",
        "crates/infiltrator-domain/",
        "crates/infiltrator-desktop/",
    )

    expected = [f"DUAL-15-{index:02d}" for index in range(1, 16)]
    for item in expected:
        if item not in rows:
            violations.append(f"{MATRIX} missing matrix row for {item}")
            continue
        line = rows[item]
        status_match = re.match(r"\| `DUAL-15-\d+` \| (?P<status>[^|]+)\|", line)
        status_cell = status_match.group("status").strip() if status_match else ""
        status = next(
            (
                word
                for word in ("parity-ready", "shared-ready", "planned")
                if word in status_cell
            ),
            None,
        )
        if status is None:
            violations.append(f"{MATRIX} {item} has no known status: {status_cell!r}")
        ledger_status = ledger.get(item)
        if ledger_status is None:
            violations.append(f"{LEDGER} has no group 15 item row for {item}")
        elif status is not None and ledger_status != status:
            violations.append(
                f"{MATRIX} {item} says {status!r} but {LEDGER} says {ledger_status!r}"
            )
        tokens = evidence.findall(line)
        if len(tokens) != len(set(tokens)):
            violations.append(f"{MATRIX} {item} cites the same evidence twice: {tokens}")
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) >= 6 and not cells[5]:
            violations.append(
                f"{MATRIX} {item} must state its honest deviation / blocker"
            )
        for path, name in tokens:
            try:
                source = read(path)
            except FileNotFoundError:
                violations.append(f"{MATRIX} {item} cites missing file {path}")
                continue
            if name not in source:
                violations.append(
                    f"{MATRIX} {item} cites {path}::{name} but the test is absent"
                )
                continue
            ignored = ignored_test_reason(source, name)
            if ignored is not None:
                violations.append(
                    f"{MATRIX} {item} cites {path}::{name}, but the cited test is "
                    f"{ignored} (evidence must be able to fail)"
                )
        if status == "planned":
            if tokens:
                violations.append(
                    f"{MATRIX} {item} is planned but claims evidence {tokens}"
                )
            continue
        if "外链" in status_cell:
            continue
        surfaces = {
            "iced": any("/infiltrator-iced/" in path for path, _ in tokens),
            "bevy": any("/infiltrator-bevy-ui/" in path for path, _ in tokens),
        }
        if not all(surfaces.values()):
            violations.append(
                f"{MATRIX} {item} is {status_cell} but lacks dual-surface evidence: {surfaces}"
            )
        shared_evidence = [
            (path, name)
            for path, name in tokens
            if path.startswith(shared_roots)
        ]
        if not shared_evidence:
            violations.append(
                f"{MATRIX} {item} claims {status!r} without shared/application evidence"
            )
        if len(tokens) < 3:
            violations.append(
                f"{MATRIX} {item} claims {status!r} with only {len(tokens)} evidence "
                "tokens; shared + Iced + Bevy are the minimum"
            )


def ledger_statuses() -> dict[str, str]:
    """The authoritative per-item group 15 statuses from the master plan.

    The first `DUAL-15-XX | 任务 | 状态 | 证据` row of an item wins, so the
    per-item ledger table stays authoritative over any later batch notes.
    """
    text = read(LEDGER)
    statuses: dict[str, str] = {}
    for line in text.splitlines():
        match = re.match(
            r"\| `(DUAL-15-\d+)` \| [^|]+ \| (?P<status>[^|]+) \|", line
        )
        if not match:
            continue
        item = match.group(1)
        if item in statuses:
            continue
        status = next(
            (
                word
                for word in ("parity-ready", "shared-ready", "planned")
                if word in match.group("status")
            ),
            None,
        )
        if status is not None:
            statuses[item] = status
    return statuses


def check_design_token_mirrors(violations: list[str]) -> None:
    """Numeric drift check across the shared contract, Iced and Bevy.

    This is the real 15-14 gate: the same numbers must appear in the contract
    (authoritative), be consumed by the Iced token module by name, and be
    mirrored channel-exactly by the business-agnostic Bevy widget layer.
    """
    cores = contract_cores()
    if set(cores) != set(SKINS):
        violations.append(
            f"{DESIGN_TOKENS} must define skin_core for every skin, found {sorted(cores)}"
        )
        return
    for skin, fields in cores.items():
        missing = set(BEVY_CORE_FIELDS) - set(fields)
        if missing:
            violations.append(
                f"{DESIGN_TOKENS} {skin} core palette missing {sorted(missing)}"
            )

    mirror = bevy_cores()
    if set(mirror) != set(SKINS):
        violations.append(
            f"{BEVY_THEME} must define all four skins, found {sorted(mirror)}"
        )
        return
    for skin in SKINS:
        for field, bevy_field in BEVY_CORE_FIELDS.items():
            expected = cores[skin].get(field)
            actual = mirror[skin].get(bevy_field)
            if expected is None:
                continue
            if actual is None:
                violations.append(
                    f"{BEVY_THEME} {skin} missing token {bevy_field!r}"
                )
                continue
            if len(expected) != len(actual) or any(
                abs(left - right) > 1e-6 for left, right in zip(expected, actual)
            ):
                violations.append(
                    f"{BEVY_THEME} {skin}.{bevy_field}={actual} must mirror "
                    f"contract {field}={expected}"
                )

    iced_text = read(ICED_THEME)
    for skin in SKINS:
        core_name = f"{skin.upper()}_CORE"
        if f"skin_core(ThemeSkin::{skin})" not in iced_text:
            violations.append(
                f"{ICED_THEME} must resolve {skin} from the shared skin_core"
            )
            continue
        block = re.search(
            rf"pub const {skin.upper()}: Tokens = Tokens \{{(?P<body>.*?)\n\}};",
            iced_text,
            re.DOTALL,
        )
        if not block:
            violations.append(f"{ICED_THEME} missing the {skin.upper()} token block")
            continue
        body = block.group("body")
        for field, iced_field in ICED_CORE_FIELDS.items():
            if f"{iced_field}: token_color({core_name}.{field})" not in body:
                violations.append(
                    f"{ICED_THEME} {skin.upper()}.{iced_field} must consume "
                    f"{core_name}.{field}"
                )

    # The hairline is part of the claimed shared scope: Iced must consume the
    # contract metric by name, and the whole Iced shell must have exactly one
    # spelling of a 1px border (the token).
    if (
        "pub const HAIRLINE: f32 = "
        "infiltrator_contract::design_tokens::metrics::HAIRLINE;" not in iced_text
    ):
        violations.append(
            f"{ICED_THEME} HAIRLINE must consume the shared contract metric"
        )
    check_no_raw_hairlines(violations)

    # Structural ladders: contract numbers, Iced by-name consumption, Bevy
    # numeric mirror.
    contract_text = read(DESIGN_TOKENS)
    expected_space = {
        "XS": 4.0,
        "SM": 8.0,
        "MD": 12.0,
        "LG": 16.0,
        "XL": 20.0,
        "XXL": 24.0,
    }
    for name, expected in expected_space.items():
        contract_value = number_const(contract_text, name)
        if contract_value != expected:
            violations.append(
                f"{DESIGN_TOKENS} space::{name}={contract_value!r}, expected {expected}"
            )
        if f"pub const SP_{name}: f32 = infiltrator_contract::design_tokens::space::{name};" not in iced_text:
            violations.append(
                f"{ICED_THEME} SP_{name} must consume the shared spacing ladder"
            )
    for name, expected in {"CARD": 16.0, "CONTROL": 10.0}.items():
        contract_value = number_const(contract_text, name)
        if contract_value != expected:
            violations.append(
                f"{DESIGN_TOKENS} radius::{name}={contract_value!r}, expected {expected}"
            )
        if f"pub const R_{name}: f32 = infiltrator_contract::design_tokens::radius::{name};" not in iced_text:
            violations.append(
                f"{ICED_THEME} R_{name} must consume the shared radius ladder"
            )

    bevy_text = read(BEVY_THEME)
    bevy_space = {
        "S4": "XS",
        "S8": "SM",
        "S12": "MD",
        "S16": "LG",
        "S20": "XL",
        "S24": "XXL",
    }
    for bevy_name, contract_name in bevy_space.items():
        bevy_value = number_const(bevy_text, bevy_name)
        expected = expected_space[contract_name]
        if bevy_value != expected:
            violations.append(
                f"{BEVY_THEME} space::{bevy_name}={bevy_value!r} must mirror "
                f"contract space::{contract_name}={expected}"
            )
    for name, expected in {"CARD": 16.0, "CONTROL": 10.0}.items():
        bevy_value = number_const(bevy_text, name)
        if bevy_value != expected:
            violations.append(
                f"{BEVY_THEME} radius::{name}={bevy_value!r} must mirror contract {expected}"
            )
    bevy_hairline = number_const(bevy_text, "HAIRLINE")
    contract_hairline = number_const(contract_text, "HAIRLINE")
    if bevy_hairline is None or bevy_hairline != contract_hairline:
        violations.append(
            f"{BEVY_THEME} metrics::HAIRLINE={bevy_hairline!r} must mirror "
            f"contract metrics::HAIRLINE={contract_hairline!r}"
        )


def check_no_raw_hairlines(violations: list[str]) -> None:
    """Fail closed if an Iced border goes back to a raw `width: 1.0`.

    DUAL-15-14 claims the hairline as a shared token; a page-local literal
    would silently break the claim (and the Bevy mirror would have nothing to
    mirror), so the whole Iced view layer must spell it `theme::HAIRLINE`.
    """
    roots = [ROOT / "crates/infiltrator-iced/src/view", ROOT / "crates/infiltrator-iced/src/view_root.rs"]
    roots.append(ROOT / "crates/infiltrator-iced/src/view_root")
    for root in roots:
        files = [root] if root.is_file() else sorted(root.rglob("*.rs"))
        for path in files:
            relative = path.relative_to(ROOT).as_posix()
            if relative.endswith("view/theme.rs"):
                continue
            for number, line in enumerate(
                path.read_text(encoding="utf-8").splitlines(), start=1
            ):
                if "width: 1.0" in line or "width: 1," in line:
                    violations.append(
                        f"{relative}:{number} hardcodes a 1px border width; "
                        "use theme::HAIRLINE (DUAL-15-14)"
                    )


PALETTE_FIELDS = {
    "scrim": "scrim",
    "hover": "hover_bg",
    "pressed": "pressed_bg",
    "focus_ring": "focus_ring",
    "disabled_ink": "disabled_ink",
}


def token_spellings(channels: tuple[float, ...]) -> set[str]:
    """The literal spellings a surface would use to hardcode these channels."""
    compact = ", ".join(repr(channel) for channel in channels)
    return {compact, compact.replace(", ", ",")}


#: Files exempt from the interaction-literal scan. `shader_fx.rs` is the
#: documented surface-local shadow/effect boundary (shadows are not
#: programmable on both toolkits), so its black-alpha shadow recipes may
#: legitimately collide with the light-skin pressed wash.
INTERACTION_LITERAL_SKIP = ("crates/infiltrator-bevy-widgets/src/shader_fx.rs",)


def check_interaction_token_mirrors(violations: list[str]) -> None:
    """DUAL-15-14: the interaction/overlay palette is shared, not surface-local.

    The contract owns the scrim, the hover/pressed washes, the focus ring and
    the disabled ink. Iced must consume them by name, the Bevy widget mirror
    must match channel-exactly and the palette must map them from its theme
    tokens. On top of that the named seams (Iced backdrops/controls, Bevy
    backdrops/focus ring/disabled ink) must reference the token, and a raw
    literal spelling of any shared interaction value outside the token module
    is rejected — a surface that hardcodes one fails this gate.
    """
    interactions = contract_interactions()
    if set(interactions) != set(SKINS):
        violations.append(
            f"{DESIGN_TOKENS} must define skin_interaction for every skin, "
            f"found {sorted(interactions)}"
        )
        return
    for skin, fields in interactions.items():
        missing = set(INTERACTION_FIELDS) - set(fields)
        if missing:
            violations.append(
                f"{DESIGN_TOKENS} {skin} interaction palette missing {sorted(missing)}"
            )

    # Iced: every skin token block consumes the contract fields by name.
    iced_text = read(ICED_THEME)
    for skin in SKINS:
        if f"skin_interaction(ThemeSkin::{skin})" not in iced_text:
            violations.append(
                f"{ICED_THEME} must resolve {skin} from the shared skin_interaction"
            )
        block = re.search(
            rf"pub const {skin.upper()}: Tokens = Tokens \{{(?P<body>.*?)\n\}};",
            iced_text,
            re.DOTALL,
        )
        if not block:
            violations.append(f"{ICED_THEME} missing the {skin.upper()} token block")
            continue
        body = block.group("body")
        for field, (iced_field, _) in INTERACTION_FIELDS.items():
            marker = (
                f"{iced_field}: token_color({skin.upper()}_INTERACTION.{field})"
            )
            if marker not in body:
                violations.append(
                    f"{ICED_THEME} {skin.upper()}.{iced_field} must consume "
                    f"{skin.upper()}_INTERACTION.{field}"
                )

    # Bevy: the widget mirror matches channel-exactly and the palette maps
    # every token from the theme instead of re-deriving one.
    mirror = bevy_cores()
    if set(mirror) != set(SKINS):
        violations.append(
            f"{BEVY_THEME} must define all four skins, found {sorted(mirror)}"
        )
    else:
        for skin in SKINS:
            for field, (_, bevy_field) in INTERACTION_FIELDS.items():
                expected = interactions[skin].get(field)
                actual = mirror[skin].get(bevy_field)
                if expected is None:
                    continue
                if actual is None:
                    violations.append(
                        f"{BEVY_THEME} {skin} missing interaction token {bevy_field!r}"
                    )
                    continue
                if len(expected) != len(actual) or any(
                    abs(left - right) > 1e-6 for left, right in zip(expected, actual)
                ):
                    violations.append(
                        f"{BEVY_THEME} {skin}.{bevy_field}={actual} must mirror "
                        f"contract {field}={expected}"
                    )
    palette_text = read("crates/infiltrator-bevy-widgets/src/palette.rs")
    for field, palette_field in PALETTE_FIELDS.items():
        if f"{palette_field}: theme_color(theme.{field})" not in palette_text:
            violations.append(
                "crates/infiltrator-bevy-widgets/src/palette.rs "
                f"must map {palette_field} from the theme token (`{field}`)"
            )
    forbid(
        violations,
        "crates/infiltrator-bevy-widgets/src/palette.rs",
        "pub fn scrim(&self)",
    )

    # Named seams: the token must be the thing the surface paints.
    for path, marker in (
        ("crates/infiltrator-iced/src/view/components.rs", "tk.hover"),
        ("crates/infiltrator-iced/src/view/components.rs", "tk.pressed"),
        ("crates/infiltrator-iced/src/view/component_forms.rs", "tk.focus_ring"),
    ):
        if marker not in read(path):
            violations.append(
                f"{path} must paint {marker} (the shared interaction token)"
            )
    for path in ICED_SCRIM_BACKDROPS:
        if ".scrim" not in read(path):
            violations.append(f"{path} must paint the shared scrim token")
        forbid(violations, path, "Color::BLACK")
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/focus.rs",
        "palette.focus_ring",
    )
    forbid(violations, "crates/infiltrator-bevy-widgets/src/focus.rs", "palette.accent")
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/button.rs",
        "palette.disabled_ink",
    )
    for path in BEVY_SCRIM_SCENES:
        if "palette.scrim" not in read(path):
            violations.append(f"{path} must paint the shared scrim token")
        forbid(violations, path, "Color::BLACK", "Color::srgba(0.0, 0.0, 0.0")

    # Raw literal rejection: the same channels typed outside the token module.
    spellings: dict[str, set[str]] = {}
    for skin in SKINS:
        for field in INTERACTION_FIELDS:
            expected = interactions[skin].get(field)
            if expected is None:
                continue
            spellings.setdefault(field, set()).update(token_spellings(expected))
    scanned: dict[str, str] = {}
    for parent, skip in (
        ("crates/infiltrator-iced/src", ICED_THEME),
        ("crates/infiltrator-bevy-widgets/src", BEVY_THEME),
        # The Bevy shell reads `UiPalette`; a shell-local literal is just as
        # much a drift as a widget-local one.
        ("crates/infiltrator-bevy-ui/src", ""),
    ):
        for path in sorted((ROOT / parent).rglob("*.rs")):
            relative = path.relative_to(ROOT).as_posix()
            if relative == skip or relative in INTERACTION_LITERAL_SKIP:
                continue
            text = scanned.get(relative)
            if text is None:
                text = path.read_text(encoding="utf-8")
                scanned[relative] = text
            for field, literals in spellings.items():
                for spelling in literals:
                    if spelling in text:
                        violations.append(
                            f"{relative} hardcodes interaction token {field}="
                            f"{spelling}; consume the shared token instead "
                            "(DUAL-15-14)"
                        )


TRAY_STATUS = "crates/infiltrator-contract/src/tray_status.rs"
WINDOW_CHROME = "crates/infiltrator-contract/src/window_chrome.rs"
A11Y = "crates/infiltrator-contract/src/a11y.rs"
ZH_LOCALES = [
    "crates/infiltrator-shared/src/locales_table.rs",
    "crates/infiltrator-shared/src/locales_table_ext.rs",
]
EN_LOCALES = [
    "crates/infiltrator-shared/src/locales_table_en.rs",
    "crates/infiltrator-shared/src/locales_table_en_ext.rs",
]


def code_lines(path: str) -> list[tuple[int, str]]:
    """Non-comment lines (`//` line comments are dropped).

    Honesty checks must look at real code: the surrounding doc comments in
    this repo deliberately explain why Iced has no AccessKit and why Bevy has
    no tray, and those explanations are not violations.
    """
    lines: list[tuple[int, str]] = []
    for number, line in enumerate(read(path).splitlines(), start=1):
        if line.strip().startswith("//"):
            continue
        lines.append((number, line.split("//", 1)[0]))
    return lines


def locale_keys(paths: list[str]) -> set[str]:
    keys: set[str] = set()
    for path in paths:
        keys.update(re.findall(r'^\s*"([a-z0-9_]+)"\s*=>', read(path), re.MULTILINE))
    return keys


def a11y_label_keys() -> list[str]:
    """Every `label_key()` arm from the shared a11y grammar."""
    text = read(A11Y)
    match = re.search(
        r"pub const fn label_key\(self\)[^{]*\{(?P<body>.*?)\n    \}",
        text,
        re.DOTALL,
    )
    if not match:
        return []
    return re.findall(r'=>\s*"([a-z0-9_]+)"', match.group("body"))


def a11y_enum_variants() -> list[str]:
    """The variant names of the shared `ShellA11yNode` grammar."""
    text = re.sub(r"//[^\n]*", "", read(A11Y))
    match = re.search(
        r"pub enum ShellA11yNode \{(?P<body>.*?)\n\}", text, re.DOTALL
    )
    if not match:
        return []
    return re.findall(r"^\s{4}([A-Z][A-Za-z0-9_]*)\s*(?:\{[^}]*\})?,", match.group("body"), re.MULTILINE)


def a11y_per_node_table(fn_name: str) -> dict[str, str]:
    """One `fn <name>(self)` arm table as `node -> literal`.

    Grouped arms (`Self::A | Self::B => ...`) expand to one entry per node, so
    the table is comparable to the variant list.
    """
    text = re.sub(r"//[^\n]*", "", read(A11Y))
    match = re.search(
        rf"pub const fn {fn_name}\(self\)[^{{]*\{{(?P<body>.*?)\n    \}}",
        text,
        re.DOTALL,
    )
    if not match:
        return {}
    table: dict[str, str] = {}
    # Arms may span lines (`Self::A\n | Self::B => ...`), so the left-hand
    # side excludes commas only.
    for left, right in re.findall(r"([^,=]*?)=>\s*([^,\n]+?),", match.group("body")):
        for node in re.findall(r"Self::(\w+)", left):
            table[node] = right.strip()
    return table


def impl_block(text: str, header: str) -> str:
    """The body of one top-level `impl ... { ... }` block."""
    match = re.search(
        rf"impl {re.escape(header)} \{{(?P<body>.*?)\n\}}",
        text,
        re.DOTALL,
    )
    return match.group("body") if match else ""


def a11y_inventory_size() -> int | None:
    """The `ShellA11yNode::ALL` array length from the grammar."""
    match = re.search(
        r"pub const ALL: \[Self; (\d+)\]",
        impl_block(read(A11Y), "ShellA11yNode"),
    )
    return int(match.group(1)) if match else None


def a11y_role_inventory_size() -> int | None:
    """The `A11yRole::ALL` array length from the grammar."""
    match = re.search(
        r"pub const ALL: \[Self; (\d+)\]",
        impl_block(read(A11Y), "A11yRole"),
    )
    return int(match.group(1)) if match else None


def bevy_accesskit_roles() -> set[str]:
    """Every shared `A11yRole` variant the Bevy mapping translates."""
    text = re.sub(r"//[^\n]*", "", read("crates/infiltrator-bevy-ui/src/a11y.rs"))
    match = re.search(
        r"pub const fn accesskit_role\(role: A11yRole\)[^{]*\{(?P<body>.*?)\n\}",
        text,
        re.DOTALL,
    )
    if not match:
        return set()
    return set(re.findall(r"A11yRole::(\w+)\s*=>", match.group("body")))


def check_a11y_label_coverage(violations: list[str]) -> None:
    """DUAL-15-10: every grammar row must resolve in both locales.

    The Iced surface resolves `label_key()` through its localizer and the
    Bevy surface publishes `label_zh()`; a key without copy would silently
    fall back to the raw key, which is exactly the drift this gate forbids.
    The inventory itself is checked too: the `ALL` array, the enum and the
    three per-node tables must describe the same rows, so a node added
    without copy or without inventory coverage fails the gate.
    """
    variants = a11y_enum_variants()
    inventory = a11y_inventory_size()
    keys = a11y_per_node_table("label_key")
    roles = a11y_per_node_table("role")
    zh_labels = a11y_per_node_table("label_zh")

    if not variants or not keys or not roles or not zh_labels:
        violations.append(f"{A11Y} grammar tables must be parseable")
        return
    for name, table in (("label_key", keys), ("role", roles), ("label_zh", zh_labels)):
        missing = sorted(set(variants) - set(table))
        extra = sorted(set(table) - set(variants))
        if missing or extra:
            violations.append(
                f"{A11Y} {name}() must cover every node once; missing={missing} extra={extra}"
            )
    if inventory != len(variants):
        violations.append(
            f"{A11Y} ShellA11yNode::ALL declares {inventory} rows for "
            f"{len(variants)} enum variants"
        )

    role_variants = a11y_role_variants()
    role_inventory = a11y_role_inventory_size()
    if not role_variants:
        violations.append(f"{A11Y} A11yRole must define its variants")
    elif role_inventory != len(role_variants):
        violations.append(
            f"{A11Y} A11yRole::ALL declares {role_inventory} roles for "
            f"{len(role_variants)} enum variants; a role added without the "
            "shared inventory would let a surface silently skip it"
        )

    label_keys = [value.strip().strip('"') for value in keys.values()]
    if len(label_keys) != len(set(label_keys)):
        violations.append(f"{A11Y} label_key() must define one unique key per node")
    zh_texts = [value.strip().strip('"') for value in zh_labels.values()]
    empty = sorted(node for node, value in zh_labels.items() if not value.strip().strip('"'))
    if empty:
        violations.append(f"{A11Y} label_zh() rows without copy: {empty}")
    if len(zh_texts) != len(set(zh_texts)):
        duplicates = sorted({text for text in zh_texts if zh_texts.count(text) > 1})
        violations.append(f"{A11Y} label_zh() must be unique; duplicated: {duplicates}")

    # The Bevy surface must translate every shared role into a real AccessKit
    # role; a missing arm would silently fall back to a wrong role.
    mapped = bevy_accesskit_roles()
    shared_roles = set(a11y_role_variants())
    if not shared_roles:
        violations.append(f"{A11Y} A11yRole must define its variants")
    missing_roles = sorted(shared_roles - mapped)
    if missing_roles:
        violations.append(
            f"crates/infiltrator-bevy-ui/src/a11y.rs accesskit_role() misses {missing_roles}"
        )

    zh_keys = locale_keys(ZH_LOCALES)
    en_keys = locale_keys(EN_LOCALES)
    for key in label_keys:
        if key not in zh_keys:
            violations.append(f"{A11Y} label key {key!r} missing from the zh-CN tables")
        if key not in en_keys:
            violations.append(f"{A11Y} label key {key!r} missing from the en-US tables")


def labelled_call_arguments(text: str) -> list[str]:
    """The argument text of every `labelled(...)` call, paren-balanced."""
    calls: list[str] = []
    for match in re.finditer(r"\blabelled\(", text):
        depth = 1
        index = match.end()
        while index < len(text) and depth > 0:
            if text[index] == "(":
                depth += 1
            elif text[index] == ")":
                depth -= 1
            index += 1
        calls.append(text[match.end() : index - 1])
    return calls


def check_iced_tooltip_coverage(violations: list[str]) -> None:
    """DUAL-15-10: the Iced tooltip rows must really be labelled.

    Iced cannot publish AccessKit roles, so its honest mapping of a shared
    semantic row is a visible localized tooltip. This gate pins that claim
    row-by-row: every declared icon-only/colour-only row must appear inside a
    `labelled(...)` call in the listed file, and every grammar row must be
    classified as tooltip / visible-text / absent-on-Iced — so a node added
    to the shared inventory without an Iced decision fails instead of
    silently shrinking the coverage inventory.
    """
    covered = set(ICED_TOOLTIP_ROWS) | ICED_VISIBLE_TEXT_ROWS | ICED_ABSENT_ROWS
    grammar = set(a11y_enum_variants())
    unclassified = sorted(grammar - covered)
    if unclassified:
        violations.append(
            f"{A11Y} rows without an Iced coverage decision: {unclassified}; "
            "classify each new row as tooltip / visible text / absent (DUAL-15-10)"
        )
    extra = sorted(covered - grammar)
    if extra:
        violations.append(
            f"multimodal-shell-guard.py classifies unknown a11y rows: {extra}"
        )

    for node, path in ICED_TOOLTIP_ROWS.items():
        calls = labelled_call_arguments(read(path))
        if not any(f"ShellA11yNode::{node}" in call for call in calls):
            violations.append(
                f"{path} must carry {node} inside a labelled(...) tooltip "
                "(DUAL-15-10)"
            )


def a11y_role_variants() -> list[str]:
    """The variant names of the shared `A11yRole` vocabulary."""
    text = re.sub(r"//[^\n]*", "", read(A11Y))
    match = re.search(r"pub enum A11yRole \{(?P<body>.*?)\n\}", text, re.DOTALL)
    if not match:
        return []
    return re.findall(
        r"^\s{4}([A-Z][A-Za-z0-9_]*)\s*(?:\{[^}]*\})?,",
        match.group("body"),
        re.MULTILINE,
    )


def check_capability_honesty(violations: list[str]) -> None:
    """No surface may reference a capability it does not host.

    Iced 0.14 has no AccessKit integration, and the Bevy shell has no tray
    host: both boundaries are honest deviations in the ledger, so a real code
    reference to the missing integration would be a fabrication (DUAL-15-02,
    DUAL-15-10).
    """
    for path in sorted((ROOT / "crates/infiltrator-iced/src").rglob("*.rs")):
        relative = path.relative_to(ROOT).as_posix()
        for number, code in code_lines(relative):
            if "accesskit" in code.lower():
                violations.append(
                    f"{relative}:{number} references accesskit; Iced has no "
                    "AccessKit integration (DUAL-15-10 must stay honest)"
                )
    for path in sorted((ROOT / "crates/infiltrator-bevy-ui/src").rglob("*.rs")):
        relative = path.relative_to(ROOT).as_posix()
        for number, code in code_lines(relative):
            lowered = code.lower()
            for marker in ("ksni", "tray-icon", "tray_icon", "muda", "statusnotifier"):
                if marker in lowered:
                    violations.append(
                        f"{relative}:{number} references {marker!r}; the Bevy "
                        "surface reports no tray host instead (DUAL-15-02)"
                    )

    # DUAL-15-11: Iced's candidate box is the toolkit text widget's job. The
    # toolkit seam is pinned by a test; the shell must not fabricate a cursor
    # area of its own (there is no public iced API to set one), and it must not
    # claim the surface-computed source vocabulary.
    for path in sorted((ROOT / "crates/infiltrator-iced/src").rglob("*.rs")):
        relative = path.relative_to(ROOT).as_posix()
        for number, code in code_lines(relative):
            lowered = code.lower()
            for marker in ("set_ime", "ime_enabled", "ime_position", "surfacecomputed"):
                if marker in lowered:
                    violations.append(
                        f"{relative}:{number} references {marker!r}; Iced's text "
                        "widget owns the IME cursor area (DUAL-15-11 must stay honest)"
                    )


def check_touch_gesture_consumer(violations: list[str]) -> None:
    """DUAL-15-07 must keep a real Bevy touch consumer.

    The ledger used to record "no consumer" as the blocker; once the item is
    `parity-ready` this gate re-derives the consumer from the shell source, so
    deleting the `MessageReader<TouchInput>` wiring (or re-implementing the
    recognizer in the shell) fails instead of silently re-opening the gap.
    """
    if ledger_statuses().get("DUAL-15-07") != "parity-ready":
        return
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/gesture.rs",
        "MessageReader<TouchInput>",
        "GestureRecognizer",
        "GestureSnapshot",
        "ShellGesturePlugin",
        "shell_gesture::",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/shell_gesture.rs",
        "pub struct GestureSnapshot",
        "pub enum GestureSemanticEvent",
        "pub enum TouchGestureSupport",
        "pub fn apply(&mut self, event: GestureSemanticEvent)",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/touch_gesture.rs",
        "pub trait TouchGesturePort",
        "pub struct TouchGestureHostReport",
    )
    # The recognizer stays the widget layer's single source of truth: its
    # private thresholds must not be copied into the shell.
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/gesture.rs",
        "threshold_pan_px",
        "long_press_duration_ms",
        "double_tap_duration_ms",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    # The full per-item ledger must exist for all fifteen items, with the
    # status vocabulary the rest of the master plan uses.
    require(
        violations,
        LEDGER,
        "DUAL-15-01",
        "DUAL-15-02",
        "DUAL-15-03",
        "DUAL-15-04",
        "DUAL-15-05",
        "DUAL-15-06",
        "DUAL-15-07",
        "DUAL-15-08",
        "DUAL-15-09",
        "DUAL-15-10",
        "DUAL-15-11",
        "DUAL-15-12",
        "DUAL-15-13",
        "DUAL-15-14",
        "DUAL-15-15",
        "parity-ready",
        "shared-ready",
        "planned",
        "组 15 逐项账目",
        "2026-09-22 组 15 批次 A",
        "组 15 多模态外壳与极客命令流 | 15 | `in progress (14/15)`",
        # Batch G: the interaction palette and the a11y coverage inventory.
        "2026-09-23 组 15 批次 G",
        "SkinInteractionPalette",
        # Batch H: the shared touch-gesture contract, host seam and Bevy consumer.
        "2026-09-23 组 15 批次 H",
        "GestureSemanticEvent",
        "GestureSnapshot",
        "TouchGesturePort",
        "ShellGesturePlugin",
        "MessageReader<TouchInput>",
        # 15-01 stays authoritatively tracked in the responsive ledger.
        "RESPONSIVE_PARITY_LEDGER.md",
    )
    require(violations, RESPONSIVE_LEDGER, "DUAL-15-01", "parity-ready")

    # Closed items carry their distinctive evidence tokens in the ledger.
    require(
        violations,
        LEDGER,
        "ShortcutRegistry",
        "ShortcutChord",
        "ThemePreference",
        "ThemeSkin",
        "ToastGate",
        "ToastPolicy",
        "push_toast",
        "sync_system_appearance",
        "a_sensitive_toast_is_redacted_before_it_renders",
        "identical_toasts_are_coalesced_and_the_stack_is_capped",
        "widget_skin_mirror_matches_the_shared_contract",
        "the_keyboard_chord_dispatches_through_the_shared_registry",
        "the_open_palette_owns_the_arrow_keys",
        "capture_rebinds_through_the_shared_settings_command",
        "a_stored_custom_chord_blocks_later_captures",
        # DUAL-15-10/11/15: the extended coverage + IME evidence.
        "CommandPaletteQuery",
        "19 行",
        "IME 中文输入法候选框定位 | `parity-ready`",
        "infiltrator-contract/src/ime.rs",
        "ImeCursorRect",
        "ImeCompositionTracker",
        "ToolkitProvided",
        "ime_tests.rs",
        "Window.ime_enabled",
        "set_ime_cursor_area",
    )

    # Shared contracts: appearance, shortcuts, notifications.
    require(
        violations,
        "crates/infiltrator-contract/src/theme.rs",
        "pub enum ThemeSkin",
        "pub enum ThemePreference",
        "pub const fn resolve",
        "pub fn parse_strict",
        "pub const fn next",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/shortcuts.rs",
        "pub enum ShortcutAction",
        "pub struct ShortcutChord",
        "pub struct ShortcutRegistry",
        "pub fn display_string",
        "pub fn find_conflict",
        "pub fn bind_or_replace",
        "pub fn detect_conflicts",
        "pub fn normalize",
        "pub fn reset_action",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/toast.rs",
        "pub enum ToastSeverity",
        "pub struct ToastPolicy",
        "pub struct ToastGate",
        "pub fn admit",
        "pub fn live_offers",
    )

    # Application layer: shared shortcut use-cases + validated settings writes.
    require(
        violations,
        "crates/infiltrator-application/src/shortcut_application.rs",
        "pub struct ShortcutApplication",
        "pub async fn registry",
        "pub async fn capture",
        "pub async fn set_enabled",
        "pub async fn reset_action",
        "settings.shortcuts",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/settings.rs",
        "pub shortcuts: Vec<infiltrator_contract::shortcuts::ShortcutBinding>",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        'strip_prefix("shortcut.")',
        "ShortcutChord::parse",
        "ThemePreference::parse_strict",
    )

    # Iced surface: appearance follow, registry-driven hotkeys, toast policy.
    require(
        violations,
        "crates/infiltrator-iced/src/update/shell.rs",
        "pub fn push_toast",
        "fn handle_keyboard_chord",
        "SystemThemeChanged",
        "toast_gate.admit",
        "Message::RemoveToast",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/shortcuts_store.rs",
        "pub async fn capture",
        "pub async fn set_enabled",
        "pub async fn reset_action",
        "ShortcutApplication::new",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings/hotkeys.rs",
        "hotkeys_card",
        "shortcut_registry.get(action)",
        "Message::BeginHotkeyCapture",
        "Message::ResetHotkey",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/theme.rs",
        "pub fn theme_for_skin",
        "ThemeSkin::from_setting",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/toast_state.rs",
        "infiltrator_contract::toast",
        "ToastGate",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/subscription.rs",
        "iced::system::theme_changes()",
        "Message::KeyboardChord",
        "Message::SystemThemeChanged",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update.rs",
        "| Message::SetTheme(_)",
        "| Message::KeyboardChord { .. }",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub shortcut_registry",
        "pub theme_preference",
        "pub toast_gate",
        "pub fn apply_theme_preference",
    )

    # Bevy surface: shared registry dispatch, OS appearance follow, toast overlay.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/shortcuts.rs",
        "pub struct ShortcutBindings",
        "pub struct HotkeyCapture",
        "pub struct ChordPressed",
        "pub fn forward_pressed_chords",
        "pub fn dispatch_chords",
        "ShortcutRegistry",
        "UiCommand::UpdateSetting",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/toast.rs",
        "pub struct ToastPolicyGate",
        "pub struct ShellToast",
        "pub fn on_shell_toast",
        "pub fn sync_toast_stack",
        "redact::redact_line",
        "toast_stack_scene",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/appearance.rs",
        "pub struct ThemeMode",
        "pub struct SystemAppearance",
        "pub fn resolved_skin",
        "pub fn sync_system_appearance",
        "window_theme",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/theme.rs",
        "pub enum ThemeSkin",
        "pub fn forest",
        "pub fn amoled",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/toast.rs",
        "DEFAULT_TOAST_CAPACITY",
        "impl Default for ToastQueue",
    )

    # Dual headless tests: one evidence test per closed item on each surface.
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs",
        "the_shell_follows_the_os_appearance_while_the_preference_is_system",
        "a_system_preference_survives_the_settings_round_trip",
        "the_keyboard_chord_dispatches_through_the_shared_registry",
        "the_open_palette_owns_the_arrow_keys",
        "identical_toasts_are_coalesced_and_the_stack_is_capped",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/theme_skin_tests.rs",
        "widget_skin_mirror_matches_the_shared_contract",
        "the_shell_follows_the_os_appearance_while_preference_is_system",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/shortcut_tests.rs",
        "capture_rebinds_through_the_shared_settings_command",
        "a_bound_chord_reaches_the_command_sink",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/toast_overlay_tests.rs",
        "a_sensitive_toast_is_redacted_before_it_renders",
        "the_overlay_mounts_a_single_stack_root",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application_settings_tests.rs",
        "shortcut_capture_persists_and_rejects_conflicts",
        "theme_writes_are_validated_and_canonicalised",
    )

    # ---- Batch B (DUAL-15-03/04/05): command palette catalogue and Mini HUD.
    # The ledger records the batch and its evidence tokens.
    require(
        violations,
        LEDGER,
        "2026-09-22 组 15 批次 B",
        "CommandCatalogue",
        "CommandTarget",
        "ShellPage",
        "MiniHudReadModel",
        "MiniHudPlacement",
        "MiniHudWindowPort",
        "MiniHudApplication",
        "the_palette_lists_the_shared_catalogue_and_wraps_like_bevy",
        "the_palette_executes_shared_targets",
        "test_palette_keyboard_navigation_typing_and_close",
        "test_palette_row_click_executes_that_row",
        "the_mini_hud_read_model_comes_from_live_projections",
        "the_mini_hud_drag_moves_the_persisted_placement",
        "the_mounted_scene_renders_the_shared_read_model",
        "the_pin_request_persists_through_the_shared_settings_command",
        "place_snaps_clamps_persists_and_reports_the_host_outcome",
        "a_host_without_the_window_adapter_reports_typed_unsupported",
        "mini_hud_placement_writes_are_validated_per_field",
    )

    # Shared contracts: the palette catalogue and the Mini HUD read model.
    require(
        violations,
        "crates/infiltrator-contract/src/command_catalogue.rs",
        "pub enum ShellPage",
        "pub enum CommandCategory",
        "pub enum CommandTarget",
        "pub struct CommandCatalogue",
        "pub fn with_profiles",
        "pub fn filtered_indices",
        "pub const fn label_zh",
        "pub const fn shortcut_action",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/mini_hud.rs",
        "pub struct MiniHudPlacement",
        "pub struct MiniHudDisplay",
        "pub struct MiniHudReadModel",
        "pub struct MiniHudSnapPlacement",
        "pub fn snap_to_edges",
        "pub fn status_line",
        "pub enum MiniHudHostOutcome",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub mini_hud: crate::mini_hud::MiniHudPlacement",
    )

    # Application + ports: placement use-cases and the host window capability.
    require(
        violations,
        "crates/infiltrator-application/src/mini_hud_application.rs",
        "pub struct MiniHudApplication",
        "pub async fn place_from",
        "pub async fn set_pinned_from",
        "pub struct MiniHudPlacementReport",
        "MINI_HUD_SNAP_THRESHOLD_PX",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/mini_hud_window.rs",
        "pub trait MiniHudWindowPort",
        "async fn apply_placement",
        "async fn set_visible",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "fn mini_hud_window_port",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/settings.rs",
        "pub mini_hud: infiltrator_contract::mini_hud::MiniHudPlacement",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        'strip_prefix("mini_hud.")',
        "placement.pinned = value.parse",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "mini_hud: settings.mini_hud",
    )

    # Iced surface: catalogue-driven palette and the real HUD window mode.
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub command_catalogue",
        "pub mini_hud_placement",
        "pub fn rebuild_command_catalogue",
        "pub fn filtered_command_indices",
        "pub fn mini_hud_read_model",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view_root/command_palette.rs",
        "infiltrator_contract::command_catalogue",
        "state.filtered_command_indices()",
        "shortcut_action()",
        "command_category_badge",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui.rs",
        "update_mini_hud",
        "self.filtered_command_indices()",
        "on_shell_shortcut(action)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/mini_hud.rs",
        "update_mini_hud",
        "mini_hud_store::place",
        "mini_hud_store::set_pinned",
        "mini_hud_window::enter",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/mini_hud_window.rs",
        "iced::window::move_to",
        "iced::window::set_level",
        "Level::AlwaysOnTop",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/mini_hud_store.rs",
        "MiniHudApplication::with_window_port",
        "place_from",
        "set_pinned_from",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/mini_hud.rs",
        "mini_hud_read_model",
        "Message::MiniHudMoved",
        "Message::MiniHudDragReleased",
        "mouse_area",
    )

    # Bevy surface: mounted palette overlay + keyboard seam, mounted HUD model.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command_palette_shell.rs",
        "pub struct CommandPalettePlugin",
        "pub fn palette_keyboard_input",
        "pub fn sync_palette_overlay",
        "pub fn sync_palette_catalogue",
        "pub fn on_execute_selected_palette_action",
        "Route::from_shell_page",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command_palette.rs",
        "pub catalogue: CommandCatalogue",
        "pub fn from_catalogue",
        "command_palette_modal_scene",
        "pinyin_fuzzy_match",
        "pub fn accelerator_for",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud_shell.rs",
        "pub struct MiniHudPlugin",
        "pub fn sync_mini_hud_model",
        "pub fn sync_mini_hud_overlay",
        '"mini_hud.pinned"',
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud.rs",
        "MiniHudReadModel",
        "pub fn mini_hud_scene",
        "status_line",
        "pub struct ToggleMiniHud",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/shortcuts.rs",
        "crate::mini_hud::ToggleMiniHud",
    )

    # Dual headless tests for this batch.
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs",
        "the_palette_lists_the_shared_catalogue_and_wraps_like_bevy",
        "the_palette_executes_shared_targets",
        "the_mini_hud_read_model_comes_from_live_projections",
        "the_mini_hud_drag_moves_the_persisted_placement",
        "always_on_top_mirrors_the_pin_onto_the_shared_placement",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/command_palette_tests.rs",
        "test_palette_mounts_and_unmounts_from_the_shared_catalogue",
        "test_palette_keyboard_navigation_typing_and_close",
        "test_palette_row_click_executes_that_row",
        "test_command_palette_action_execution_dispatches_route_and_command",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/mini_hud_tests.rs",
        "the_toggle_event_mounts_and_unmounts_the_overlay",
        "the_read_model_comes_from_the_live_projections",
        "the_mounted_scene_renders_the_shared_read_model",
        "the_pin_request_persists_through_the_shared_settings_command",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application_settings_tests.rs",
        "mini_hud_placement_writes_are_validated_per_field",
    )
    require(
        violations,
        "crates/infiltrator-application/src/mini_hud_application.rs",
        "place_snaps_clamps_persists_and_reports_the_host_outcome",
        "a_host_without_the_window_adapter_reports_typed_unsupported",
    )

    # ---- Batch C (DUAL-15-03/04/08/14/15): the desktop HUD window adapter,
    # the shared design tokens, the shared render cadence and the matrix.
    require(
        violations,
        LEDGER,
        "2026-09-22 组 15 批次 C",
        "DesktopMiniHudWindow",
        "MiniHudWindowHandle",
        "IcedMiniHudWindowHandle",
        "RenderCadence",
        "design_tokens",
        "MULTIMODAL_SHELL_MATRIX.md",
        "a_persisted_placement_reaches_the_iced_window_handle",
        "the_placement_update_drains_the_host_window_requests",
        "a_bound_handle_receives_the_placement_and_the_visibility_request",
        "an_unbound_or_stale_handle_reports_typed_unsupported",
        "the_hud_quick_switches_dispatch_the_shared_toggle_commands",
        "a_pending_hud_quick_switch_dispatches_nothing",
        "the_widget_palette_mirrors_the_shared_design_tokens",
        "the_widget_ladders_mirror_the_shared_contract_numbers",
        "the_iced_tokens_resolve_the_shared_design_contract",
        "the_shell_tracks_window_focus_for_the_shared_cadence",
        "the_winit_modes_follow_the_shared_cadence",
        "focus_and_occlusion_events_reselect_the_winit_cadence",
        "the_widget_frame_pacing_vocabulary_mirrors_the_shared_cadence",
    )

    # Ports: the window-owner half of the HUD window capability.
    require(
        violations,
        "crates/infiltrator-ports/src/mini_hud_window.rs",
        "pub trait MiniHudWindowHandle",
        "fn apply_placement(&self, placement: MiniHudPlacement) -> bool",
        "fn set_visible(&self, visible: bool) -> bool",
    )
    # Desktop: the real host adapter + runtime capability.
    require(
        violations,
        "crates/infiltrator-desktop/src/mini_hud_window.rs",
        "pub struct DesktopMiniHudWindow",
        "pub fn shared",
        "pub fn bind",
        "impl MiniHudWindowPort for DesktopMiniHudWindow",
        "MiniHudHostOutcome::Unsupported",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "fn mini_hud_window_port",
        "DesktopMiniHudWindow::shared()",
    )
    # Iced: the window handle, the update-path drain and the store port.
    require(
        violations,
        "crates/infiltrator-iced/src/mini_hud_window.rs",
        "pub struct IcedMiniHudWindowHandle",
        "pub fn install_host_handle",
        "pub fn host_requests_task",
        "pub fn mark_live",
        "pub fn take_pending",
        "impl MiniHudWindowHandle for IcedMiniHudWindowHandle",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/mini_hud.rs",
        "host_requests_task",
        "mark_live",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/mini_hud_store.rs",
        "crate::host::mini_hud::window_port",
    )
    # The concrete desktop dependency stays at the composition boundary.
    require(
        violations,
        "crates/infiltrator-iced/src/host.rs",
        "DesktopMiniHudWindow::shared",
        "pub fn window_port",
        "pub fn bind_window_handle",
    )
    # Iced: the shared render cadence.
    require(
        violations,
        "crates/infiltrator-iced/src/subscription.rs",
        "frame_cadence_subscription",
        "window::Event::Focused",
        "window::Event::Unfocused",
        "RenderCadence::from_focused",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub window_focused",
    )
    # Contract: the shared palette + cadence policy.
    require(
        violations,
        DESIGN_TOKENS,
        "pub struct RgbaToken",
        "pub struct SkinCorePalette",
        "pub const fn skin_core",
        "pub mod space",
        "pub mod radius",
        "pub mod metrics",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/cadence.rs",
        "pub enum RenderCadence",
        "pub const fn from_focused",
        "pub const fn from_visible_focused",
        "pub const fn frame_time_ms",
        "pub fn frame_interval",
        "BACKGROUND_FRAME_TIME_MS",
    )
    # Bevy: cadence projection onto winit + the HUD quick switches.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/cadence.rs",
        "pub fn winit_settings_for",
        "pub fn sync_window_cadence",
        "pub struct CadencePlugin",
        "WinitSettings",
        "WindowFocused",
        "WindowOccluded",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/lib.rs",
        "pub mod cadence",
        "cadence::CadencePlugin",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud.rs",
        "MiniHudSystemProxyToggle",
        "MiniHudTunToggle",
        "ButtonDisabled",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud_shell.rs",
        "on_mini_hud_system_proxy_activated",
        "on_mini_hud_tun_activated",
        "submit_toggle",
        "UiCommand::ToggleTun",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/cadence.rs",
        "FramePacingMode::BackgroundThrottled => 500",
    )
    # The dual tests introduced by this batch.
    require(
        violations,
        "crates/infiltrator-desktop/src/mini_hud_window.rs",
        "fn a_bound_handle_receives_the_placement_and_the_visibility_request",
        "fn an_unbound_or_stale_handle_reports_typed_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/mini_hud_window_tests.rs",
        "fn a_handle_without_a_live_window_refuses_the_placement",
        "fn a_live_window_accepts_exactly_once_per_request",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/mini_hud_store_tests.rs",
        "fn a_persisted_placement_reaches_the_iced_window_handle",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs",
        "the_placement_update_drains_the_host_window_requests",
        "the_iced_tokens_resolve_the_shared_design_contract",
        "the_shell_tracks_window_focus_for_the_shared_cadence",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/mini_hud_tests.rs",
        "the_hud_quick_switches_dispatch_the_shared_toggle_commands",
        "a_pending_hud_quick_switch_dispatches_nothing",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/design_token_tests.rs",
        "the_widget_palette_mirrors_the_shared_design_tokens",
        "the_widget_ladders_mirror_the_shared_contract_numbers",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/cadence_tests.rs",
        "the_winit_modes_follow_the_shared_cadence",
        "focus_and_occlusion_events_reselect_the_winit_cadence",
        "the_widget_frame_pacing_vocabulary_mirrors_the_shared_cadence",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/cadence.rs",
        "fn every_host_fact_maps_to_one_cadence",
        "fn the_product_rates_are_sixty_and_two_fps",
    )
    require(
        violations,
        DESIGN_TOKENS,
        "fn every_skin_has_a_distinct_core_palette",
        "fn the_spacing_and_radius_ladders_are_positive_and_ordered",
    )

    # ---- Batch D (DUAL-15-03/14/15): the shared HUD waveform strip, the
    # consumed hairline token, and the hardened matrix.
    require(
        violations,
        LEDGER,
        "2026-09-22 组 15 批次 D",
        "MiniHudWaveformStrip",
        "the_waveform_strip_projects_the_newest_real_samples",
        "an_empty_or_non_finite_waveform_never_fabricates_bars",
        "the_waveform_strip_comes_from_the_shared_live_samples",
        "the_mounted_waveform_slots_rasterize_the_shared_strip",
        "the_mini_hud_view_renders_the_shared_strip",
        "demo_seeds_the_shared_live_waveform_slot",
        "sparkline_image_projects_normalized_bars_without_a_grid",
        "theme_hairline_consumes_the_shared_contract_metric",
        "check_no_raw_hairlines",
        "ledger_statuses",
        "sync_mini_hud_waveforms",
    )

    # Contract: the shared normalization both surfaces consume.
    require(
        violations,
        "crates/infiltrator-contract/src/mini_hud.rs",
        "pub const MINI_HUD_WAVEFORM_BARS",
        "pub struct MiniHudWaveformStrip",
        "pub fn from_snapshot(snapshot: &TrafficWaveformSnapshot) -> Self",
        "pub fn bar_fraction(bar: u16) -> f32",
        "pub fn with_waveform(mut self, snapshot: &TrafficWaveformSnapshot) -> Self",
        "pub waveform: MiniHudWaveformStrip",
        "pub const WIDTH_PX: u32 = 60",
    )

    # Iced: the strip canvas and the hairline token.
    require(
        violations,
        "crates/infiltrator-iced/src/view/waveform.rs",
        "pub enum StripInk",
        "pub struct MiniWaveformStrip",
        "pub fn hud_waveform<'a, Message: 'a>(bars: &[u16], ink: StripInk) -> Element<'a, Message>",
        "MiniHudWaveformStrip::WIDTH_PX",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/mini_hud.rs",
        "hud_waveform(&model.waveform.down, StripInk::Accent)",
        "hud_waveform(&model.waveform.up, StripInk::Success)",
    )
    require(
        violations,
        ICED_THEME,
        "pub const HAIRLINE: f32 = "
        "infiltrator_contract::design_tokens::metrics::HAIRLINE;",
    )

    # Bevy: the business-agnostic sparkline seam and the mounted HUD slots.
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/chart.rs",
        "pub fn sparkline_image(",
        "linear_polyline(samples, width as f32, height as f32, Some(1.0))",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud.rs",
        "pub struct MiniHudDownWaveform",
        "pub struct MiniHudUpWaveform",
        "fn down_waveform_slot_scene()",
        "fn up_waveform_slot_scene()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud_shell.rs",
        ".with_waveform(&overview.traffic_waveform)",
        "pub fn sync_mini_hud_waveforms",
        "sparkline_image(",
        "MiniHudDownWaveform",
        "MiniHudUpWaveform",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/tests/headless/chart_tests.rs",
        "fn sparkline_image_projects_normalized_bars_without_a_grid",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/mini_hud_tests.rs",
        "fn the_waveform_strip_comes_from_the_shared_live_samples",
        "fn the_mounted_waveform_slots_rasterize_the_shared_strip",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_theme_tests.rs",
        "fn theme_hairline_consumes_the_shared_contract_metric",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs",
        "fn the_mini_hud_view_renders_the_shared_strip",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/mini_hud.rs",
        "fn the_waveform_strip_projects_the_newest_real_samples",
        "fn an_empty_or_non_finite_waveform_never_fabricates_bars",
    )

    # The mirrored skin vocabulary must match the contract numerically.
    contract_skins = skin_settings("crates/infiltrator-contract/src/theme.rs")
    widget_skins = skin_settings("crates/infiltrator-bevy-widgets/src/theme.rs")
    if contract_skins != widget_skins or not contract_skins:
        violations.append(
            "skin setting vocabulary drift: contract "
            f"{sorted(contract_skins)} != widget {sorted(widget_skins)}"
        )

    # DUAL-15-14: the numeric design-token mirror (contract -> Iced -> Bevy).
    check_design_token_mirrors(violations)

    # DUAL-15-15: the machine-checkable multimodal regression matrix.
    check_matrix(violations)

    # ---- Batch E (DUAL-15-02/10/13): the shared tray-status contract with the
    # live rate badge, the shared accessibility grammar, and the frameless
    # window-chrome contract with its real host ops on both surfaces.
    require(
        violations,
        LEDGER,
        "2026-09-22 组 15 批次 E",
        "TrayRateBadge",
        "TRAY_RATE_REFRESH_INTERVAL_MS",
        "ChromeRequest",
        "WindowChromePlugin",
        "ShellA11yNode",
        "TrayStatusReport",
        "the_tray_spec_carries_the_live_rate_badge_from_the_shared_waveform",
        "the_rate_badge_push_is_deduplicated_and_uses_the_shared_interval",
        "the_frameless_window_settings_consume_the_shared_chrome_contract",
        "every_chrome_message_maps_to_one_real_host_window_op",
        "every_shared_semantic_node_resolves_a_localized_label_and_role",
        "the_host_applies_the_shared_frameless_shape_and_reports_it",
        "a_press_on_the_chrome_bar_starts_the_os_drag_move",
        "the_three_chrome_controls_request_minimize_maximize_and_exit",
        "the_mounted_shell_publishes_every_shared_semantic_row",
        "the_bevy_surface_reports_no_tray_host_instead_of_a_badge",
    )

    # Shared contracts: the tray rate badge, the chrome shape, the grammar.
    require(
        violations,
        TRAY_STATUS,
        "pub const TRAY_RATE_REFRESH_INTERVAL_MS",
        "pub struct TrayRateBadge",
        "pub fn from_waveform",
        "pub fn badge_text",
        "pub fn format_rate",
        "pub enum TraySupport",
        "pub const fn live_rate_badge",
        "pub const fn unsupported_reason",
        "fn the_rate_badge_projects_the_newest_real_sample",
        "fn an_empty_or_non_finite_waveform_never_reports_a_badge",
        "fn a_surface_without_a_tray_states_the_typed_reason",
    )
    require(
        violations,
        WINDOW_CHROME,
        "pub const CHROME_DRAG_STRIP_HEIGHT_PX",
        "pub struct WindowChrome",
        "pub const FRAMELESS",
        "pub const SYSTEM",
        "pub const fn os_decorations",
        "pub const fn needs_custom_controls",
        "pub enum WindowChromeSupport",
        "fn the_frameless_style_owns_a_real_drag_strip",
        "fn a_host_without_a_drag_path_reports_the_typed_boundary",
    )
    require(
        violations,
        A11Y,
        "pub enum A11yRole",
        "pub enum ShellA11yNode",
        "pub const ALL: [Self; 19]",
        "pub const fn label_key",
        "pub const fn label_zh",
        "pub const fn role",
        "pub fn shell_a11y_specs",
        "fn every_shell_node_carries_a_role_and_both_label_sources",
        "fn the_coverage_inventory_is_unique_and_complete",
    )

    # Iced: the live rate badge on the real tray spec + its throttled push.
    require(
        violations,
        "crates/infiltrator-iced/src/tray/spec.rs",
        "pub const TRAY_ACTION_INFO_RATE",
        "pub rate_badge: Option<infiltrator_contract::tray_status::TrayRateBadge>",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/tray/menu.rs",
        'tr("tray_info_rate")',
        "badge.badge_text()",
        "TRAY_ACTION_INFO_RATE",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/tray.rs",
        "tray_status::TrayRateBadge::from_waveform",
        "pub fn refresh_tray",
        "pub fn refresh_tray_throttled",
        "pub fn refresh_tray_rates",
        "TRAY_RATE_REFRESH_INTERVAL",
        "tray_last_rate_text",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update.rs",
        "self.refresh_tray_rates()",
    )
    # Iced: the frameless host window + the mounted drag strip.
    require(
        violations,
        "crates/infiltrator-iced/src/window_chrome.rs",
        "pub fn chrome",
        "pub fn support",
        "pub fn window_settings",
        "decorations: chrome().os_decorations()",
        "pub enum ChromeRequest",
        "pub const fn from_message",
        "iced::window::drag(id)",
        "iced::window::toggle_maximize(id)",
        "iced::window::minimize(id, true)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/chrome.rs",
        "pub fn chrome_strip",
        "fn strip_height_px",
        "mouse_area",
        "on_double_click",
        "Message::WindowChromeDragRequested",
        "ShellA11yNode::ChromeMinimize",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/chrome.rs",
        "pub(crate) fn update_chrome",
        "ChromeRequest::from_message",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view_root.rs",
        "view::chrome::chrome_strip(self)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/desktop_composition.rs",
        "crate::window_chrome::window_settings",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/demo.rs",
        "crate::window_chrome::window_settings",
    )
    # Iced: the shared grammar consumed as localized labels/tooltips.
    require(
        violations,
        "crates/infiltrator-iced/src/accessibility.rs",
        "pub fn a11y_label",
        "pub fn a11y_role",
        "pub fn labelled",
        "node.label_key()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/sidebar.rs",
        "ShellA11yNode::GlobalStatusDot",
        "ShellA11yNode::TrafficReadout",
        "crate::accessibility::labelled",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/mini_hud.rs",
        "ShellA11yNode::MiniHudCard",
        "ShellA11yNode::MiniHudSystemProxySwitch",
        "ShellA11yNode::MiniHudTunSwitch",
    )

    # Bevy: the real chrome path, the grammar mounting and the tray report.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/chrome.rs",
        "pub struct WindowChromePlugin",
        "pub const fn chrome_shape",
        "pub const fn support",
        "pub struct WindowChromeReport",
        "pub struct ChromeMaximizeLatch",
        "pub fn chrome_bar_scene",
        "window.start_drag_move()",
        "window.set_minimized(true)",
        "window.set_maximized(next)",
        "exits.write(AppExit::Success)",
        "pub struct ChromeDragBar",
        "pub struct ChromeMinimizeButton",
        "pub struct ChromeMaximizeButton",
        "pub struct ChromeCloseButton",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/a11y.rs",
        "pub const fn accesskit_role",
        "pub fn semantic_node",
        "pub fn switch_node",
        "pub fn value_node",
        "node.label_zh()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/tray_status.rs",
        "pub const fn support",
        "bevy-shell-has-no-tray-host",
        "pub struct TrayStatusReport",
        "pub fn live_badge",
        "TRAY_RATE_REFRESH_INTERVAL_MS",
        "pub struct TrayStatusPlugin",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/lib.rs",
        "decorations: chrome::chrome_shape().os_decorations()",
        "chrome::WindowChromePlugin",
        "tray_status::TrayStatusPlugin",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/shell_scene.rs",
        "crate::chrome::chrome_bar_scene(palette)",
        "crate::a11y::{semantic_node, switch_node}",
        "ShellA11yNode::Window",
        "ShellA11yNode::ModeSegment",
        "switch_node(ShellA11yNode::SystemProxySwitch",
        "switch_node(ShellA11yNode::TunSwitch",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/toast.rs",
        "fn sync_toast_semantics",
        "ShellA11yNode::ToastRegion",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud.rs",
        "ShellA11yNode::MiniHudCard",
        "switch_node(ShellA11yNode::MiniHudSystemProxySwitch",
        "ShellA11yNode::MiniHudTunSwitch",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command_palette.rs",
        "ShellA11yNode::CommandPaletteDialog",
    )

    # ---- Batch G (DUAL-15-10/14): the shared interaction palette consumed by
    # both surfaces, the tooltip coverage inventory, and the shared role
    # inventory a surface can iterate instead of hand-listing roles.
    require(
        violations,
        DESIGN_TOKENS,
        "pub struct SkinInteractionPalette",
        "pub const fn skin_interaction(skin: ThemeSkin) -> SkinInteractionPalette",
        "fn every_skin_defines_the_shared_interaction_tokens",
        "fn the_interaction_washes_stay_distinct_across_skins",
    )
    require(
        violations,
        ICED_THEME,
        "LIGHT_INTERACTION",
        "DARK_INTERACTION",
        "FOREST_INTERACTION",
        "AMOLED_INTERACTION",
        "pub hover: Color",
        "pub pressed: Color",
        "pub focus_ring: Color",
        "pub scrim: Color",
    )
    require(
        violations,
        BEVY_THEME,
        "pub scrim: TokenColor",
        "pub focus_ring: TokenColor",
        "pub disabled_ink: TokenColor",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/palette.rs",
        "pub scrim: Color",
        "pub focus_ring: Color",
        "pub disabled_ink: Color",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/components.rs",
        "tk.hover",
        "tk.pressed",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/component_forms.rs",
        "pub fn form_input_style",
        "tk.focus_ring",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_theme_tests.rs",
        "fn the_interaction_tokens_resolve_the_shared_contract",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_components_tests.rs",
        "fn test_interaction_seams_consume_the_shared_tokens",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/design_token_tests.rs",
        "fn the_widget_interaction_tokens_mirror_the_shared_contract",
        "fn the_mirrored_scrim_dimms_the_backdrop_on_every_skin",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/tests/headless/palette_tests.rs",
        "assert_same_color(palette.scrim, theme.scrim)",
        "assert_same_color(palette.focus_ring, theme.focus_ring)",
        "assert_same_color(palette.disabled_ink, theme.disabled_ink)",
    )
    require(
        violations,
        A11Y,
        "pub const ALL: [Self; 10]",
        "fn the_role_inventory_covers_every_variant_once",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/sidebar.rs",
        "ShellA11yNode::SystemProxySwitch",
        "ShellA11yNode::TunSwitch",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/a11y_semantics_tests.rs",
        "for role in A11yRole::ALL",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view_root/modals/card.rs",
        "tokens(t).scrim",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/focus.rs",
        "palette.focus_ring",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/button.rs",
        "palette.disabled_ink",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/adaptive_modal.rs",
        "palette.scrim",
    )

    # DUAL-15-11: the shared IME grammar, the Bevy host wiring, the Iced
    # toolkit boundary, and the dual tests that pin them.
    require(
        violations,
        "crates/infiltrator-contract/src/ime.rs",
        "pub struct ImeCursorRect",
        "pub enum ImeCursorSource",
        "SurfaceComputed",
        "ToolkitProvided",
        "pub enum ImeCursorSupport",
        "pub struct ImeFocusPlan",
        "pub enum ImeCompositionEvent",
        "pub struct ImeCompositionTracker",
        "pub fn announcement",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/ime.rs",
        "pub const fn cursor_support",
        "ImeCursorSource::SurfaceComputed",
        "pub struct ShellImePlugin",
        "pub struct ImeHostReport",
        "Window::ime_enabled",
        "window.ime_position",
        "Ime::Preedit",
        "Ime::Commit",
        "pub fn shared_composition_event",
        "pub fn apply_composition_action",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/app.rs",
        "crate::ime::ShellImePlugin",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/ime.rs",
        "pub const fn cursor_support",
        "ImeCursorSource::ToolkitProvided",
        "pub fn composition_event",
        "pub fn composition_message",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/subscription.rs",
        "crate::ime::composition_message",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/shell.rs",
        "self.shell.ime.is_composing()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "ImeComposition(infiltrator_contract::ime::ImeCompositionEvent)",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command_palette.rs",
        "ShellA11yNode::CommandPaletteDialog",
        "ShellA11yNode::CommandPaletteQuery",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/ime_tests.rs",
        "fn the_focused_field_enables_the_window_ime_at_the_real_caret",
        "fn an_unfocused_shell_disables_the_window_ime_instead_of_tracking_a_stale_caret",
        "fn a_caret_outside_the_window_is_clamped_before_it_reaches_the_os",
        "fn composition_events_reach_the_focused_field_through_the_shared_tracker",
        "fn a_cancelled_composition_rolls_the_field_back",
        "fn composition_for_another_window_is_ignored",
        "fn a_headless_composition_without_a_window_reports_the_plan_honestly",
        "fn a_mounted_shell_keeps_its_ime_disabled_until_a_field_is_focused",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/ime_tests.rs",
        "fn the_shell_reports_the_toolkit_provided_cursor_source",
        "fn the_raw_toolkit_ime_events_map_onto_the_shared_vocabulary",
        "fn a_composition_session_keeps_the_shared_phase_and_never_steals_chords",
        "fn the_pinned_toolkit_forwards_a_widget_caret_to_the_shell_strategy",
        "fn the_palette_query_line_carries_the_shared_label_as_a_visible_tooltip",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/test_mounts.rs",
        "../tests/gui/ime_tests.rs",
    )

    # The dual tests introduced by this batch.
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/multimodal_shell_tests.rs",
        "fn the_tray_spec_carries_the_live_rate_badge_from_the_shared_waveform",
        "fn a_host_without_live_samples_shows_no_rate_badge",
        "fn the_rate_badge_push_is_deduplicated_and_uses_the_shared_interval",
        "fn the_frameless_window_settings_consume_the_shared_chrome_contract",
        "fn every_chrome_message_maps_to_one_real_host_window_op",
        "fn the_chrome_strip_mounts_above_the_shell_and_never_fabricates_a_window",
        "fn every_shared_semantic_node_resolves_a_localized_label_and_role",
        "fn the_shell_carries_the_shared_labels_as_visible_tooltips",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/window_chrome_tests.rs",
        "fn the_host_applies_the_shared_frameless_shape_and_reports_it",
        "fn a_press_on_the_chrome_bar_starts_the_os_drag_move",
        "fn a_double_click_toggles_the_maximize_request_and_the_latch",
        "fn the_three_chrome_controls_request_minimize_maximize_and_exit",
        "fn the_mounted_shell_carries_the_chrome_bar_and_its_controls",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/a11y_semantics_tests.rs",
        "fn the_mounted_shell_publishes_every_shared_semantic_row",
        "fn a_switch_node_announces_its_live_state_and_a_status_node_its_value",
        "fn every_shared_role_maps_onto_a_real_accesskit_role",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/tray_status_tests.rs",
        "fn the_bevy_surface_reports_no_tray_host_instead_of_a_badge",
        "fn the_report_carries_the_shared_refresh_cadence",
    )

    # ---- Batch H (DUAL-15-07): the shared touch-gesture semantic contract,
    # the host touch seam, and the real Bevy-shell touch consumer. Iced keeps
    # the typed-unsupported boundary (no touch host on the desktop surface).
    require(
        violations,
        LEDGER,
        "2026-09-23 组 15 批次 H",
        "shell_gesture",
        "GestureSemanticEvent",
        "GestureSnapshot",
        "TouchGestureSupport",
        "SafeAreaInsets",
        "TouchGesturePort",
        "TouchGestureHostReport",
        "ShellGesturePlugin",
        "ShellGestureSnapshot",
        "MessageReader<TouchInput>",
        "a_pull_release_maps_to_a_bounded_semantic_event",
        "a_host_that_does_not_declare_touch_reports_typed_unsupported",
        "a_downward_drag_from_the_top_band_maps_to_pull_to_refresh",
        "a_two_finger_sequence_publishes_a_pinch_event",
        "a_desktop_surface_reports_no_touch_gesture_host",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/shell_gesture.rs",
        "pub enum GestureTouchPhase",
        "pub enum GestureSemanticEvent",
        "pub struct GestureSnapshot",
        "pub enum TouchGestureSupport",
        "pub struct SafeAreaInsets",
        "pub fn pull_to_refresh(",
        "pub fn swipe_to_action(",
        "pub fn pinch(",
        "fn a_pull_release_maps_to_a_bounded_semantic_event",
        "fn a_host_without_touch_reports_a_typed_reason",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/touch_gesture.rs",
        "pub trait TouchGesturePort",
        "pub struct TouchGestureHostReport",
        "fn a_host_that_does_not_declare_touch_reports_typed_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/gesture.rs",
        "pub struct ShellGesturePlugin",
        "pub struct ShellGestureSnapshot",
        "pub struct GestureHostReport",
        "pub const fn touch_support()",
        "pub fn shared_phase(",
        "pub fn shared_outcome(",
        "MessageReader<TouchInput>",
        "GestureRecognizer",
        "GestureSemanticEvent::pull_to_refresh(",
        "GestureSemanticEvent::swipe_to_action(",
        "GestureSemanticEvent::pinch(",
        "GestureSemanticEvent::SafeAreaInsets(",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/app.rs",
        "crate::gesture::ShellGesturePlugin",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/lib.rs",
        "pub mod gesture",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/gesture.rs",
        "pub const fn touch_support()",
        "iced-desktop-surface-has-no-touch-gesture-host",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/gesture_tests.rs",
        "fn a_tap_and_a_double_tap_map_through_the_widget_recognizer",
        "fn a_downward_drag_from_the_top_band_maps_to_pull_to_refresh",
        "fn a_two_finger_sequence_publishes_a_pinch_event",
        "fn host_declared_insets_flow_into_the_shared_snapshot",
        "fn the_mounted_shell_consumes_touch_through_the_shared_recognizer",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/gesture_tests.rs",
        "fn a_desktop_surface_reports_no_touch_gesture_host",
    )

    # DUAL-15-07: the closed claim must keep a real Bevy touch consumer.
    check_touch_gesture_consumer(violations)

    # DUAL-15-10 label coverage + the two honesty boundaries (no fake
    # AccessKit on Iced, no fake tray on Bevy).
    check_a11y_label_coverage(violations)
    check_iced_tooltip_coverage(violations)
    check_capability_honesty(violations)

    # DUAL-15-14: the interaction/overlay palette is shared and consumed by
    # name on both surfaces; raw literal spellings fail the gate.
    check_interaction_token_mirrors(violations)

    # Fixed defects must not regress.
    forbid(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "hotkeys_config",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "UpdateHotkeyCombo",
        "RemoveToast(usize)",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/toast_state.rs",
        "elapsed <= 2000",
        "max_visible: usize,\n    active_toasts",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/appearance.rs",
        "ThemeSkin::Light => ThemeSkin::Dark",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/command_palette.rs",
        "ThemeSkin::Light => ThemeSkin::Dark",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/view/theme.rs",
        'value.trim().to_ascii_lowercase().as_str() {\n        "forest"',
    )
    # Batch B regressions: the HUD must stay read-model driven, the palette
    # must stay catalogue driven, and the Iced command model must stay shared.
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/mini_hud.rs",
        "系统代理: 开启",
        'Text({ "RULE".to_owned() })',
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/types/app.rs",
        "pub enum CommandAction",
        "pub struct CommandItem",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/command_palette.rs",
        "Vec<PaletteAction>",
        "pub enum PaletteCategory",
        "pub struct PaletteAction",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "CommandAction",
    )

    if args.mode == "enforce" and violations:
        print("multimodal-shell guard: FAIL", file=sys.stdout)
        for violation in violations:
            print(f"  - {violation}")
        return 1
    print(
        "multimodal-shell guard: "
        f"ledger_rows=15 skins={sorted(contract_skins)} violations={len(violations)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
