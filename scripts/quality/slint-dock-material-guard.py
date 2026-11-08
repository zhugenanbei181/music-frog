#!/usr/bin/env python3
"""SLINT-DOCK-MATERIAL: machine-locked floating capsule material decision.

Locks down the bottom floating dock capsule material into a machine-enforced
decision, eliminating repeated flipping between opaque slab and translucent veil.

1. Contract owner:
   `crates/infiltrator-contract/src/mobile_chrome.rs` owns `MobileDockMaterial`
   and `VEIL_ALPHA` (0x00 < VEIL_ALPHA < 0xFF, currently 0x99 = 60%).
2. Slint theme:
   `crates/infiltrator-slint/ui/components/theme.slint` defines `floating-surface`
   with 8-digit `#RRGGBBAA` literals across all 4 skins, with the exact same
   alpha byte matching `VEIL_ALPHA`.
3. Material separation:
   `floating-surface` RGB must not equal `zinc-3` card tier color for tinted skins
   (no substituting dark card background as a fake veil).
4. Consumer tree:
   `crates/infiltrator-slint/ui/components/app_chrome.slint` `AppBottomBar`:
   - Dock capsule background binds `LumenTheme.floating-surface`.
   - Dock capsule border-color binds `LumenTheme.dock-rim-color`.
   - No hardcoded color literals (`#RGB`, `#RRGGBB`, `#RRGGBBAA`) or `.with-alpha`/`alpha(`.
   - Band itself has `background: transparent;`.
   - No full-width plinth, `dock-backdrop`, or `zinc-0` covering background.
5. Glass rim:
   `dock-rim-color` exists in theme and all skins have translucent alpha (0x00 < alpha < 0xFF).
6. Published capture receipt verification (optional/advisory):
   If `.evidence/captures/slint/<latest>/` exists, sample pixels within the capsule
   (x=120..280, y=735..745) on `overview-mobile-dark` / `overview-mobile-light`.
   Measured gray levels must lie strictly between page ground and opaque fill,
   and differ by >= 6 gray levels from opaque fill.
"""

from __future__ import annotations

import argparse
from collections import Counter
import pathlib
import re
import sys
import unittest
import unittest.mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
UI = ROOT / "crates/infiltrator-slint/ui"
THEME = UI / "components/theme.slint"
CHROME = UI / "components/app_chrome.slint"
CONTRACT = ROOT / "crates/infiltrator-contract/src/mobile_chrome.rs"
CAPTURES = ROOT / ".evidence/captures/slint"
FULL_MATRIX_SIZE = 197

SKINS = ("amoled", "forest", "dark", "light")

HEX_COLOR_RE = re.compile(r"#[0-9a-fA-F]{3,8}\b")
HEX8_RE = re.compile(r"^#([0-9a-fA-F]{6})([0-9a-fA-F]{2})$")


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


def extract_component(source: str, component_name: str) -> str | None:
    pattern = re.compile(rf"\bcomponent\s+{component_name}\b[^{{]*\{{")
    match = pattern.search(source)
    if not match:
        return None
    start = match.end() - 1
    depth = 0
    for i in range(start, len(source)):
        char = source[i]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[match.start() : i + 1]
    return None


def extract_sub_block(source: str, prefix_pattern: str) -> str | None:
    match = re.search(prefix_pattern, source)
    if not match:
        return None
    start = match.end() - 1
    depth = 0
    for i in range(start, len(source)):
        char = source[i]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[match.start() : i + 1]
    return None


