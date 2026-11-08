#!/usr/bin/env python3
"""Generate non-Rust design-resource adapters from the contract authority.

The Rust contract exports `design-authority.json`; this script is the only
writer of the Kotlin font/typography/icon adapters and the Slint typography
global. A generated file is checked for exact equality by the structure gate,
so a hand edit cannot silently become a second design source.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
AUTHORITY = ROOT / "crates/infiltrator-contract/assets/design-authority.json"
CATALOG = ROOT / "crates/infiltrator-shared/assets/asset-catalog.json"
ANDROID_THEME = ROOT / "android/app/src/main/java/com/musicfrog/infiltrator/ui/theme"
ANDROID_COMMON = ROOT / "android/app/src/main/java/com/musicfrog/infiltrator/ui/common"
SLINT_UI = ROOT / "crates/infiltrator-slint/ui"
SLINT_COMPONENTS = SLINT_UI / "components"
SLINT_FONT_IMPORT_PREFIX = "../../../infiltrator-shared/assets/fonts/"
EXPECTED_CJK_FAMILY = "HarmonyOS Sans SC"
EXPECTED_CATALOG_LICENSE = "HarmonyOS Sans Fonts License Agreement"
EXPECTED_CATALOG_LICENSE_PATH = (
    "crates/infiltrator-shared/assets/fonts/HarmonyOS-Sans-LICENSE.txt"
)
RETIRED_ROLE_TOKENS = {
    "MicroMedium",
    "CaptionMedium",
    "LabelStrongCompact",
    "LabelCompact",
    "MonoNumericLarge",
    "MonoNumericDense",
}
# Distinct semantic roles may share one physical typeface, but only when the
# authority explicitly names the group. This ledger prevents new exact aliases;
# adding a role here is an architecture decision, not a compatibility fallback.
SEMANTIC_DUPLICATE_EXEMPTIONS = {
    frozenset({"Title", "Heading"}),
    frozenset({"Mono", "MonoStrong", "NumericSmall"}),
}


def pascal(name: str) -> str:
    return "".join(part.capitalize() for part in name.replace("-", "_").split("_"))


def semantic_constant(name: str) -> str:
    """`chevronRight` → `ChevronRight`; contract casing is authoritative."""
    parts = re.split(r"(?<=[a-z])(?=[A-Z])", name)
    return "".join(part[:1].upper() + part[1:] for part in parts)


def role_constant(name: str) -> str:
    """`bodyLarge` -> `BodyLarge`; contract role casing is authoritative."""
    return name[:1].upper() + name[1:]

def role_property(name: str) -> str:
    """`Display` -> `display`; Slint globals use camel case properties."""
    return name[:1].lower() + name[1:]


def authority_problems(authority: dict) -> list[str]:
    """Reject newly registered exact typeface aliases.

    The batch expansion is closed: only the documented pre-batch semantic
    duplicates may share a triple. A future role reusing any typeface is a
    second alias and fails here before it can reach adapters.
    """
    problems: list[str] = []
    authoritative_faces(authority, problems)
    if len(authority.get("font_faces", [])) != 7:
        problems.append("font_faces must contain exactly 7 physical faces")
    if len(authority.get("type_roles", [])) != 26:
        problems.append("type_roles must contain exactly 26 registered roles")
    if authority.get("font_weights") != [400, 500, 600]:
        problems.append("font_weights must be the closed set [400, 500, 600]")
    allowed_families = {
        (face["family_name"], face["weight"]): face["catalog_id"]
        for face in authority.get("font_faces", [])
    }
    for entry in authority.get("type_roles", []):
        role_key = (entry["family_name"], entry["weight"])
        if role_key not in allowed_families:
            problems.append(
                f"role {entry['role']} requests unshipped family/weight {role_key}"
            )
        elif allowed_families[role_key] != entry["catalog_id"]:
            problems.append(f"role {entry['role']} disagrees with its physical face")
        if entry["family_name"] == EXPECTED_CJK_FAMILY:
            if not entry["catalog_id"].startswith("font.CJK-"):
                problems.append(
                    f"CJK role {entry['role']} does not resolve to a HarmonyOS Sans SC face"
                )
            expected_weight = {
                "font.CJK-Regular": "Regular",
                "font.CJK-Medium": "Medium",
                "font.CJK-SemiBold": "SemiBold",
            }.get(entry["catalog_id"])
            if entry["weight"] != expected_weight:
                problems.append(
                    f"CJK role {entry['role']} disagrees with its physical weight"
                )
    seen: dict[tuple[float, str, str], list[str]] = {}
    for entry in authority.get("type_roles", []):
        triple = (entry["size_px"], entry["weight"], entry["family_name"])
        seen.setdefault(triple, []).append(entry["role"])
    for triple, roles in seen.items():
        grouped = frozenset(roles)
        if len(roles) > 1 and grouped not in SEMANTIC_DUPLICATE_EXEMPTIONS:
            problems.append(
                f"duplicate typeface triple {triple}: {', '.join(sorted(roles))}"
            )
    for entry in authority.get("type_roles", []):
        if entry["role"] in RETIRED_ROLE_TOKENS:
            problems.append(f"retired TextRole is still exported: {entry['role']}")
    if not any(entry["role"] == "Body" for entry in authority.get("type_roles", [])):
        problems.append("the Body fallback role is missing from the authority")
    if len({entry["role"] for entry in authority.get("type_roles", [])}) != len(
        authority.get("type_roles", [])
    ):
        problems.append("TextRole names are not unique")
    if len({face["catalog_id"] for face in authority.get("font_faces", [])}) != len(
        authority.get("font_faces", [])
    ):
        problems.append("font face catalog ids are not unique")
    for key, expected in (
        ("connection_column_weights", 7),
        ("hero_fact_column_weights", 2),
    ):
        weights = authority.get(key)
        if not isinstance(weights, list) or len(weights) != expected:
            problems.append(f"{key} must contain exactly {expected} absolute fractions")
            continue
        if any(not isinstance(weight, int) or isinstance(weight, bool) or weight <= 0 for weight in weights):
            problems.append(f"{key} must contain positive integer fractions")
        else:
            if sum(weights) != (48 if key == "connection_column_weights" else 12):
                problems.append(f"{key} no longer matches the contract weight total")
    return problems


def authoritative_faces(authority: dict, problems: list[str]) -> dict[str, str]:
    """Resolve each face id to its one shared catalog path and licence."""
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))["assets"]
    by_id = {record["id"]: record for record in catalog if record["format"] == "ttf"}
    paths: dict[str, str] = {}
    for face in authority.get("font_faces", []):
        record = by_id.get(face["catalog_id"])
        if record is None:
            problems.append(f"font face is missing from shared catalog: {face['catalog_id']}")
            continue
        paths[face["catalog_id"]] = record["path"]
        if record["path"] != f"fonts/{pathlib.PurePosixPath(face['catalog_id'].removeprefix('font.')).with_suffix('.ttf')}":
            problems.append(
                f"font face path drifts from its catalog id: {face['catalog_id']}"
            )
        if face["family"] == "Cjk":
            if face["family_name"] != EXPECTED_CJK_FAMILY:
                problems.append("CJK family must be uniquely HarmonyOS Sans SC")
            if record["license"] != EXPECTED_CATALOG_LICENSE:
                problems.append(f"CJK face has an unexpected licence: {face['catalog_id']}")
            if record["license_path"] != EXPECTED_CATALOG_LICENSE_PATH:
                problems.append(f"CJK face has an unexpected licence path: {face['catalog_id']}")
    cjk_families = {
        face["family_name"]
        for face in authority.get("font_faces", [])
        if face["family"] == "Cjk"
    }
    if cjk_families != {EXPECTED_CJK_FAMILY}:
        problems.append("CJK faces do not share exactly one family name")
    return paths


def font_import_lines(authority: dict) -> list[str]:
    paths = authoritative_faces(authority, [])
    lines = [
        "// Generated by scripts/generate_design_resources.py from design-authority.json.",
        "// The seven shipped physical faces are registered once from the shared catalog.",
    ]
    for face in authority["font_faces"]:
        path = paths.get(face["catalog_id"])
        if path is None:
            continue
        lines.append(f'import "{SLINT_FONT_IMPORT_PREFIX}{pathlib.PurePosixPath(path).name}";')
    return lines

def font_family(authority: dict) -> str:
    lines = [
        "package com.musicfrog.infiltrator.ui.theme",
        "",
        "// Generated by scripts/generate_design_resources.py from design-authority.json.",
        "// Do not edit by hand: the physical font inventory is owned by the shared asset catalog.",
        "",
        "import androidx.compose.ui.text.font.Font",
        "import androidx.compose.ui.text.font.FontFamily",
        "import androidx.compose.ui.text.font.FontWeight",
        "import com.musicfrog.infiltrator.R",
        "",
        "val AppFontFamily = FontFamily(",
    ]
    for face in authority["font_faces"]:
        weight = "Normal" if face["weight"] == "Regular" else ("SemiBold" if face["weight"] == "SemiBold" else pascal(face["weight"]))
        lines.append(f'    Font(R.font.{face["android_resource"]}, FontWeight.{weight}),')
    lines += [
        ")",
        "",
        "val AppMonoFontFamily = FontFamily(",
        '    Font(R.font.jetbrains_mono_regular, FontWeight.Normal),',
        ")",
        "",
    ]
    return "\n".join(lines)


def type_scale(authority: dict) -> str:
    lines = [
        "package com.musicfrog.infiltrator.ui.theme",
        "",
        "// Generated by scripts/generate_design_resources.py from design-authority.json.",
        "",
        "import androidx.compose.ui.unit.TextUnit",
        "import androidx.compose.ui.unit.sp",
        "",
        "object TypeScale {",
    ]
    for entry in authority["type_roles"]:
        key = role_constant(entry["role"])
        lines.append(f"    val {key}: TextUnit = {entry['size_px']:g}.sp")
    lines += ["}", ""]
    return "\n".join(lines)


def android_typography(authority: dict) -> str:
    roles = {entry["role"]: entry for entry in authority["type_roles"]}
    lines = [
        "package com.musicfrog.infiltrator.ui.theme",
        "",
        "// Generated by scripts/generate_design_resources.py from design-authority.json.",
        "// Weights are limited to the three shipped physical faces (400/500/600).",
        "",
        "import androidx.compose.material3.Typography",
        "import androidx.compose.ui.text.TextStyle",
        "import androidx.compose.ui.text.font.FontWeight",
        "import androidx.compose.ui.unit.sp",
        "",
        "val AppTypography = Typography(",
    ]
    mapping = [
        ("displayLarge", "displayLarge", "Display"),
        ("displayMedium", "displayMedium", "Title"),
        ("displaySmall", "displaySmall", "Subheading"),
        ("headlineLarge", "headlineLarge", "Title"),
        ("headlineMedium", "headlineMedium", "Subheading"),
        ("headlineSmall", "headlineSmall", "Subheading"),
        ("titleLarge", "titleLarge", "Heading"),
        ("titleMedium", "titleMedium", "LabelStrong"),
        ("titleSmall", "titleSmall", "Label"),
        ("bodyLarge", "bodyLarge", "BodyLarge"),
        ("bodyMedium", "bodyMedium", "Body"),
        ("bodySmall", "bodySmall", "Caption"),
        ("labelLarge", "labelLarge", "Label"),
        ("labelMedium", "labelMedium", "Micro"),
        ("labelSmall", "labelSmall", "Tag"),
    ]
    for property_name, _, role in mapping:
        spec = roles[role]
        catalog_id = spec["catalog_id"]
        family = "AppMonoFontFamily" if "JetBrainsMono" in catalog_id else "AppFontFamily"
        weight = "Normal" if spec["weight"] == "Regular" else ("SemiBold" if spec["weight"] == "SemiBold" else pascal(spec["weight"]))
        lines += [
            f"    {property_name} = TextStyle(",
            f"        fontFamily = {family},",
            f"        fontWeight = FontWeight.{weight},",
            f"        fontSize = {spec['size_px']:g}.sp,",
            "    ),",
        ]
    lines += [")", ""]
    return "\n".join(lines)


def slint_type(authority: dict) -> str:
    weight_values = {"Regular": 400, "Medium": 500, "SemiBold": 600}
    roles = authority["type_roles"]
    fallback = next(spec for spec in roles if spec["role"] == "Body")
    lines = font_import_lines(authority) + [
        "// Every surface reads the same closed set of physical weights.",
        "// A Text asks for one TextRole and resolves size/weight/family together;",
        "// it never assembles a typeface from three unrelated role prefixes.",
        'import { LumenUiScale } from "../lumen-ui-scale.slint";',
        "",
        "export enum TextRole {",
    ]
    for spec in roles:
        lines.append(f"    {role_property(spec['role'])},")
    lines += [
        "}",
        "",
        "export global DesignType {",
    ]
    for spec in roles:
        key = role_property(spec["role"])
        lines.append(f"    out property <length> {key}-size: {spec['size_px']:g} * LumenUiScale.unit;")
        lines.append(f"    out property <int> {key}-weight: {weight_values[spec['weight']]};")
        lines.append(f'    out property <string> {key}-family: "{spec["family_name"]}";')
    for function_name, value_type, value in (
        ("size", "length", lambda spec: f"{spec['size_px']:g} * LumenUiScale.unit"),
        ("weight", "int", lambda spec: str(weight_values[spec["weight"]])),
        ("family", "string", lambda spec: f'"{spec["family_name"]}"'),
    ):
        lines.append(f"    public pure function {function_name}(role: TextRole) -> {value_type} {{")
        for spec in roles:
            variant = role_property(spec["role"])
            lines.append(f"        if role == TextRole.{variant} {{ return {value(spec)}; }}")
        lines.append(f"        return {value(fallback)};")
        lines.append("    }")
    lines += ["}", ""]
    return "\n".join(lines)


def slint_mascot_assets(authority: dict) -> str:
    mascot = authority["mascot"]
    master = pathlib.PurePosixPath(mascot["path"]).name
    hero = pathlib.PurePosixPath(mascot["hero_path"]).name
    return "\n".join([
        "// Generated by scripts/generate_design_resources.py from design-authority.json.",
        "// The hero uses the prefiltered draw plate; the master stays the one catalog identity.",
        "",
        "export global MascotAssets {",
        f'    out property <image> master: @image-url("../../../infiltrator-shared/assets/mascot/{master}");',
        f'    out property <image> frog: @image-url("../../../infiltrator-shared/assets/mascot/{hero}");',
        "}",
        "",
    ])


def slint_layout_tokens(authority: dict) -> str:
    weights = authority["connection_column_weights"]
    total = 48
    hero_weights = authority["hero_fact_column_weights"]
    hero_total = 12
    lines = [
        "// Generated by scripts/generate_design_resources.py from design-authority.json.",
        "// The live connection table's column vocabulary has one authority.",
        "//",
        "// These are ABSOLUTE fractions of the table's content width, not",
        "// stretch weights. Slint distributes `horizontal-stretch` slack only",
        "// AFTER every child has taken its preferred width, so a 2-glyph CJK",
        "// header and a 12-glyph mono address in the same column start with",
        "// different pools and land on different x positions. A column edge",
        "// must be the same fraction of the row for the header and every body",
        "// row, regardless of how much text is inside the cell.",
        "",
        "export global LayoutTokens {",
    ]
    for index, weight in enumerate(weights, 1):
        lines.append(f"    out property <float> connection-column-{index}: {weight} / {total};")
    lines += [
        "",
        "    // The hero card's two labelled facts share the same absolute",
        "    // column fractions on the phone and desktop tiers. A stretch",
        "    // spacer between them let the organization text push the field",
        "    // grid off its own baseline.",
    ]
    for index, weight in enumerate(hero_weights, 1):
        lines.append(f"    out property <float> hero-fact-column-{index}: {weight} / {hero_total};")
    lines += ["}", ""]
    return "\n".join(lines)


def android_icon_semantics(authority: dict) -> str:
    lines = [
        "package com.musicfrog.infiltrator.ui.common",
        "",
        "// Generated by scripts/generate_design_resources.py from design-authority.json.",
        "// Business screens request a semantic id; they never name a toolkit symbol.",
        "",
        "enum class AppIconSemantic {",
    ]
    for icon in authority["icons"]:
        lines.append(f"    {semantic_constant(icon['semantic'])},")
    lines += ["}", ""]
    return "\n".join(lines)


def design_tokens(authority: dict) -> str:
    roles = {entry["role"]: entry for entry in authority["type_roles"]}
    lines = [
        "package com.musicfrog.infiltrator.ui.theme",
        "",
        "// Generated by scripts/generate_design_resources.py from design-authority.json.",
        "// Layout tokens remain the Kotlin mirror of contract::design_tokens;",
        "// the type scale is imported from the generated TypeScale object.",
        "",
        "import androidx.compose.ui.unit.Dp",
        "import androidx.compose.ui.unit.TextUnit",
        "import androidx.compose.ui.unit.dp",
        "",
        "object DesignTokens {",
        "    object Space {",
        "        val XS: Dp = 4.dp",
        "        val SM: Dp = 8.dp",
        "        val MD: Dp = 12.dp",
        "        val LG: Dp = 16.dp",
        "        val XL: Dp = 20.dp",
        "        val XXL: Dp = 24.dp",
        "        val ScreenMarginMobile: Dp = 16.dp",
        "        val ScreenMarginTablet: Dp = 24.dp",
        "    }",
        "",
        "    object Radius {",
        "        val XS: Dp = 4.dp",
        "        val SM: Dp = 8.dp",
        "        val Control: Dp = 10.dp",
        "        val Card: Dp = 16.dp",
        "        val Modal: Dp = 24.dp",
        "        val BottomSheet: Dp = 28.dp",
        "        val Dialog: Dp = 28.dp",
        "        val Pill: Dp = 999.dp",
        "    }",
        "",
        "    object Metrics {",
        "        val Hairline: Dp = 1.dp",
        "        val MinTouchTarget: Dp = 48.dp",
        "        val TouchTargetCompact: Dp = 44.dp",
        "        val TouchTargetComfortable: Dp = 48.dp",
        "        val BottomNavHeight: Dp = 56.dp",
        "        val TopAppBarHeight: Dp = 64.dp",
        "    }",
        "",
        "    object TypeScale {",
    ]
    for entry in authority["type_roles"]:
        key = role_constant(entry["role"])
        lines.append(f"        val {key}: TextUnit = com.musicfrog.infiltrator.ui.theme.TypeScale.{key}")
    lines += ["    }", "}", ""]
    assert roles["Body"]["size_px"] == 15.0
    return "\n".join(lines)


def generated_files(authority: dict) -> dict[pathlib.Path, str]:
    return {
        ANDROID_THEME / "AppFontFamily.kt": font_family(authority),
        ANDROID_THEME / "TypeScale.kt": type_scale(authority),
        ANDROID_THEME / "AppTypography.kt": android_typography(authority),
        ANDROID_COMMON / "AppIconSemantic.kt": android_icon_semantics(authority),
        SLINT_COMPONENTS / "design-type.slint": slint_type(authority),
        SLINT_COMPONENTS / "layout-tokens.slint": slint_layout_tokens(authority),
        SLINT_COMPONENTS / "mascot-assets.slint": slint_mascot_assets(authority),
        ANDROID_THEME / "DesignTokens.kt": design_tokens(authority),
    }


def compare(path: pathlib.Path, content: str) -> list[str]:
    if not path.is_file():
        return [f"missing generated file: {path.relative_to(ROOT)}"]
    if path.read_text(encoding="utf-8") != content:
        return [f"stale generated file: {path.relative_to(ROOT)}"]
    return []


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify generated files without writing")
    args = parser.parse_args()
    if not AUTHORITY.is_file():
        print(f"missing design authority: {AUTHORITY}", file=sys.stderr)
        return 1
    authority = json.loads(AUTHORITY.read_text(encoding="utf-8"))
    problems = authority_problems(authority)
    if problems:
        for problem in problems:
            print(problem, file=sys.stderr)
        return 1
    files = generated_files(authority)
    if args.check:
        problems = [item for path, content in files.items() for item in compare(path, content)]
        for problem in problems:
            print(problem, file=sys.stderr)
        return 1 if problems else 0
    for path, content in files.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    print(f"generated {len(files)} design-resource adapters")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
