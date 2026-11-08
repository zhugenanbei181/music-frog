#!/usr/bin/env python3
"""SLINT-006: one typed mobile dock band and one floating pill.

The published receipts are the proof: a 400x800 mobile frame may not paint a
vertical scrollbar, the `as-needed` policy may only remain on desktop, and the
mascot component must clip its character, notes and superimposed control to a
single rounded hit surface.

The bottom dock must stay floating: only the pill itself may be opaque dock
geometry. A full-width plinth or pill-wide scrim cuts scrolling cards in half
and turns a floating control into a covering bar. The typed `MobileDockBand
State` is the one authority for both the reserved content band and the mounted
dock; a `present` flag combined with a `visible` flag is the retired two-truth
state that lets content leak through the transparent strip.
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
CONTRACT = ROOT / "crates/infiltrator-contract/src/mobile_chrome.rs"
PRESENTATION = ROOT / "crates/infiltrator-presentation/src/mobile_chrome_projection.rs"
MOBILE_MOBILE_POLICY = re.compile(
    r"vertical-scrollbar-policy:\s*LumenTheme\.compact-layout\s*\?\s*ScrollBarPolicy\.always-off\s*:\s*ScrollBarPolicy\.as-needed"
)
AS_NEEDED = re.compile(r"vertical-scrollbar-policy:\s*ScrollBarPolicy\.as-needed")
AS_NEEDED_HORIZONTAL = re.compile(r"horizontal-scrollbar-policy:\s*ScrollBarPolicy\.as-needed")
BOTTOM_PLINTH = re.compile(
    r"Rectangle\s*\{\s*x:\s*0;\s*width:\s*parent\.width;\s*height:\s*parent\.height;\s*background:\s*LumenTheme\.zinc-0;",
    re.S,
)
DOCK_BACKDROP = re.compile(r"^\s*dock-backdrop\s*:=\s*Rectangle\s*\{", re.M)
DOCK_SCRIM = re.compile(
    r"(?<!component MobileDockPageGround inherits )Rectangle\s*\{[^{}]*?background:\s*LumenTheme\.zinc-0;[^{}]*?\}",
    re.S,
)
PAGE_GROUND = re.compile(
    r"export component MobileDockPageGround inherits Rectangle \{"
    r"[^{}]*?y:\s*root\.window-height\s*-\s*self\.height;"
    r"[^{}]*?background:\s*LumenTheme\.zinc-0;[^{}]*?\}",
    re.S,
)
RETIRED_STAGE_FLAGS = re.compile(
    r"bottom-bar-present|bottom-bar-visible|hide-bottom-bar-on-scroll|"
    r"set_hide_bottom_bar_on_scroll"
)
TYPED_STATE_BINDING = re.compile(
    r"in property <MobileDockBandState>\s+mobile-dock-band-state"
)


def mask_slint_comments(source: str) -> str:
    result: list[str] = []
    index = 0
    in_block = False
    while index < len(source):
        if not in_block and source[index : index + 2] == "//":
            end = source.find("\n", index)
            if end == -1:
                break
            result.append("\n")
            index = end + 1
        elif not in_block and source[index : index + 2] == "/*":
            in_block = True
            index += 2
        elif in_block and source[index : index + 2] == "*/":
            in_block = False
            index += 2
        else:
            result.append("" if in_block else source[index])
            index += 1
    return "".join(result)


def source_violations(ui: pathlib.Path, contract: pathlib.Path, presentation: pathlib.Path) -> list[str]:
    found: list[str] = []
    files = list(sorted(ui.rglob("*.slint")))
    live_code = "\n".join(
        mask_slint_comments(path.read_text(encoding="utf-8")) for path in files
    )
    if RETIRED_STAGE_FLAGS.search(live_code):
        found.append("retired dock stage flags came back; consume one typed MobileDockBandState")
    if TYPED_STATE_BINDING.search(live_code) is None:
        found.append("no UI surface consumes the typed MobileDockBandState")
    contract_code = contract.read_text(encoding="utf-8")
    projection_code = presentation.read_text(encoding="utf-8")
    if "pub enum MobileDockBandState" not in contract_code or "Closed" not in contract_code or "Reserved" not in contract_code:
        found.append("the contract no longer owns the closed/reserved dock states")
    if "pub const fn transition(" not in contract_code or "match (self, intent)" not in contract_code:
        found.append("the dock contract lost its exhaustive typed transition")
    if "project_mobile_dock_band" not in projection_code or "intent_for_mobile_dock_band" not in projection_code:
        found.append("the shared presentation projection no longer folds dock facts")
    chrome = mask_slint_comments(
        (ui / "components/app_chrome.slint").read_text(encoding="utf-8")
    )
    if "export component MobileDockPageGround" not in chrome:
        found.append("the reserved band has no typed page-ground owner")
    if PAGE_GROUND.search(chrome) is None:
        found.append("MobileDockPageGround no longer paints only the bottom page ground")
    body = mask_slint_comments(
        (ui / "components/app_body.slint").read_text(encoding="utf-8")
    )
    if "MobileDockBandProjection.reserves_band(root.mobile-dock-band-state)" not in body:
        found.append("the body no longer reads the typed dock band state")
    if "padding-bottom: root.dock-band-height;" not in body:
        found.append("the body no longer subtracts exactly one derived band")
    app = mask_slint_comments((ui / "lumen-app.slint").read_text(encoding="utf-8"))
    if "MobileDockBandProjection.reserves_band(root.mobile-dock-band-state): MobileDockPageGround" not in app:
        found.append("the page ground is not mounted from the typed state")
    if "MobileDockBandProjection.draws_dock(root.mobile-dock-band-state): AppBottomBar" not in app:
        found.append("the dock pill is not mounted from the typed state")
    return found


def violations(
    ui: pathlib.Path,
    contract: pathlib.Path = CONTRACT,
    presentation: pathlib.Path = PRESENTATION,
) -> list[str]:
    found: list[str] = []
    found.extend(source_violations(ui, contract, presentation))
    def display(path: pathlib.Path) -> str:
        try:
            return str(path.relative_to(ROOT))
        except ValueError:
            return str(path.relative_to(ui.parent))

    for path in sorted(ui.rglob("*.slint")):
        text = path.read_text(encoding="utf-8")
        relative = display(path)
        if AS_NEEDED.search(text) and not MOBILE_MOBILE_POLICY.search(text):
            found.append(f"{relative}: vertical rail can appear on a compact layout")
        if AS_NEEDED_HORIZONTAL.search(text) and "LumenTheme.compact-layout" not in text:
            found.append(f"{relative}: horizontal rail can appear on a compact layout")
    mascot = (ui / "components/mascot.slint").read_text(encoding="utf-8")
    if "clip: true;" not in mascot or "border-radius: self.width / 2;" not in mascot:
        found.append("components/mascot.slint: hero no longer clips to one circle")
    if "y: parent.height * 0.62;" not in mascot or "x: (parent.width - self.width) / 2;" not in mascot:
        found.append("components/mascot.slint: control is no longer superimposed on the character")
    if "width: parent.width;" not in mascot or "transform-rotation: root.spec.sway * 2deg;" not in mascot:
        found.append("components/mascot.slint: a transformed frog can escape the asset-owned circle")
    if "in property <length> disc-size:" not in mascot:
        found.append("components/mascot.slint: the mascot lost its single size owner")
    for path in sorted(ui.rglob("*.slint")):
        text = path.read_text(encoding="utf-8")
        relative = display(path)
        if "hero-scale" in text or "button-size" in text:
            found.append(f"{relative}: the retired mascot size entry points are still in the tree")
    chrome = (ui / "components/app_chrome.slint").read_text(encoding="utf-8")
    bottom_bar = chrome.split("export component ClearTrafficDialog", 1)[0]
    title_bar = chrome.split("export component DesktopTitleBar", 1)[-1]
    if BOTTOM_PLINTH.search(chrome):
        found.append("components/app_chrome.slint: full-width opaque plinth covers the scrolling page")
    opaque_ground = chrome.split("export component AppBottomBar", 1)[0]
    if DOCK_BACKDROP.search(bottom_bar) or DOCK_SCRIM.search(bottom_bar.replace(opaque_ground, "", 1)):
        found.append("components/app_chrome.slint: opaque dock backdrop covers the scrolling page")
    if "background: transparent;" not in bottom_bar:
        found.append("components/app_chrome.slint: bottom strip paints a covering band")
    if "min(LumenTheme.bottom-bar-max-width," not in bottom_bar or \
            "drop-shadow-color: LumenTheme.dock-shadow-color;" not in bottom_bar:
        found.append("components/app_chrome.slint: floating dock geometry left the theme")
    if "border-color: LumenTheme.dock-rim-color;" not in bottom_bar:
        found.append("components/app_chrome.slint: floating dock has no glass rim")
    # ONE drag region: a second raw TouchArea used to cover the traffic
    # capsule and the search button with a different double-click-only
    # behaviour, and the two regions disagreed about x=896..1039.
    if title_bar.count("WindowMoveArea {") != 1:
        found.append("components/app_chrome.slint: the desktop title bar has no single drag owner")
    if "double-clicked =>" in title_bar:
        found.append("components/app_chrome.slint: the title bar has a second double-click drag surface")
    if "search-btn.visible ? search-btn.x : title-buttons.x" not in title_bar:
        found.append("components/app_chrome.slint: the drag band no longer ends at the first interactive control")
    workbench = (ui / "pages" / "page_workbench.slint").read_text(encoding="utf-8")
    if "min-height: panel-content.min-height;" not in workbench:
        found.append("pages/page_workbench.slint: the workbench card no longer hugs its own content")
    # The overview reorder action exists in two compact layouts. It used to
    # spell its own height twice (44u and 48u), and 44u is below the touch
    # minimum this surface publishes, so one action had two bodies.
    for page in ("page_home.slint", "home_compact_flow.slint"):
        text = (ui / "pages" / page).read_text(encoding="utf-8")
        if "调整卡片顺序" not in text:
            found.append(f"pages/{page}: the reorder action left without a replacement owner")
        elif "height: LumenTheme.min-touch-target;" not in text:
            found.append(f"pages/{page}: the page action height left the touch-target owner")
    sync = (ui / "pages" / "page_sync.slint").read_text(encoding="utf-8")
    if "spacing: 6 * LumenUiScale.unit;" in sync:
        found.append("pages/page_sync.slint: a per-item loop pays a phantom gap for non-matching items")
    return found


class GuardSelfTest(unittest.TestCase):
    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(violations(UI), [])

    def test_two_truth_stage_flags_are_caught(self) -> None:
        planted = mask_slint_comments(
            (UI / "lumen-app.slint").read_text(encoding="utf-8")
        ) + "\nproperty <bool> bottom-bar-present: true;"
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == UI / "lumen-app.slint":
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            planted_findings = violations(UI)
        self.assertTrue(any("retired dock stage flags" in item for item in planted_findings))

    def test_transparent_band_page_ground_must_exist(self) -> None:
        chrome = (UI / "components/app_chrome.slint").read_text(encoding="utf-8")
        planted = chrome.replace(
            "background: LumenTheme.zinc-0;\n}\n\nexport component AppBottomBar",
            "background: transparent;\n}\n\nexport component AppBottomBar",
            1,
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == UI / "components/app_chrome.slint":
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            planted_findings = violations(UI)
        self.assertTrue(any("page-ground owner" in item or "only the bottom page ground" in item for item in planted_findings))

    def test_old_bool_geometry_reflow_is_caught(self) -> None:
        body = (UI / "components/app_body.slint").read_text(encoding="utf-8")
        planted = body.replace(
            "padding-bottom: root.dock-band-height;",
            "padding-bottom: root.bottom-bar-present ? root.dock-band-height : 0 * LumenUiScale.unit;",
            1,
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == UI / "components/app_body.slint":
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            planted_findings = violations(UI)
        self.assertTrue(
            any("retired dock stage flags" in item or "subtracts exactly one" in item for item in planted_findings)
        )

    def test_planted_desktop_only_policy_is_caught(self) -> None:
        class Mirror(dict):
            def rglob(self, _pattern: str):
                return [UI / "components/app_body.slint"]

            def __truediv__(self, _name: str):
                return UI / "components/mascot.slint"

        planted = (UI / "components/app_body.slint").read_text(encoding="utf-8").replace(
            "vertical-scrollbar-policy: LumenTheme.compact-layout ? ScrollBarPolicy.always-off : ScrollBarPolicy.as-needed;",
            "vertical-scrollbar-policy: ScrollBarPolicy.as-needed;",
            1,
        )
        original = (UI / "components/app_body.slint").read_text(encoding="utf-8")
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path.name == "app_body.slint":
                    return planted
                return original if path.name == "mascot.slint" else ""

            read_text.side_effect = fake_read_text
            planted_findings = violations(Mirror())
        self.assertTrue(any("app_body.slint" in item for item in planted_findings))

    def test_full_width_plinth_is_caught(self) -> None:
        chrome = (UI / "components/app_chrome.slint").read_text(encoding="utf-8")
        planted = "Rectangle { x: 0; width: parent.width; height: parent.height; background: LumenTheme.zinc-0; }"
        self.assertFalse(BOTTOM_PLINTH.search(chrome))
        self.assertTrue(BOTTOM_PLINTH.search(planted))

    def test_pill_wide_scrim_is_caught(self) -> None:
        planted = """dock-backdrop := Rectangle {
    x: root.dock-x - 12 * LumenUiScale.unit;
    width: root.dock-width + 24 * LumenUiScale.unit;
    background: LumenTheme.zinc-0;
}"""
        self.assertTrue(DOCK_BACKDROP.search(planted))
        self.assertTrue(DOCK_SCRIM.search(planted))

    def test_typed_page_ground_is_not_a_scrim(self) -> None:
        chrome = (UI / "components/app_chrome.slint").read_text(encoding="utf-8")
        ground = chrome.split("export component AppBottomBar", 1)[0]
        self.assertTrue(PAGE_GROUND.search(ground))
        self.assertFalse(DOCK_SCRIM.search(ground))


ORIGINAL_READ_TEXT = pathlib.Path.read_text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--ui-root", type=pathlib.Path, default=UI)
    parser.add_argument("--contract", type=pathlib.Path, default=CONTRACT)
    parser.add_argument("--presentation", type=pathlib.Path, default=PRESENTATION)
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        return 0 if unittest.TextTestRunner(verbosity=0).run(suite).wasSuccessful() else 1
    found = violations(args.ui_root, args.contract, args.presentation)
    for item in found:
        print(f"SLINT-006 [{args.mode}]: {item}")
    print(f"slint mobile chrome guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