def parse_skin_colors(expr: str) -> dict[str, str]:
    amoled_m = re.search(r"root\.amoled-mode\s*\?\s*([^:?]+?)(?:\s*:|\s*\))", expr)
    forest_m = re.search(r"root\.forest-mode\s*\?\s*([^:?]+?)(?:\s*:|\s*\))", expr)
    dark_m = re.search(r"root\.dark-mode\s*\?\s*([^:?]+?)(?:\s*:|\s*\))", expr)
    last_colon = expr.rfind(":")
    fallback = expr[last_colon + 1 :].strip(" ();\n\r\t") if last_colon != -1 else ""
    return {
        "amoled": amoled_m.group(1).strip(" ();\n\r\t") if amoled_m else "",
        "forest": forest_m.group(1).strip(" ();\n\r\t") if forest_m else "",
        "dark": dark_m.group(1).strip(" ();\n\r\t") if dark_m else "",
        "light": fallback,
    }


def check_contract(contract_path: pathlib.Path) -> tuple[int | None, list[str]]:
    found: list[str] = []
    if not contract_path.is_file():
        found.append(f"contract: 契约文件不存在: {contract_path}")
        return None, found

    text = contract_path.read_text(encoding="utf-8")
    if "MobileDockMaterial" not in text:
        found.append("contract: mobile_chrome.rs 未定义 MobileDockMaterial 材质类型")

    alpha_m = re.search(
        r"\bpub\s+const\s+VEIL_ALPHA\s*:\s*u8\s*=\s*(0x[0-9a-fA-F]+|\d+)\s*;",
        text,
    )
    if not alpha_m:
        found.append("contract: mobile_chrome.rs 未定义 pub const VEIL_ALPHA: u8 常量")
        return None, found

    veil_alpha = int(alpha_m.group(1), 0)
    if not (0x00 < veil_alpha < 0xFF):
        found.append(
            f"contract: VEIL_ALPHA (0x{veil_alpha:02X}) 必须满足 0x00 < VEIL_ALPHA < 0xFF"
            "（必须为半透明玻璃帷幕，不可为实心板 0xFF 或全透明 0x00）"
        )
    return veil_alpha, found


