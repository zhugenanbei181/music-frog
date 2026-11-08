#!/usr/bin/env python3
"""Enforce one typography/icon authority across all four UI surfaces.

Contract exports the closed tables; a generator writes Kotlin and Slint
adapters from that JSON. This guard proves the generated files are fresh,
font weights map to shipped physical faces, font bytes have one owner, and
icon semantics resolve to the canonical shared vector inventory. Platform
native exceptions are explicit entries with written reasons.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys
import unittest
import struct

ROOT = pathlib.Path(__file__).resolve().parents[2]
AUTHORITY = ROOT / "crates/infiltrator-contract/assets/design-authority.json"
CATALOG = ROOT / "crates/infiltrator-shared/assets/asset-catalog.json"
GENERATOR = ROOT / "scripts/generate_design_resources.py"
SLINT_UI = ROOT / "crates/infiltrator-slint/ui"
ANDROID_JAVA = ROOT / "android/app/src/main/java/com/musicfrog/infiltrator"
ICED_SRC = ROOT / "crates/infiltrator-iced/src"
BEVY_WIDGETS_SRC = ROOT / "crates/infiltrator-bevy-widgets/src"
SHARED_SRC = ROOT / "crates/infiltrator-shared/src"
ALLOWED_WEIGHTS = {400, 500, 600}
FONT_PATH_RE = re.compile(r"include_bytes!\(\"\.\./\.\./infiltrator-shared/assets/fonts/")
FONT_IMPORT_RE = re.compile(r'^\s*import\s+"([^"]+\.(?:ttf|otf|ttc))";', re.M)
FONT_WEIGHT_VALUES = {"Regular": 400, "Medium": 500, "SemiBold": 600}
FORBIDDEN_SLINT_WEIGHT_RE = re.compile(
    r"font-weight:\s*(?:700|[89]\d\d)|\bFontWeight\.Bold\b|font-synthesis|FontSynthesis"
)
ANIMATED_SLINT_WEIGHT_RE = re.compile(r"\banimate\s+[^;{]*\bfont-weight\b")
ICON_SEMANTIC_RE = re.compile(r"\bIcons\.")
CANONICAL_ICON_REF_RE = re.compile(r"ui/iced/svg/[a-z0-9-]+\.svg")
LUMEN_ICON_REF_RE = re.compile(r"ui/lumen/svg/([a-z0-9-]+\.svg)")
RAW_SLINT_SIZE_RE = re.compile(r"font-size:\s*\d+(?:\.\d+)?")
RAW_SLINT_WEIGHT_RE = re.compile(r"font-weight:\s*\d+")
RAW_SLINT_NUMERIC_SIZE_RE = re.compile(
    r"(?:font-size|title-size):\s*(?:[^;]*\?\s*)?\d+(?:\.\d+)?\s*\*\s*LumenUiScale\.unit"
)
RAW_ICED_SIZE_RE = re.compile(r"\.size\(\s*\d+(?:\.\d+)?\s*\)")
RAW_ICED_FONT_RE = re.compile(r"\.font\((?:theme::)?(?:MONO|FONT_MEDIUM|FONT_SEMIBOLD)\)")
BUSINESS_KOTLIN_FONT_RE = re.compile(
    r"fontSize\s*=|fontWeight\s*=|FontFamily\.Monospace"
)


def read_authority() -> dict:
    return json.loads(AUTHORITY.read_text(encoding="utf-8"))


def font_catalog_records() -> dict[str, dict]:
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))["assets"]
    return {
        record["id"]: record
        for record in catalog
        if record.get("format") == "ttf" and record.get("variant") == "default"
    }


def font_metadata(record: dict) -> tuple[int, str | None, str | None]:
    """Read one TTF's OS/2 weight and name-16 family without editing bytes."""
    data = (ROOT / "crates/infiltrator-shared/assets" / record["path"]).read_bytes()
    num_tables = struct.unpack(">H", data[4:6])[0]
    tables = {}
    for index in range(num_tables):
        offset = 12 + index * 16
        tag = data[offset : offset + 4].decode("latin-1")
        checksum, table_offset, length = struct.unpack(">III", data[offset + 4 : offset + 16])
        tables[tag] = (table_offset, length)
    if "OS/2" not in tables or "name" not in tables:
        raise ValueError(f"font is missing OS/2 or name table: {record['path']}")
    os2_offset, _ = tables["OS/2"]
    weight = struct.unpack(">H", data[os2_offset + 4 : os2_offset + 6])[0]
    name_offset, _ = tables["name"]
    _, count, string_offset = struct.unpack(">HHH", data[name_offset : name_offset + 6])
    family = None
    typographic_family = None
    subfamily = None
    typographic_subfamily = None
    for index in range(count):
        entry = name_offset + 6 + index * 12
        platform, encoding, language, name_id, length, offset = struct.unpack(
            ">HHHHHH", data[entry : entry + 12]
        )
        if platform != 3 or language not in (0x409, 0x804):
            continue
        raw = data[name_offset + string_offset + offset : name_offset + string_offset + offset + length]
        try:
            value = raw.decode("utf-16-be" if encoding in (1, 10) else "latin-1")
        except UnicodeDecodeError:
            continue
        if name_id == 16 and typographic_family is None:
            typographic_family = value
        elif name_id == 1 and family is None:
            family = value
        elif name_id == 17 and typographic_subfamily is None:
            typographic_subfamily = value
        elif name_id == 2 and subfamily is None:
            subfamily = value
    return (
        weight,
        typographic_family or family,
        typographic_subfamily or subfamily,
    )


