#!/usr/bin/env python3
"""SLINT-007: one surface ladder, one elevation language, every skin.

`LumenTheme.zinc-*` is the only surface tier vocabulary in the Slint tree, so
its meaning has to be the same in all four skins. Two failures were measured
from published receipts, not guessed:

  * The paper ladder used to run *backwards*: `zinc-0` was the lightest paper
    and `zinc-3` a darker grey, so a card painted 10 luminance steps UNDER its
    own page in light/forest while the same token painted 21 steps OVER the
    page in dark. The FlClash reference separates white cards from a warm-grey
    page; a sinking card has no such separation.
  * Hover / active expressions used tiers that differed by 4 luminance steps
    in one skin, so the state answered nothing there.
  * The rail was painted by a colour the HOST recomputed: `sidebar-background`
    was piped from Rust through three components and built from a hardcoded
    pair `(0.09, 0.09, 0.11)` / `(0.95, 0.95, 0.96)`. That pair is not a tier
    of this ladder, so forest and amoled painted the dark rail, and every
    focus change could repaint it off-ladder again.

The guard reads `theme.slint` as the single owner and checks the four roles
the tree actually consumes: page -> card climb, a clear card/inset boundary,
a visible hover lift, and a visible pressed drop. It also resolves every
`has-hover` tier pair in the tree so no state collapses to a no-op.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest
import unittest.mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
UI = ROOT / "crates/infiltrator-slint/ui"
THEME = UI / "components/theme.slint"
CARDS = UI / "components/cards.slint"
SKINS = ("amoled", "forest", "dark", "light")
TIER = re.compile(
    r"out property <color> zinc-(\d):\s*"
    r"root\.amoled-mode \? (#(?:[0-9a-fA-F]){6})\s*:\s*"
    r"root\.forest-mode \? (#(?:[0-9a-fA-F]){6})\s*:\s*"
    r"root\.dark-mode \? (#(?:[0-9a-fA-F]){6})\s*:\s*"
    r"(#(?:[0-9a-fA-F]){6});"
)
PAIR = re.compile(
    r"LumenTheme\.(zinc-\d|surface-muted|surface-raised|surface|floating-surface)\b"
)
STATE = re.compile(r"\b(?:has-hover|has-pressed|selected|active)\b")
CARD_GEOMETRY = re.compile(
    r"^\s*in property <length> (inset|vertical-padding|radius|content-spacing):\s*([^;]+);"
)
CARD_GEOMETRY_OWNER = {
    "inset": "LumenTheme.card-inset",
    "vertical-padding": "LumenTheme.card-vertical-padding",
    "radius": "LumenTheme.card-radius",
    "content-spacing": "LumenTheme.card-content-gap",
}
MIN_DELTA = 5.0
ROLE_TO_TIER = {
    "floating-surface": 2,
    "surface": 3,
    "surface-muted": 4,
    "surface-raised": 5,
}


def without_comments(text: str) -> str:
    """Blank out Slint line comments so prose cannot satisfy or trip a rule."""
    out: list[str] = []
    index = 0
    length = len(text)
    while index < length:
        if text.startswith("//", index):
            end = text.find("\n", index)
            end = length if end < 0 else end
            out.append(" " * (end - index))
            index = end
        elif text.startswith("/*", index):
            end = text.find("*/", index + 2)
            end = length if end < 0 else end + 2
            out.append("".join("\n" if ch == "\n" else " " for ch in text[index:end]))
            index = end
        else:
            out.append(text[index])
            index += 1
    return "".join(out)


def luminance(color: str) -> float:
    value = int(color[1:], 16)
    return (0.2126 * ((value >> 16) & 0xFF)
            + 0.7152 * ((value >> 8) & 0xFF)
            + 0.0722 * (value & 0xFF))


def tiers(text: str) -> dict[str, list[float]]:
    found: dict[str, list[tuple[int, str]]] = {skin: [] for skin in SKINS}
    for match in TIER.finditer(text):
        tier = int(match.group(1))
        for skin, color in zip(SKINS, match.groups()[1:]):
            found[skin].append((tier, color))
    return {
        skin: [luminance(color) for _tier, color in sorted(steps)]
        for skin, steps in found.items()
    }


def tier_of(token: str) -> int:
    return int(token.split("-")[1]) if token.startswith("zinc-") else ROLE_TO_TIER[token]


def state_pairs(ui: pathlib.Path, ladder: dict[str, list[float]]) -> list[str]:
    problems = []
    for path in sorted(ui.rglob("*.slint")):
        if path == THEME:
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if "background:" not in line or not STATE.search(line):
                continue
            tokens = PAIR.findall(line)
            if len(tokens) < 2 or any(
                not (token.startswith("zinc-") or token in ROLE_TO_TIER)
                for token in tokens
            ):
                continue
            first, second = (tier_of(token) for token in tokens[:2])
            for skin_index, skin in enumerate(SKINS):
                delta = abs(ladder[skin][first] - ladder[skin][second])
                if delta < MIN_DELTA:
                    label = path.relative_to(ROOT) if path.is_absolute() else path
                    problems.append(
                        f"{label}:{number}: {tokens[0]}/{tokens[1]} "
                        f"collapse in {skin} ({delta:.1f} luminance)"
                    )
                    break
    return problems


def card_geometry(cards_text: str) -> list[str]:
    found: list[str] = []
    component = None
    for number, line in enumerate(cards_text.splitlines(), 1):
        match = re.match(r"export component (\w+)", line)
        if match:
            component = match.group(1)
        geometry = CARD_GEOMETRY.match(line)
        if component in {"SectionCard", "SurfaceCard"} and geometry and \
                geometry.group(2).strip() != CARD_GEOMETRY_OWNER[geometry.group(1)]:
            found.append(
                f"cards.slint:{number}: {component} owns a second {geometry.group(1)}; "
                f"read {CARD_GEOMETRY_OWNER[geometry.group(1)]}"
            )
    return found


def violations(theme_text: str, ui: pathlib.Path | None = None) -> list[str]:
    ladder = tiers(theme_text)
    found: list[str] = []
    for skin in SKINS:
        ramp = ladder[skin]
        if len(ramp) != 8:
            found.append(f"theme.slint: {skin} declares {len(ramp)} of 8 tiers")
            continue
        if not all(ramp[i + 1] > ramp[i] for i in range(3)):
            found.append(f"theme.slint: {skin} page->card ladder does not climb")
        if abs(ramp[3] - ramp[4]) < MIN_DELTA:
            found.append(f"theme.slint: {skin} has no card/inset boundary")
        if abs(ramp[5] - ramp[4]) < MIN_DELTA:
            found.append(f"theme.slint: {skin} hover tier answers nothing")
        if abs(ramp[6] - ramp[4]) < MIN_DELTA:
            found.append(f"theme.slint: {skin} pressed tier answers nothing")
    if ui is not None:
        found.extend(state_pairs(ui, ladder))
        if isinstance(ui, pathlib.Path):
            found.extend(card_geometry((ui / "components/cards.slint").read_text(encoding="utf-8")))
            desktop = (ui / "pages/home_desktop_grid.slint").read_text(encoding="utf-8")
            if "min-width: LumenTheme.policy-card-min-width;" not in desktop:
                found.append("pages/home_desktop_grid.slint: policy card min-width left the theme")
            src = ROOT / "crates/infiltrator-slint/src"
            for path in sorted(src.rglob("*.rs")):
                text = path.read_text(encoding="utf-8")
                if "sidebar_background" in text or "project_slint_sidebar_wash" in text:
                    found.append(
                        f"{path.relative_to(ROOT)}: the host paints the rail again instead of the ladder"
                    )
            for path in sorted(ui.rglob("*.slint")):
                text = without_comments(path.read_text(encoding="utf-8"))
                if "sidebar-background" in text:
                    found.append(
                        f"{path.relative_to(ui)}: a rail colour is piped through the host again"
                    )
    return found


class LadderSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        # Neutral greys, so the hex value IS the luminance. Roles: page 30,
        # 50, 70, card 90; inset 50, hover 110, pressed 30, deep 10.
        values = (30, 50, 70, 90, 50, 110, 30, 10)
        def grey(tier: int, offset: int) -> str:
            value = min(255, values[tier] + offset)
            return f"#{value:02X}{value:02X}{value:02X}"
        self.good = "\n".join(
            f"out property <color> zinc-{tier}: root.amoled-mode ? {grey(tier, 0)}"
            f" : root.forest-mode ? {grey(tier, 40)}"
            f" : root.dark-mode ? {grey(tier, 0)}"
            f" : {grey(tier, 20)};"
            for tier in range(8)
        )

    def test_broken_paper_ladder_is_caught(self) -> None:
        # Sink only forest's card below its own page: the caller-visible
        # `zinc-3` token still exists, but it no longer means "raised" there.
        lines = self.good.splitlines()
        lines[3] = lines[3].replace("root.forest-mode ? #828282", "root.forest-mode ? #1E1E1E")
        broken = "\n".join(lines)
        self.assertTrue(any("forest page->card" in item for item in violations(broken)), violations(broken))

    def test_good_ladder_passes(self) -> None:
        self.assertEqual(violations(self.good), [])

    def test_dead_hover_pair_is_caught(self) -> None:
        class Mirror(dict):
            def rglob(self, _pattern: str):
                return [ROOT / "fake.slint"]

        def with_line(text: str) -> list[str]:
            original = pathlib.Path.read_text
            with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
                def fake(path: pathlib.Path, *args, **kwargs):
                    return text if path.name == "fake.slint" else original(path, *args, **kwargs)
                read_text.side_effect = fake
                return violations(self.good, Mirror())

        clean_line = "background: (touch.has-hover && LumenTheme.hover-allowed) ? LumenTheme.zinc-5 : LumenTheme.zinc-4;"
        dead_line = clean_line.replace("zinc-5", "zinc-4")
        self.assertEqual(with_line(clean_line), [])
        self.assertTrue(any("collapse" in item for item in with_line(dead_line)))

    def test_host_piped_rail_colour_is_caught(self) -> None:
        """The rail tier has one owner: the ladder, never a host-side colour."""
        rail = UI / "components/sidebar.slint"
        original = rail.read_text(encoding="utf-8")
        assert "background: LumenTheme.zinc-2;" in original
        planted = original.replace(
            "background: LumenTheme.zinc-2;",
            "in property <color> sidebar-background;\n    background: root.sidebar-background;",
            1,
        )
        real = pathlib.Path.read_text

        def fake(self, *args, **kwargs):
            if self == rail:
                return planted
            return real(self, *args, **kwargs)

        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            read_text.side_effect = fake
            findings = violations(self.good, UI)
        self.assertTrue(
            any("rail colour is piped through the host" in item for item in findings),
            findings,
        )

    def test_mixed_role_and_tier_pairs_are_not_skipped(self) -> None:
        class Mirror(dict):
            def rglob(self, _pattern: str):
                return [ROOT / "fake.slint"]

        original = pathlib.Path.read_text
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake(path: pathlib.Path, *args, **kwargs):
                dead = "background: (t.has-hover) ? LumenTheme.zinc-3 : LumenTheme.surface;"
                return dead if path.name == "fake.slint" else original(path, *args, **kwargs)
            read_text.side_effect = fake
            self.assertTrue(any("collapse" in item for item in violations(self.good, Mirror())))

    def test_card_geometry_has_one_owner(self) -> None:
        good = "export component SectionCard inherits Rectangle {\n" \
               "    in property <length> inset: LumenTheme.card-inset;\n" \
               "}\n"
        self.assertEqual(card_geometry(good), [])
        bad = good.replace("LumenTheme.card-inset", "12 * LumenUiScale.unit")
        self.assertTrue(any("second inset" in item for item in card_geometry(bad)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(LadderSelfTest)
        return 0 if unittest.TextTestRunner(verbosity=0).run(suite).wasSuccessful() else 1
    found = violations(THEME.read_text(encoding="utf-8"), UI)
    for item in found:
        print(f"SLINT-007 [{args.mode}]: {item}")
    print(f"slint surface ladder guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