def check_theme(theme_path: pathlib.Path, contract_alpha: int | None) -> list[str]:
    found: list[str] = []
    if not theme_path.is_file():
        found.append(f"theme.slint: 主题文件不存在: {theme_path}")
        return found

    text = mask_slint_comments(theme_path.read_text(encoding="utf-8"))

    # 1. Check floating-surface
    fs_m = re.search(r"\bout\s+property\s*<\s*color\s*>\s*floating-surface\s*:\s*([^;]+);", text, re.S)
    if not fs_m:
        found.append("theme.slint: 未定义 out property <color> floating-surface 属性")
        return found

    fs_expr = fs_m.group(1).strip()
    if re.search(r"\b(zinc-3|surface)\b", fs_expr):
        found.append("theme.slint: floating-surface 属性表达式不得直接引用 zinc-3 或 surface 卡片层")

    fs_colors = parse_skin_colors(fs_expr)
    fs_alphas: dict[str, int] = {}
    fs_rgbs: dict[str, str] = {}

    for skin in SKINS:
        val = fs_colors.get(skin, "")
        if not val:
            found.append(f"theme.slint: floating-surface 缺失 {skin} 皮肤分支")
            continue
        m = HEX8_RE.match(val)
        if not m:
            found.append(
                f"theme.slint: floating-surface 的 {skin} 皮肤字面量 '{val}' 不是 8 位 #RRGGBBAA 格式"
            )
            continue
        rgb = m.group(1).upper()
        alpha = int(m.group(2), 16)
        fs_rgbs[skin] = rgb
        fs_alphas[skin] = alpha
        if alpha == 0xFF:
            found.append(
                f"theme.slint: floating-surface 的 {skin} 皮肤为不透明实心板 (alpha=0xFF)，必须为半透明玻璃帷幕"
            )
        elif alpha == 0x00:
            found.append(
                f"theme.slint: floating-surface 的 {skin} 皮肤为全透明 (alpha=0x00)，材质不可缺失"
            )

    if len(fs_alphas) == len(SKINS):
        distinct_alphas = set(fs_alphas.values())
        if len(distinct_alphas) > 1:
            hex_map = {s: f"0x{a:02X}" for s, a in fs_alphas.items()}
            found.append(
                f"theme.slint: floating-surface 四个皮肤的 alpha 字节不一致: {hex_map}，必须完全相同"
            )
        elif contract_alpha is not None:
            shared_alpha = next(iter(distinct_alphas))
            if shared_alpha != contract_alpha:
                found.append(
                    f"theme.slint: floating-surface 的 alpha 字节 (0x{shared_alpha:02X}) 与 contract 的 VEIL_ALPHA (0x{contract_alpha:02X}) 不一致"
                )

    # 2. Check that floating-surface RGB does not steal zinc-3 card tier
    z3_m = re.search(r"\bout\s+property\s*<\s*color\s*>\s*zinc-3\s*:\s*([^;]+);", text, re.S)
    if z3_m:
        z3_colors = parse_skin_colors(z3_m.group(1).strip())
        for skin in ("amoled", "forest", "dark"):
            if skin in fs_rgbs and skin in z3_colors:
                z3_val = z3_colors[skin]
                z3_rgb = z3_val.lstrip("#")[:6].upper()
                if fs_rgbs[skin] == z3_rgb:
                    found.append(
                        f"theme.slint: {skin} 皮肤的 floating-surface RGB (#{fs_rgbs[skin]}) "
                        f"不得等于该皮肤的 zinc-3 卡片层色 (#{z3_rgb})（胶囊材质必须是悬浮玻璃帷幕，不可偷换卡片色假充半透明）"
                    )

    # 3. Check dock-rim-color
    rim_m = re.search(r"\bout\s+property\s*<\s*color\s*>\s*dock-rim-color\s*:\s*([^;]+);", text, re.S)
    if not rim_m:
        found.append("theme.slint: 未定义 out property <color> dock-rim-color 描边属性")
    else:
        rim_expr = rim_m.group(1).strip()
        rim_colors = parse_skin_colors(rim_expr)
        for skin in SKINS:
            val = rim_colors.get(skin, "")
            if not val:
                found.append(f"theme.slint: dock-rim-color 缺失 {skin} 皮肤分支")
                continue
            m = HEX8_RE.match(val)
            if not m:
                found.append(
                    f"theme.slint: dock-rim-color 的 {skin} 皮肤字面量 '{val}' 不是 8 位 #RRGGBBAA 带 alpha 格式"
                )
                continue
            alpha = int(m.group(2), 16)
            if alpha == 0xFF:
                found.append(
                    f"theme.slint: dock-rim-color 的 {skin} 皮肤字面量 '{val}' 为实心色 (AA=FF)，玻璃边缘必须为半透明描边"
                )
            elif alpha == 0x00:
                found.append(
                    f"theme.slint: dock-rim-color 的 {skin} 皮肤字面量 '{val}' 为全透明 (AA=00)，玻璃边缘描边不可缺失"
                )

    return found