def generated_check() -> list[str]:
    result = subprocess.run(
        [sys.executable, str(GENERATOR), "--check"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode:
        return [line.strip() for line in result.stderr.splitlines() if line.strip()]
    return []


def authority_freshness() -> list[str]:
    binary = ROOT / "target/debug/export_design_authority"
    if not binary.is_file():
        return ["design authority exporter is not built"]
    temporary = ROOT / "target/design-authority-check.json"
    result = subprocess.run(
        [str(binary), str(temporary)], cwd=ROOT, capture_output=True, text=True, check=False
    )
    if result.returncode:
        return ["design authority exporter failed: " + result.stderr.strip()]
    if temporary.read_text(encoding="utf-8") != AUTHORITY.read_text(encoding="utf-8"):
        return ["design-authority.json is stale; rerun export_design_authority"]
    return []


def slint_weight_check() -> list[str]:
    problems: list[str] = []
    for path in sorted(SLINT_UI.rglob("*.slint")):
        relative = path.relative_to(ROOT)
        text = path.read_text(encoding="utf-8")
        for match in re.finditer(r"font-weight:\s*([^;]+);", text):
            for value in re.findall(r"\b\d{3}\b", match.group(1)):
                if int(value) not in ALLOWED_WEIGHTS:
                    problems.append(f"{relative}: font-weight {value} has no shipped face")
        if FORBIDDEN_SLINT_WEIGHT_RE.search(text):
            problems.append(f"{relative}: synthetic bold or an unshipped weight is requested")
        if ANIMATED_SLINT_WEIGHT_RE.search(text):
            problems.append(f"{relative}: font-weight is animated instead of switching typed roles")
    return problems


def kotlin_weight_check() -> list[str]:
    problems: list[str] = []
    allowed = {"Normal", "Medium", "SemiBold"}
    for path in sorted(ANDROID_JAVA.rglob("*.kt")):
        relative = path.relative_to(ROOT)
        for weight in re.findall(r"FontWeight\.(\w+)", path.read_text(encoding="utf-8")):
            if weight not in allowed:
                problems.append(f"{relative}: FontWeight.{weight} has no shipped face")
    return problems


def surface_role_consumer_check() -> list[str]:
    """A shipped table is not enough: views must request its roles only."""
    problems: list[str] = []
    for path in sorted(SLINT_UI.rglob("*.slint")):
        if path.name == "design-type.slint":
            continue
        text = path.read_text(encoding="utf-8")
        if RAW_SLINT_SIZE_RE.search(text):
            problems.append(
                f"{path.relative_to(ROOT)}: raw font-size; use DesignType.<role>-size"
            )
        if RAW_SLINT_WEIGHT_RE.search(text):
            problems.append(
                f"{path.relative_to(ROOT)}: raw font-weight; use DesignType.<role>-weight"
            )
        if RAW_SLINT_NUMERIC_SIZE_RE.search(text):
            problems.append(
                f"{path.relative_to(ROOT)}: raw Slint size; use DesignType.<role>-size"
            )
    for name in ("pages/home_widgets.slint", "pages/home_desktop_grid.slint"):
        code = "\n".join(
            line.split("//", 1)[0] for line in (SLINT_UI / name).read_text(encoding="utf-8").splitlines()
        )
        if "column-unit" in code:
            problems.append(f"{name}: page-derived column-unit regressed")
    for path in sorted(ICED_SRC.rglob("*.rs")):
        if path.name == "theme.rs":
            continue
        text = path.read_text(encoding="utf-8")
        if RAW_ICED_SIZE_RE.search(text):
            problems.append(
                f"{path.relative_to(ROOT)}: raw Iced text size; use theme::text/role_text"
            )
        if RAW_ICED_FONT_RE.search(text):
            problems.append(
                f"{path.relative_to(ROOT)}: legacy Iced font constant; use TextRole"
            )
    theme_text = ICED_SRC / "view/theme.rs"
    theme_src = theme_text.read_text(encoding="utf-8")
    for name in ("role_body_text", "role_text"):
        if not re.search(rf"pub fn {name}<'a, \w+>", theme_src):
            problems.append(f"Iced role text entry point is missing: pub fn {name}<'a, ...>")
    for path in sorted(ANDROID_JAVA.rglob("*.kt")):
        if "ui/theme/" in path.as_posix():
            continue
        if BUSINESS_KOTLIN_FONT_RE.search(path.read_text(encoding="utf-8")):
            problems.append(
                f"{path.relative_to(ROOT)}: business code overrides typography; use AppTypography"
            )
    bevy_text = (BEVY_WIDGETS_SRC / "text.rs").read_text(encoding="utf-8")
    if "use infiltrator_contract::design_resources;" not in bevy_text:
        problems.append("Bevy widgets do not import the contract design_resources authority")
    if "design_resources::TextRole" not in bevy_text:
        problems.append("Bevy widgets do not consume the contract TextRole")
    if "role_typography" not in bevy_text or "typeface(role)" not in bevy_text:
        problems.append("Bevy widgets do not resolve typography through typeface(role)")
    # A second local role table would re-open the drift this standard closes.
    if "pub enum Role" in bevy_text:
        problems.append("Bevy widgets still declare a local Role table")
    return problems


def font_owner_check() -> list[str]:
    problems: list[str] = []
    for base in (ICED_SRC, BEVY_WIDGETS_SRC):
        for path in sorted(base.rglob("*.rs")):
            if FONT_PATH_RE.search(path.read_text(encoding="utf-8")):
                problems.append(
                    f"{path.relative_to(ROOT)}: surface repeats a shared font path; use FontFace::bytes()"
                )
    shared_assets = SHARED_SRC / "assets.rs"
    authority = read_authority()
    catalog = font_catalog_records()
    catalog_ids = set(catalog)
    authority_faces = authority["font_faces"]
    authority_ids = {entry["catalog_id"] for entry in authority_faces}
    if catalog_ids != authority_ids:
        problems.append("catalog font ids differ from design authority font ids")
    if len(authority_faces) != 7 or len(authority["type_roles"]) != 26:
        problems.append("design authority must own 7 physical faces and 26 typed roles")
    if authority.get("font_weights") != [400, 500, 600]:
        problems.append("design authority must own the closed 400/500/600 weight set")
    cjk_names = {
        face["family_name"] for face in authority_faces if face["family"] == "Cjk"
    }
    if cjk_names != {"HarmonyOS Sans SC"}:
        problems.append("CJK family is not uniquely HarmonyOS Sans SC")
    for face in authority_faces:
        record = catalog.get(face["catalog_id"])
        if record is None:
            continue
        expected_weight = FONT_WEIGHT_VALUES[face["weight"]]
        try:
            actual_weight, family, subfamily = font_metadata(record)
        except (OSError, ValueError, struct.error) as error:
            problems.append(f"cannot read physical font metadata for {record['path']}: {error}")
            continue
        if actual_weight != expected_weight:
            problems.append(
                f"{record['path']}: OS/2 weight {actual_weight} disagrees with {expected_weight}"
            )
        if face["family"] == "Cjk":
            if family != "HarmonyOS Sans SC":
                problems.append(f"{record['path']}: CJK family metadata is {family!r}")
            if subfamily not in {"Regular", "Medium", "Semibold"}:
                problems.append(f"{record['path']}: unexpected CJK subfamily {subfamily!r}")
            if record.get("license") != "HarmonyOS Sans Fonts License Agreement":
                problems.append(f"{record['path']}: HarmonyOS Sans licence is not declared")
            if record.get("license_path") != "crates/infiltrator-shared/assets/fonts/HarmonyOS-Sans-LICENSE.txt":
                problems.append(f"{record['path']}: HarmonyOS Sans licence path drifts")
    generated_imports = FONT_IMPORT_RE.findall(
        (SLINT_UI / "components/design-type.slint").read_text(encoding="utf-8")
    )
    expected_imports = [
        f"../../../infiltrator-shared/assets/fonts/{pathlib.PurePosixPath(face['catalog_id'].removeprefix('font.')).with_suffix('.ttf').name}"
        for face in authority_faces
    ]
    if generated_imports != expected_imports:
        problems.append("design-type.slint does not import exactly the seven authority font faces")
    for path in sorted(SLINT_UI.rglob("*.slint")):
        if path.name == "design-type.slint":
            continue
        if FONT_IMPORT_RE.search(path.read_text(encoding="utf-8")):
            problems.append(
                f"{path.relative_to(ROOT)}: surface registers a second font path; use generated design-type.slint"
            )
    shared_effect_assets = SHARED_SRC / "effect_assets.rs"
    source = shared_effect_assets.read_text(encoding="utf-8")
    if "pub const ALL: [Self; 7]" not in source:
        problems.append("shared FontFace byte owner does not declare seven faces")
    if "pub const Cjk: Self" in source:
        problems.append("shared FontFace keeps a single-face Cjk compatibility alias")
    if "font.CJK-Medium" not in source or "font.CJK-SemiBold" not in source:
        problems.append("shared FontFace owner does not register all three CJK physical faces")
    if not shared_assets.is_file():
        problems.append("shared asset catalog source is missing")
    tracked_fonts = subprocess.run(
        ["git", "ls-files", "--deleted", "--cached", "android/app/src/main/res/font"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    present = [
        line
        for line in tracked_fonts.stdout.splitlines()
        if (ROOT / line).exists()
    ]
    if tracked_fonts.returncode == 0 and present:
        problems.append(
            "android/app/src/main/res/font is a generated source directory; do not track font copies"
        )
    gradle = ROOT / "android/app/build.gradle.kts"
    if "GenerateSharedFonts" not in gradle.read_text(encoding="utf-8"):
        problems.append("Android font generator is not registered")
    return problems


def icon_authority_check() -> list[str]:
    problems: list[str] = []
    authority = read_authority()
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    svg_ids = {
        record["id"]
        for record in catalog["assets"]
        if record["format"] == "svg" and record["variant"] == "default" and record["id"].startswith("icon.")
    }
    for icon in authority["icons"]:
        if icon["catalog_id"] not in svg_ids:
            problems.append(f"icon {icon['semantic']} has no canonical SVG: {icon['catalog_id']}")
        canonical = ROOT / "crates/infiltrator-shared/assets/ui/iced/svg" / f"{icon['canonical_name']}.svg"
        if not canonical.is_file():
            problems.append(f"icon {icon['semantic']} has no canonical file: {canonical.name}")
        if icon["tier"] != "Canonical":
            problems.append(f"icon {icon['semantic']} is not classified Canonical")
    for entry in authority["platform_native_icons"]:
        if not entry.get("reason"):
            problems.append(f"platform-native icon {entry.get('id')} has no written reason")
    for path in sorted(SLINT_UI.rglob("*.slint")):
        for name in LUMEN_ICON_REF_RE.findall(path.read_text(encoding="utf-8")):
            if not name.startswith("ping-spin-"):
                problems.append(
                    f"{path.relative_to(ROOT)}: {name} is not a registered platform-native exception"
                )
    for path in sorted((ROOT / "android/app/src/main/java").rglob("*.kt")):
        if ICON_SEMANTIC_RE.search(path.read_text(encoding="utf-8")):
            problems.append(
                f"{path.relative_to(ROOT)}: business code names a toolkit icon family; use AppIcon"
            )
    semantic_enum = ANDROID_JAVA / "ui/common/AppIconSemantic.kt"
    if not semantic_enum.is_file():
        problems.append("Android icon semantic owner is missing")
    else:
        semantics = {icon["semantic"] for icon in authority["icons"]}
        listed = set(re.findall(r"^    (\w+),$", semantic_enum.read_text(encoding="utf-8"), re.M))
        if len(listed) != len(semantics):
            problems.append("Android icon semantic owner is not exhaustive")
    icon_owner = ANDROID_JAVA / "ui/common/AppIcon.kt"
    if not icon_owner.is_file():
        problems.append("Android canonical icon resolver is missing")
    else:
        for semantic in re.findall(r"AppIconSemantic\.(\w+)", icon_owner.read_text(encoding="utf-8")):
            if semantic not in semantics:
                problems.append(f"Android icon resolver invents semantic {semantic}")
    gradle = ROOT / "android/app/build.gradle.kts"
    if "material-icons-extended" in gradle.read_text(encoding="utf-8"):
        problems.append("Android still depends on a second icon family")
    return problems


def mascot_asset_check() -> list[str]:
    """The brand asset has one owner, real transparency and an honest licence."""
    problems: list[str] = []
    authority = read_authority()
    mascot = authority.get("mascot")
    if not mascot:
        return ["design authority has no mascot asset"]
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    records = [
        r for r in catalog["assets"]
        if r["id"] == mascot["catalog_id"] and r["path"] == mascot["path"]
    ]
    if len(records) != 1:
        problems.append(f"mascot catalog master {mascot['catalog_id']} is not unique")
        return problems
    record = records[0]
    if record["format"] != mascot["format"] or record["path"] != mascot["path"]:
        problems.append("mascot catalog record disagrees with design authority")
    asset = ROOT / "crates/infiltrator-shared/assets" / record["path"]
    if not asset.is_file():
        problems.append(f"mascot asset is missing: {record['path']}")
        return problems
    data = asset.read_bytes()
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        problems.append("mascot asset is not a PNG")
        return problems
    # IHDR: width, height, bit depth and colour type (must be RGBA = 6).
    width, height, depth, colour = struct.unpack(">IIBB", data[16:26])
    if depth != 8 or colour != 6:
        problems.append("mascot asset must be 8-bit RGBA with a real alpha channel")
    if width < 1024 or height < 1024:
        problems.append(
            f"mascot asset plate is {width}x{height}; at least 1024px is required so a hero draw is never an upscaled blur"
        )
    # A cutout must have both fully transparent and fully opaque pixels.
    try:
        from PIL import Image
    except ImportError:
        problems.append("Pillow is required to verify the mascot alpha channel")
        return problems
    with Image.open(asset) as image:
        alpha = image.convert("RGBA").getchannel("A")
        low, high = alpha.getextrema()
        corners = [
            image.convert("RGBA").getpixel(point)[3]
            for point in ((0, 0), (width - 1, 0), (0, height - 1), (width - 1, height - 1))
        ]
    if low != 0 or high != 255:
        problems.append("mascot alpha channel is not a real cutout (needs 0 and 255)")
    if any(value != 0 for value in corners):
        problems.append("mascot plate corners are not transparent")
    # A large IHDR is not proof of detail: the previous plate was an upscaled
    # 440px reference and measured only ~16.7 mean edge energy. The identity-
    # preserving 1254px master measures ~23.9. This rejects a fake resize.
    try:
        import numpy as np
        from PIL import ImageFilter
    except ImportError:
        problems.append("NumPy and Pillow are required to verify mascot plate detail")
        return problems
    with Image.open(asset) as image:
        rgba = image.convert("RGBA")
        alpha_mask = rgba.getchannel("A").point(lambda value: 255 if value > 200 else 0)
        opaque_edges = np.asarray(
            rgba.convert("L").filter(ImageFilter.FIND_EDGES), dtype=np.float32
        )[np.asarray(alpha_mask, dtype=bool)]
    if opaque_edges.size == 0 or float(opaque_edges.mean()) < 20.0:
        problems.append("mascot plate has no genuine high-resolution edge detail")
    hero_path = mascot.get("hero_path")
    hero_records = [
        r for r in catalog["assets"]
        if r["id"] == mascot["catalog_id"] and r["path"] == hero_path
    ]
    if not hero_path or len(hero_records) != 1:
        problems.append("mascot authority has no unique 512px hero draw variant")
    else:
        hero = ROOT / "crates/infiltrator-shared/assets" / hero_path
        if not hero.is_file():
            problems.append(f"mascot hero variant is missing: {hero_path}")
        else:
            with Image.open(hero) as image:
                hero_rgba = image.convert("RGBA")
                hero_alpha = hero_rgba.getchannel("A")
                hero_corners = [
                    hero_rgba.getpixel(point)[3]
                    for point in (
                        (0, 0), (image.width - 1, 0),
                        (0, image.height - 1), (image.width - 1, image.height - 1),
                    )
                ]
            if image.size != (512, 512):
                problems.append(
                    f"mascot hero variant is {image.width}x{image.height}; expected 512x512"
                )
            if hero_alpha.getextrema() != (0, 255) or any(hero_corners):
                problems.append("mascot hero variant is not a real transparent cutout")
            # The hero plate owns the circular boundary. A renderer may skip
            # clipping transformed children, so square-corner ink would escape
            # the disc even when the Slint component declares clip: true.
            hero_rgba_array = np.asarray(hero_rgba)
            hero_y, hero_x = np.mgrid[:image.height, :image.width]
            radius = np.hypot(
                hero_x - (image.width - 1) / 2.0,
                hero_y - (image.height - 1) / 2.0,
            )
            opaque_outside = (hero_rgba_array[..., 3] > 8) & (radius > image.width * 0.49)
            if opaque_outside.any():
                problems.append("mascot hero variant has character ink outside its circular mask")
    # The licence record must exist while redistribution is false.
    if not mascot["redistributable"]:
        licence = ROOT / record.get("license_path", "")
        if not licence.is_file():
            problems.append("non-redistributable mascot has no written licence status")
    # No surface may keep its own copy of the frog.
    for surface in ("crates/infiltrator-slint", "crates/infiltrator-iced", "crates/infiltrator-bevy-ui"):
        for stray in sorted((ROOT / surface).rglob("*frog*.png")):
            problems.append(f"surface keeps a second mascot image: {stray.relative_to(ROOT)}")
    return problems


def all_violations(include_freshness: bool = True) -> list[str]:
    violations = generated_check() + slint_weight_check() + kotlin_weight_check()
    violations += surface_role_consumer_check() + font_owner_check() + icon_authority_check()
    violations += mascot_asset_check()
    standard = ROOT / "docs/DESIGN_RESOURCE_STANDARD.md"
    if not standard.is_file():
        violations.append("design resource standard document is missing")
    else:
        text = standard.read_text(encoding="utf-8")
        for required in (
            "> 层级：L2",
            "400 / 500 / 600",
            "7 个物理面",
            "26 个 `TextRole`",
            "HarmonyOS Sans SC",
            "design-resource-authority.py",
            "PLATFORM_NATIVE_ICONS",
            "design-authority.json",
        ):
            if required not in text:
                violations.append(f"design resource standard is missing clause: {required}")
    structure_gate = ROOT / "scripts/quality/check-structure.sh"
    if "design-resource-authority.py" not in structure_gate.read_text(encoding="utf-8"):
        violations.append("design resource authority guard is not registered in the structure gate")
    main_gate = ROOT / ".github/workflows/main-gate.yml"
    if "scripts/quality/check-structure.sh" not in main_gate.read_text(encoding="utf-8"):
        violations.append("main CI gate no longer runs the structure gate")
    if include_freshness:
        violations += authority_freshness()
    return violations


class GuardSelfTest(unittest.TestCase):
    def test_shipped_weight_closed_set(self) -> None:
        self.assertEqual(read_authority()["font_weights"], [400, 500, 600])
        self.assertEqual(len(read_authority()["font_faces"]), 7)
        self.assertEqual(len(read_authority()["type_roles"]), 26)

    def test_font_metadata_matches_authority(self) -> None:
        problems = []
        for face in read_authority()["font_faces"]:
            record = font_catalog_records().get(face["catalog_id"])
            self.assertIsNotNone(record)
            weight, family, subfamily = font_metadata(record)
            self.assertEqual(weight, FONT_WEIGHT_VALUES[face["weight"]])
            if face["family"] == "Cjk":
                self.assertEqual(family, "HarmonyOS Sans SC")
                self.assertIn(subfamily, {"Regular", "Medium", "Semibold"})
        self.assertEqual(problems, [])

    def test_platform_native_exceptions_carry_reasons(self) -> None:
        self.assertTrue(
            all(entry["reason"] for entry in read_authority()["platform_native_icons"])
        )

    def test_generated_adapters_are_fresh(self) -> None:
        self.assertEqual(generated_check(), [])

    def test_synthetic_bold_is_caught(self) -> None:
        self.assertTrue(
            FORBIDDEN_SLINT_WEIGHT_RE.search("font-weight: 700;")
        )
        self.assertTrue(ANIMATED_SLINT_WEIGHT_RE.search("animate color, font-weight { }"))
        self.assertFalse(ANIMATED_SLINT_WEIGHT_RE.search("animate color { }"))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--skip-exporter", action="store_true", help="offline structural check")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        return 0 if unittest.TextTestRunner(verbosity=0).run(suite).wasSuccessful() else 1
    violations = all_violations(include_freshness=not args.skip_exporter)
    for violation in violations:
        print(f"VIOLATION [design-resource-authority]: {violation}", file=sys.stderr)
    print(f"design resource authority: violations={len(violations)}")
    return 1 if violations else 0


if __name__ == "__main__":
    raise SystemExit(main())
