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
ICON_SEMANTIC_RE = re.compile(r"\bIcons\.")
CANONICAL_ICON_REF_RE = re.compile(r"ui/iced/svg/[a-z0-9-]+\.svg")
LUMEN_ICON_REF_RE = re.compile(r"ui/lumen/svg/([a-z0-9-]+\.svg)")
RAW_SLINT_SIZE_RE = re.compile(r"font-size:\s*\d+(?:\.\d+)?")
RAW_SLINT_WEIGHT_RE = re.compile(r"font-weight:\s*\d+")
RAW_ICED_SIZE_RE = re.compile(r"\.size\(\s*\d+(?:\.\d+)?\s*\)")
RAW_ICED_FONT_RE = re.compile(r"\.font\((?:theme::)?(?:MONO|FONT_MEDIUM|FONT_SEMIBOLD)\)")
BUSINESS_KOTLIN_FONT_RE = re.compile(
    r"fontSize\s*=|fontWeight\s*=|FontFamily\.Monospace"
)


def read_authority() -> dict:
    return json.loads(AUTHORITY.read_text(encoding="utf-8"))


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
        for match in re.finditer(r"font-weight:\s*([^;]+);", path.read_text(encoding="utf-8")):
            for value in re.findall(r"\b\d{3}\b", match.group(1)):
                if int(value) not in ALLOWED_WEIGHTS:
                    problems.append(f"{relative}: font-weight {value} has no shipped face")
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
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    catalog_ids = {record["id"] for record in catalog["assets"] if record["format"] == "ttf"}
    authority_ids = {entry["catalog_id"] for entry in read_authority()["font_faces"]}
    if catalog_ids != authority_ids:
        problems.append("catalog font ids differ from design authority font ids")
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


def all_violations(include_freshness: bool = True) -> list[str]:
    violations = generated_check() + slint_weight_check() + kotlin_weight_check()
    violations += surface_role_consumer_check() + font_owner_check() + icon_authority_check()
    standard = ROOT / "docs/DESIGN_RESOURCE_STANDARD.md"
    if not standard.is_file():
        violations.append("design resource standard document is missing")
    else:
        text = standard.read_text(encoding="utf-8")
        for required in (
            "> 层级：L2",
            "400 / 500 / 600",
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

    def test_platform_native_exceptions_carry_reasons(self) -> None:
        self.assertTrue(
            all(entry["reason"] for entry in read_authority()["platform_native_icons"])
        )

    def test_generated_adapters_are_fresh(self) -> None:
        self.assertEqual(generated_check(), [])


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