def check_app_bottom_bar(chrome_path: pathlib.Path) -> list[str]:
    found: list[str] = []
    if not chrome_path.is_file():
        found.append(f"app_chrome.slint: 文件不存在: {chrome_path}")
        return found

    text = mask_slint_comments(chrome_path.read_text(encoding="utf-8"))
    comp = extract_component(text, "AppBottomBar")
    if not comp:
        found.append("app_chrome.slint: 未找到 AppBottomBar 组件定义")
        return found

    # 1. Bar background must be transparent
    dock_idx = comp.find("dock :=")
    root_props = comp[:dock_idx] if dock_idx != -1 else comp
    if not re.search(r"\bbackground:\s*transparent\s*;", root_props):
        found.append("app_chrome.slint: AppBottomBar 条带自身 background 必须为 transparent，不可覆盖页面底板")

    # 2. Dock capsule background & border-color
    dock_block = extract_sub_block(comp, r"\bdock\s*:=\s*Rectangle\s*\{")
    if not dock_block:
        found.append("app_chrome.slint: AppBottomBar 未包含 dock := Rectangle 胶囊定义")
    else:
        if not re.search(r"\bbackground:\s*LumenTheme\.floating-surface\s*;", dock_block):
            found.append("app_chrome.slint: AppBottomBar 的 dock 胶囊 background 必须绑定 LumenTheme.floating-surface")
        if not re.search(r"\bborder-color:\s*LumenTheme\.dock-rim-color\s*;", dock_block):
            found.append("app_chrome.slint: AppBottomBar 的 dock 胶囊 border-color 必须绑定 LumenTheme.dock-rim-color")

    # 3. No literal hex colors inside AppBottomBar subtree
    literal_colors = HEX_COLOR_RE.findall(comp)
    if literal_colors:
        found.append(
            f"app_chrome.slint: AppBottomBar 子树内不得出现硬编码字面颜色 {literal_colors}，必须使用主题 token"
        )

    # 4. No with-alpha / alpha( expressions
    if re.search(r"(\.with-alpha\b|\balpha\s*\()", comp):
        found.append(
            "app_chrome.slint: AppBottomBar 子树内不得使用 with-alpha 或 alpha( 动态透明度推导，材质透明度必须由 contract 与主题锁定"
        )

    # 5. No dock-backdrop, plinth or zinc-0 covering rectangle
    if re.search(r"\bdock-backdrop\s*:?=", comp):
        found.append("app_chrome.slint: AppBottomBar 中不得出现 dock-backdrop 遮挡层")

    if re.search(r"\bbackground:\s*LumenTheme\.zinc-0\b", comp):
        found.append("app_chrome.slint: AppBottomBar 中不得出现用 zinc-0 铺底的覆盖矩形")

    if re.search(
        r"Rectangle\s*\{[^{}]*?(width:\s*(?:parent\.width|100%|root\.window-width))[^{}]*?background:\s*LumenTheme\.",
        comp,
    ):
        found.append("app_chrome.slint: AppBottomBar 中不得出现全幅遮挡底板 (plinth)")

    return found

def find_latest_published_run(captures_root: pathlib.Path) -> pathlib.Path | None:
    if not captures_root.is_dir():
        return None
    if (
        (captures_root / "overview-mobile-dark/image.png").exists()
        or (captures_root / "overview-mobile-dark.png").exists()
    ):
        return captures_root

    candidates: list[tuple[int, pathlib.Path]] = []
    for meta in captures_root.glob("*/capture-metadata.txt"):
        run = meta.parent
        manifest = run / "capture-manifest.tsv"
        if not manifest.is_file():
            continue
        facts = dict(
            line.split("=", 1)
            for line in meta.read_text(encoding="utf-8").splitlines()
            if "=" in line
        )
        if facts.get("scenario_count") != str(FULL_MATRIX_SIZE):
            continue
        candidates.append((manifest.stat().st_mtime_ns, run))
    if candidates:
        return max(candidates, default=(0, None))[1]
    return None


def check_capture_pixels(captures_root: pathlib.Path | None = None) -> list[str]:
    findings: list[str] = []
    root = captures_root or CAPTURES
    if not root.is_dir():
        return findings

    try:
        from PIL import Image
    except ImportError:
        return findings

    latest_dir = find_latest_published_run(root)

    if not latest_dir:
        return findings

    expected = {
        "dark": {
            "page_rgb": (10, 11, 14),      # zinc-0 #0A0B0E
            "opaque_rgb": (51, 61, 72),     # floating-surface tint #333D48
            "glass_rgb": (33, 39, 45),      # #333D4899 composited over the page
        },
        "light": {
            "page_rgb": (239, 240, 243),   # zinc-0 #EFF0F3
            "opaque_rgb": (255, 255, 255), # floating-surface tint #FFFFFF
            "glass_rgb": (234, 234, 235),  # #FFFFFF99 over the page (Skia dithered)
        },
    }

    for skin in ("dark", "light"):
        img_p = latest_dir / f"overview-mobile-{skin}/image.png"
        if not img_p.exists():
            img_p = latest_dir / f"overview-mobile-{skin}.png"
        if not img_p.exists():
            continue

        im = Image.open(img_p).convert("RGB")
        # Capsule filler between the icon row and the tab labels: x=122..278,
        # y=772..777. The reference rows are pure capsule body, so any icon
        # ink or label ink cannot be mistaken for the glass fill itself.
        pixels = [im.getpixel((x, y)) for y in range(772, 778) for x in range(122, 279)]
        if not pixels:
            continue
        common_rgb = Counter(pixels).most_common(1)[0][0]

        def to_gray(rgb: tuple[int, int, int]) -> float:
            return (rgb[0] + rgb[1] + rgb[2]) / 3.0

        page_gray = to_gray(expected[skin]["page_rgb"])
        opaque_gray = to_gray(expected[skin]["opaque_rgb"])
        measured_gray = to_gray(common_rgb)

        glass_gray = to_gray(expected[skin]["glass_rgb"])
        glass_gray = to_gray(expected[skin]["glass_rgb"])
        if abs(measured_gray - glass_gray) > 4.0:
            findings.append(
                f"pixel receipt: {skin} 胶囊采样像素灰阶 ({measured_gray:.1f}) 偏离玻璃 veil 合成值 ({glass_gray:.1f}) 超过 4 阶，材质已漂移"
            )
        delta_opaque = abs(measured_gray - opaque_gray)
        if delta_opaque < 6.0:
            findings.append(
                f"pixel receipt: {skin} 胶囊采样像素灰阶 ({measured_gray:.1f}) 与不透明填充色 ({opaque_gray:.1f}) 灰阶差小于 6 阶 (当前相差 {delta_opaque:.1f})"
            )

    return findings


def violations(
    ui_root: pathlib.Path = UI,
    contract_path: pathlib.Path = CONTRACT,
    captures_root: pathlib.Path | None = None,
    check_pixels: bool = True,
) -> list[str]:
    found: list[str] = []
    contract_alpha, contract_findings = check_contract(contract_path)
    found.extend(contract_findings)

    theme_path = ui_root / "components/theme.slint"
    found.extend(check_theme(theme_path, contract_alpha))

    chrome_path = ui_root / "components/app_chrome.slint"
    found.extend(check_app_bottom_bar(chrome_path))

    if check_pixels:
        found.extend(check_capture_pixels(captures_root))

    return found


ORIGINAL_READ_TEXT = pathlib.Path.read_text


class GuardSelfTest(unittest.TestCase):
    def test_live_tree_has_zero_violations(self) -> None:
        findings = violations()
        self.assertEqual(findings, [])

    def test_opaque_floating_surface_is_caught(self) -> None:
        theme_text = THEME.read_text(encoding="utf-8")
        # Plant 6-digit hex
        planted_6hex = theme_text.replace("#303A4499", "#303A44", 1)
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == THEME:
                    return planted_6hex
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations()
        self.assertTrue(any("8 位 #RRGGBBAA" in item for item in findings), findings)

        # Plant AA=FF (opaque plate)
        planted_ff = theme_text.replace("#303A4499", "#303A44FF", 1)
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == THEME:
                    return planted_ff
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations()
        self.assertTrue(any("实心板" in item or "不透明" in item for item in findings), findings)

    def test_inconsistent_skin_alphas_are_caught(self) -> None:
        theme_text = THEME.read_text(encoding="utf-8")
        planted = theme_text.replace("#303A4499", "#303A4480", 1)
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == THEME:
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations()
        self.assertTrue(any("alpha 字节不一致" in item for item in findings), findings)

    def test_alpha_mismatch_with_contract_is_caught(self) -> None:
        contract_text = CONTRACT.read_text(encoding="utf-8")
        planted = contract_text.replace(
            "pub const VEIL_ALPHA: u8 = 0x99;",
            "pub const VEIL_ALPHA: u8 = 0x88;",
            1,
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == CONTRACT:
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations()
        self.assertTrue(any("VEIL_ALPHA" in item and "不一致" in item for item in findings), findings)

    def test_floating_surface_stolen_zinc3_card_color_is_caught(self) -> None:
        theme_text = THEME.read_text(encoding="utf-8")
        # In dark mode, replace #333D4899 with dark zinc-3 #1D212699
        planted = theme_text.replace("#333D4899", "#1D212699", 1)
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == THEME:
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations()
        self.assertTrue(any("zinc-3" in item for item in findings), findings)

    def test_literal_colors_and_alpha_in_dock_subtree_are_caught(self) -> None:
        chrome_text = CHROME.read_text(encoding="utf-8")
        planted_literal = chrome_text.replace(
            "border-color: LumenTheme.dock-rim-color;",
            "border-color: #FFFFFF26;",
            1,
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == CHROME:
                    return planted_literal
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations()
        self.assertTrue(any("硬编码字面颜色" in item for item in findings), findings)

        planted_alpha = chrome_text.replace(
            "border-color: LumenTheme.dock-rim-color;",
            "border-color: LumenTheme.dock-rim-color.with-alpha(0.5);",
            1,
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == CHROME:
                    return planted_alpha
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations()
        self.assertTrue(any("with-alpha" in item for item in findings), findings)

    def test_strip_not_transparent_or_plinth_is_caught(self) -> None:
        chrome_text = CHROME.read_text(encoding="utf-8")
        # Bar not transparent
        planted_bg = chrome_text.replace(
            "background: transparent;",
            "background: LumenTheme.zinc-0;",
            1,
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == CHROME:
                    return planted_bg
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations()
        self.assertTrue(any("transparent" in item for item in findings), findings)

        # Plinth / backdrop
        planted_plinth = chrome_text.replace(
            "dock := Rectangle {",
            "dock-backdrop := Rectangle { background: LumenTheme.zinc-0; }\n        dock := Rectangle {",
            1,
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == CHROME:
                    return planted_plinth
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations()
        self.assertTrue(any("dock-backdrop" in item or "zinc-0" in item for item in findings), findings)

    def test_dock_rim_color_solid_is_caught(self) -> None:
        theme_text = THEME.read_text(encoding="utf-8")
        planted = theme_text.replace("#FFFFFF26", "#FFFFFFFF", 1)
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == THEME:
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations()
        self.assertTrue(any("dock-rim-color" in item and "实心色" in item for item in findings), findings)

    def test_contract_solid_plate_is_caught(self) -> None:
        contract_text = CONTRACT.read_text(encoding="utf-8")
        planted = contract_text.replace(
            "pub const VEIL_ALPHA: u8 = 0x99;",
            "pub const VEIL_ALPHA: u8 = 0xFF;",
            1,
        )
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path == CONTRACT:
                    return planted
                return ORIGINAL_READ_TEXT(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations()
        self.assertTrue(any("VEIL_ALPHA" in item for item in findings), findings)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce", "test"], default="report")
    parser.add_argument("--self-test", action="store_true", help="运行内部单元测试")
    parser.add_argument("--ui-root", type=pathlib.Path, default=UI)
    parser.add_argument("--contract", type=pathlib.Path, default=CONTRACT)
    parser.add_argument("--captures-dir", type=pathlib.Path, default=CAPTURES)
    args = parser.parse_args()

    if args.self_test or args.mode == "test":
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    found = violations(args.ui_root, args.contract, args.captures_dir)
    for item in found:
        print(f"SLINT-DOCK-MATERIAL [{args.mode}]: {item}")
    print(f"slint dock material guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
