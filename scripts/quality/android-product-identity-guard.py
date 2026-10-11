#!/usr/bin/env python3
"""Keep Android and desktop product IDs, versions, names, icons, and artifacts aligned."""

from __future__ import annotations

import argparse
from pathlib import Path
import plistlib
import re
import struct
import subprocess
import sys
import tomllib
import xml.etree.ElementTree as ET
from zipfile import BadZipFile, ZipFile

ROOT = Path(__file__).resolve().parents[2]
ANDROID_NS = "http://schemas.android.com/apk/res/android"
ANDROID = f"{{{ANDROID_NS}}}"
PRODUCT_FIELDS = (
    "application_id",
    "display_name",
    "icon_resource",
    "icon_asset",
    "name_resource",
    "name_resource_origin",
    "launch_manifest",
    "launcher_activity",
    "release_artifact_prefix",
    "version_code_scheme",
    "builder",
)
DESKTOP_FIELDS = (
    "display_name",
    "binary_name",
    "bundle_id",
    "icon_png",
    "icon_ico",
    "release_artifact_prefix",
    "release_matrix_key",
    "linux_package_id",
    "linux_desktop_file",
    "linux_comment",
    "url_scheme",
    "macos_app_dir",
    "macos_icon_filename",
    "windows_nsis_script",
    "windows_installer_prefix",
)
PACKAGE_ID = re.compile(r"^[A-Za-z][A-Za-z0-9_]*(?:\.[A-Za-z][A-Za-z0-9_]*)+$")
ANDROID_APPLICATION_PLUGIN = re.compile(
    r"(?:\bid\s*\(\s*['\"]com\.android\.application['\"]\s*\)"
    r"|\bid\s+['\"]com\.android\.application['\"]"
    r"|\bapply\s*\(\s*plugin\s*=\s*['\"]com\.android\.application['\"]\s*\)"
    r"|\bapply\s+plugin\s*:\s*['\"]com\.android\.application['\"]"
    r"|\balias\s*\([^)]*(?:android[._-]?application|androidapplication)[^)]*\))",
    re.IGNORECASE,
)
BADGING_PACKAGE = re.compile(
    r"^package: name='([^']+)' versionCode='(\d+)' versionName='([^']+)'"
)

sys.path.insert(0, str(ROOT / "scripts"))
from product_identity import (
    get_value,
    product_ids,
    read_properties,
    render_template,
    version_parts,
)


def android_application_module(source: str) -> bool:
    for line in source.splitlines():
        uncommented = line.split("//", 1)[0]
        if ANDROID_APPLICATION_PLUGIN.search(uncommented) is None:
            continue
        if re.search(r"\bapply\s+false\b", uncommented):
            continue
        return True
    return re.search(r"\bapplicationId\b", source) is not None


def source_problems(root: Path = ROOT) -> list[str]:
    problems: list[str] = []
    identity_path = root / "product-identity.properties"
    try:
        identity = read_properties(identity_path)
        version_parts(identity)
        products = product_ids(identity)
        desktop_products = product_ids(identity, "desktop")
    except (OSError, ValueError) as error:
        return [f"product identity file: {error}"]

    required_keys = [
        "version.name",
        "android.products",
        "android.namespace",
        "desktop.products",
        "rust.version_exemptions",
        "rust.non_product_binaries",
    ]
    required_keys.append("desktop.iced.cargo_packager_product_name")
    if "bevy" in products:
        required_keys.append("bevy.smoke.apk_name")
    for product in products:
        required_keys.extend(f"{product}.{field}" for field in PRODUCT_FIELDS)
        if identity.get(f"{product}.builder") == "cargo-apk":
            required_keys.append(f"{product}.apk_name")
        if identity.get(f"{product}.version_code_scheme") == "android-packed":
            required_keys.append(f"{product}.version_code_slot")
    for product in desktop_products:
        required_keys.extend(f"desktop.{product}.{field}" for field in DESKTOP_FIELDS)
    for key in required_keys:
        if not identity.get(key):
            problems.append(f"product identity is missing {key}")
    if any(not identity.get(key) for key in required_keys):
        return problems

    package_ids: set[str] = set()
    for product in products:
        package_id = identity.get(f"{product}.application_id", "")
        if not PACKAGE_ID.fullmatch(package_id):
            problems.append(f"{product}.application_id is not a valid application ID")
        elif package_id in package_ids:
            problems.append(f"Android product application ID is duplicated: {package_id}")
        package_ids.add(package_id)
        icon_name = identity.get(f"{product}.icon_resource", "")
        icon_asset = identity.get(f"{product}.icon_asset", "")
        icon_path = root / icon_asset
        if not icon_asset or not icon_path.is_file():
            problems.append(f"{product} launcher icon asset is missing: {icon_path}")
        elif icon_path.suffix.lower() != ".png" or not icon_path.read_bytes().startswith(
            b"\x89PNG\r\n\x1a\n"
        ):
            problems.append(f"{product}.icon_asset must be a valid PNG source file: {icon_path}")
        if identity[f"{product}.name_resource_origin"] != "gradle-resValue":
            problems.append(
                f"{product}.name_resource_origin must be gradle-resValue so its name comes from the registry"
            )
        launch_manifest = root / identity[f"{product}.launch_manifest"]
        if not launch_manifest.is_file():
            problems.append(f"{product} launcher manifest is missing: {launch_manifest}")
        if not identity[f"{product}.launcher_activity"].strip():
            problems.append(f"{product}.launcher_activity must identify the product entry point")
        if identity[f"{product}.builder"] not in ("gradle", "cargo-apk"):
            problems.append(f"{product}.builder must name a supported Android builder")
        try:
            get_value(identity, f"{product}.version_code")
        except (KeyError, ValueError) as error:
            problems.append(f"{product} versionCode cannot be derived: {error}")

    for product in products:
        try:
            code = int(get_value(identity, f"{product}.version_code"))
        except (KeyError, ValueError):
            continue
        if code > 2_100_000_000:
            problems.append(f"{product} derived Android versionCode exceeds the platform limit")

    build_sources = [
        *sorted((root / "android").rglob("build.gradle")),
        *sorted((root / "android").rglob("build.gradle.kts")),
        *sorted((root / "scripts").glob("build*apk.sh")),
    ]
    identity_references: set[str] = set()
    gradle_application_products: dict[str, list[Path]] = {}
    hardcoded_application_ids: set[str] = set()
    identity_reference = re.compile(
        r"identity(?:Value|_value)\s*(?:\(\s*)?[\"']?([a-z][a-z0-9_-]*)\.application_id"
    )
    hardcoded_application_id = re.compile(r"\bapplicationId\s*(?:=\s*)?[\"']([^\"']+)[\"']")
    for build_source in build_sources:
        source = build_source.read_text(encoding="utf-8")
        referenced_products = set(identity_reference.findall(source))
        identity_references.update(referenced_products)
        hardcoded_application_ids.update(hardcoded_application_id.findall(source))
        if build_source.is_relative_to(root / "android") and android_application_module(source):
            if not referenced_products:
                problems.append(
                    f"Android application module {build_source.relative_to(root)} has no registered product identity"
                )
            for product in referenced_products:
                if product not in products:
                    continue
                if identity.get(f"{product}.builder") != "gradle":
                    problems.append(
                        f"Android application module {build_source.relative_to(root)} uses {product!r}, "
                        "but its registered builder is not Gradle"
                    )
                gradle_application_products.setdefault(product, []).append(build_source)
    unregistered = identity_references.difference(products)
    for product in sorted(unregistered):
        problems.append(f"Android build references unregistered product {product!r}")
    for product in products:
        if product not in identity_references:
            problems.append(f"registered product {product!r} has no Android build adapter")
        if identity.get(f"{product}.builder") == "gradle":
            adapters = gradle_application_products.get(product, [])
            if not adapters:
                problems.append(f"registered Gradle product {product!r} has no Android application module")
            elif len(adapters) > 1:
                paths = ", ".join(str(path.relative_to(root)) for path in adapters)
                problems.append(
                    f"registered Gradle product {product!r} is claimed by multiple application modules: {paths}"
                )
            for adapter in adapters:
                source = adapter.read_text(encoding="utf-8")
                flavor_match = re.search(
                    rf"create\(\s*['\"]{re.escape(product)}['\"]\s*\)\s*\{{(?P<body>.*?)(?=\n\s*create\(|\n\s*\}})",
                    source,
                    re.DOTALL,
                )
                if flavor_match is None:
                    problems.append(
                        f"Android application module {adapter.relative_to(root)} must declare "
                        f"the {product!r} product flavor"
                    )
                    continue
                flavor_source = flavor_match.group("body")

                application_id_variable = re.search(
                    rf"\b(?:val|var)\s+(\w+)\s*=\s*identityValue\(\s*['\"]{re.escape(product)}\.application_id['\"]\s*\)",
                    source,
                )
                if application_id_variable is None or re.search(
                    rf"\bapplicationId\s*=\s*{application_id_variable.group(1)}\b",
                    flavor_source,
                ) is None:
                    problems.append(
                        f"Android application module {adapter.relative_to(root)} must bind applicationId "
                        f"to {product}.application_id"
                    )

                version_name_variable = re.search(
                    r"\b(?:val|var)\s+(\w+)\s*=\s*generatedIdentityValue\(\s*['\"]version\.name['\"]\s*\)",
                    source,
                )
                if version_name_variable is None or re.search(
                    rf"\bversionName\s*=\s*{version_name_variable.group(1)}\b",
                    flavor_source,
                ) is None:
                    problems.append(
                        f"Android application module {adapter.relative_to(root)} must derive versionName "
                        "from the workspace product version"
                    )

                version_code_variable = re.search(
                    rf"\b(?:val|var)\s+(\w+)\s*=\s*generatedIdentityValue\(\s*['\"]{re.escape(product)}\.version_code['\"]\s*\)\.toInt\(\)",
                    source,
                )
                if version_code_variable is None or re.search(
                    rf"\bversionCode\s*=\s*{version_code_variable.group(1)}\b",
                    flavor_source,
                ) is None:
                    problems.append(
                        f"Android application module {adapter.relative_to(root)} must derive versionCode "
                        f"from {product}.version_code"
                    )

                manifest_path = adapter.parent / "src/main/AndroidManifest.xml"
                try:
                    manifest_root = ET.parse(manifest_path).getroot()
                    application = manifest_root.find("application")
                except (OSError, ET.ParseError) as error:
                    application = None
                    problems.append(
                        f"Android application module {adapter.relative_to(root)} has no readable manifest: {error}"
                    )
                if application is None:
                    problems.append(
                        f"Android application module {adapter.relative_to(root)} has no application element"
                    )
                else:
                    expected_label = f"@string/{identity[f'{product}.name_resource']}"
                    if application.get(f"{ANDROID}label") != expected_label:
                        problems.append(
                            f"Android application module {adapter.relative_to(root)} label must be {expected_label!r}"
                        )
                display_name_variable = re.search(
                    rf"\b(?:val|var)\s+(\w+)\s*=\s*identityValue\(\s*['\"]{re.escape(product)}\.display_name['\"]\s*\)",
                    source,
                )
                if display_name_variable is None or re.search(
                    rf"resValue\(\s*['\"]string['\"]\s*,\s*['\"]{re.escape(identity[f'{product}.name_resource'])}['\"]\s*,\s*{display_name_variable.group(1)}\s*\)",
                    flavor_source,
                ) is None:
                    problems.append(
                        f"Android application module {adapter.relative_to(root)} must generate "
                        f"{product} display name from the registered identity"
                    )

                icon_asset_variable = re.search(
                    rf"\b(?:val|var)\s+(\w+)\s*=\s*identityValue\(\s*['\"]{re.escape(product)}\.icon_asset['\"]\s*\)",
                    source,
                )
                icon_resource_variable = re.search(
                    rf"\b(?:val|var)\s+(\w+)\s*=\s*identityValue\(\s*['\"]{re.escape(product)}\.icon_resource['\"]\s*\)",
                    source,
                )
                if (
                    icon_asset_variable is None
                    or icon_resource_variable is None
                    or re.search(
                        rf"['\"]{re.escape(product)}['\"]\s+to\s+\({icon_asset_variable.group(1)}\s+to\s+{icon_resource_variable.group(1)}\)",
                        source,
                    ) is None
                    or "addGeneratedSourceDirectory" not in source
                ):
                    problems.append(
                        f"Android application module {adapter.relative_to(root)} must generate "
                        f"{product} icon resources from the registered source asset"
                    )

                launcher_manifest_path = root / identity[f"{product}.launch_manifest"]
                try:
                    launcher_manifest = ET.parse(launcher_manifest_path).getroot()
                    launcher_application = launcher_manifest.find("application")
                except (OSError, ET.ParseError) as error:
                    launcher_application = None
                    problems.append(
                        f"{product} launcher manifest cannot be read: {error}"
                    )
                if launcher_application is None:
                    problems.append(f"{product} launcher manifest has no application element")
                else:
                    expected_icon = f"@drawable/{identity[f'{product}.icon_resource']}"
                    for attribute in ("icon", "roundIcon"):
                        if launcher_application.get(f"{ANDROID}{attribute}") != expected_icon:
                            problems.append(
                                f"{product} launcher manifest {attribute} must be {expected_icon!r}"
                            )
                    expected_label = f"@string/{identity[f'{product}.name_resource']}"
                    if launcher_application.get(f"{ANDROID}label") != expected_label:
                        problems.append(
                            f"{product} launcher manifest label must be {expected_label!r}"
                        )
                    expected_activity = identity[f"{product}.launcher_activity"]
                    activities = [
                        activity
                        for activity in launcher_application.findall("activity")
                        if activity.get(f"{ANDROID}name") == expected_activity
                    ]
                    launcher_activities = [
                        activity
                        for activity in activities
                        if any(
                            action.get(f"{ANDROID}name") == "android.intent.action.MAIN"
                            for intent_filter in activity.findall("intent-filter")
                            for action in intent_filter.findall("action")
                        )
                        and any(
                            category.get(f"{ANDROID}name") == "android.intent.category.LAUNCHER"
                            for intent_filter in activity.findall("intent-filter")
                            for category in intent_filter.findall("category")
                        )
                    ]
                    if len(launcher_activities) != 1:
                        problems.append(
                            f"{product} launcher manifest must expose exactly one MAIN/LAUNCHER "
                            f"activity named {expected_activity!r}"
                        )
                    elif launcher_activities[0].get(f"{ANDROID}exported") != "true":
                        problems.append(
                            f"{product} launcher activity {expected_activity!r} must be exported"
                        )

                launcher_variable = re.search(
                    rf"\b(?:val|var)\s+(\w+)\s*=\s*identityValue\(\s*['\"]{re.escape(product)}\.launcher_activity['\"]\s*\)",
                    source,
                )
                if launcher_variable is None or re.search(
                    rf"resValue\(\s*['\"]string['\"]\s*,\s*['\"]notification_activity['\"]\s*,\s*launcherActivityClass\(\s*{launcher_variable.group(1)}\s*\)\s*,?\s*\)",
                    flavor_source,
                ) is None:
                    problems.append(
                        f"Android application module {adapter.relative_to(root)} must derive "
                        f"{product} notification target from its registered launcher activity"
                    )
    for package_id in sorted(hardcoded_application_ids):
        if package_id not in package_ids:
            problems.append(f"Android build hard-codes an unregistered application ID: {package_id}")

    if "bevy" in products:
        smoke_build_path = root / "scripts/build-bevy-apk.sh"
        smoke_verify_path = root / "scripts/verify-bevy-apk.sh"
        try:
            smoke_build = smoke_build_path.read_text(encoding="utf-8")
        except OSError as error:
            problems.append(f"Bevy diagnostic APK builder cannot be read: {error}")
            smoke_build = ""
        try:
            smoke_verify = smoke_verify_path.read_text(encoding="utf-8")
        except OSError as error:
            problems.append(f"Bevy diagnostic APK verifier cannot be read: {error}")
            smoke_verify = ""
        for token in (
            "scripts/product_identity.py",
            "identity_value version.name",
            "identity_value bevy.application_id",
            "identity_value bevy.display_name",
            "identity_value bevy.icon_resource",
            "identity_value bevy.icon_asset",
            "identity_value bevy.smoke.apk_name",
        ):
            if token not in smoke_build:
                problems.append(f"Bevy diagnostic APK builder must consume product identity via {token}")
        for token in (
            "scripts/product_identity.py",
            "identity_value bevy.application_id",
            "identity_value bevy.smoke.apk_name",
        ):
            if token not in smoke_verify:
                problems.append(f"Bevy diagnostic APK verifier must consume product identity via {token}")

    for manifest_path in sorted((root / "android").rglob("AndroidManifest.xml")):
        if "build" in manifest_path.parts or ".gradle" in manifest_path.parts:
            continue
        try:
            manifest = ET.parse(manifest_path).getroot()
        except ET.ParseError as error:
            problems.append(f"Android manifest cannot be parsed: {manifest_path}: {error}")
            continue
        if manifest.find("application") is not None:
            package_id = manifest.get("package")
            if package_id and package_id not in package_ids:
                problems.append(f"Android manifest declares an unregistered application ID: {package_id}")

    for strings_path in sorted((root / "android/app/src").rglob("strings.xml")):
        if "build" in strings_path.parts:
            continue
        try:
            strings_root = ET.parse(strings_path).getroot()
        except ET.ParseError as error:
            problems.append(f"Android strings resource cannot be parsed: {strings_path}: {error}")
            continue
        for resource_name in ("app_name", "notification_activity"):
            if strings_root.find(f"string[@name='{resource_name}']") is not None:
                problems.append(
                    f"{strings_path.relative_to(root)} must not duplicate generated identity "
                    f"resource {resource_name!r}"
                )

    if "compose" in products:
        gradle_path = root / "android/app/build.gradle.kts"
        gradle = gradle_path.read_text(encoding="utf-8")
        for token in (
            'rootProject.file("../product-identity.properties")',
            'rootProject.file("../scripts/product_identity.py")',
            'val productVersionName = generatedIdentityValue("version.name")',
            'val composeVersionCode = generatedIdentityValue("compose.version_code").toInt()',
            'identityValue("compose.application_id")',
            'identityValue("android.namespace")',
        ):
            if token not in gradle:
                problems.append(f"Compose Gradle build no longer reads identity token {token}")
        namespace_variable = re.search(
            r"\b(?:val|var)\s+(\w+)\s*=\s*identityValue\(\s*['\"]android\.namespace['\"]\s*\)",
            gradle,
        )
        if namespace_variable is None or re.search(
            rf"\bnamespace\s*=\s*{namespace_variable.group(1)}\b", gradle
        ) is None:
            problems.append("Android Gradle namespace must derive from the product identity registry")

        manifest_path = root / "android/app/src/main/AndroidManifest.xml"
        manifest = ET.parse(manifest_path).getroot()
        application = manifest.find("application")
        if application is None:
            problems.append("Compose AndroidManifest.xml has no application element")
        else:
            if application.get(f"{ANDROID}label") != "@string/app_name":
                problems.append("Compose manifest label must resolve through @string/app_name")

    version_exemptions = {
        name.strip()
        for name in identity.get("rust.version_exemptions", "").split(",")
        if name.strip()
    }
    for manifest_path in sorted((root / "crates").rglob("Cargo.toml")):
        cargo_manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        package = cargo_manifest.get("package", {})
        package_name = package.get("name", "")
        if not package_name.startswith("infiltrator-") or package_name in version_exemptions:
            continue
        package_version = package.get("version")
        if not isinstance(package_version, dict) or package_version.get("workspace") is not True:
            problems.append(
                f"{package_name} must inherit [workspace.package].version instead of declaring a copy"
            )

    release_path = root / ".github/workflows/release.yml"
    release = release_path.read_text(encoding="utf-8")
    if "product_identity.py" not in release:
        problems.append("Android release workflow must derive version and artifact names from product identity")
    if "android-product-identity-guard.py" not in release:
        problems.append("Android release workflow must verify the final APK identities")
    release_apk_products = set(
        re.findall(r"--apk\s+['\"]([a-z][a-z0-9_-]*)=\$[A-Za-z_][A-Za-z0-9_]*['\"]", release)
    )
    for product in sorted(release_apk_products.difference(products)):
        problems.append(f"Android release workflow checks unregistered product {product!r}")
    for product in products:
        token = f"--get {product}.release_artifact_prefix"
        if token not in release:
            problems.append(f"Android release workflow does not derive {product} artifact names")
        artifact_prefix_variable = f"{product.upper().replace('-', '_')}_ARTIFACT_PREFIX"
        if (
            f'echo "{artifact_prefix_variable}=${artifact_prefix_variable}" >> "$GITHUB_ENV"'
            not in release
        ):
            problems.append(
                f"Android release workflow must export the registered {product} artifact prefix"
            )
        if f"name: ${{{{ env.{artifact_prefix_variable} }}}}-APKs" not in release:
            problems.append(
                f"Android upload artifact name for {product} must derive from its registered prefix"
            )
        apk_guard = re.compile(
            rf"--apk\s+['\"]{re.escape(product)}=\$[A-Za-z_][A-Za-z0-9_]*['\"]"
        )
        if apk_guard.search(release) is None:
            problems.append(f"Android release workflow does not aapt-check the {product} APK")
        prefix_assignment = re.search(
            rf"\b([A-Z][A-Z0-9_]*)=[\"']?\$\(\s*python3\s+scripts/product_identity\.py\s+--get\s+{re.escape(product)}\.release_artifact_prefix\s*\)[\"']?",
            release,
        )
        if prefix_assignment is None or re.search(
            rf'\$\{{{prefix_assignment.group(1)}\}}-\$\{{PRODUCT_VERSION\}}-[^"\s]*release\.apk',
            release,
        ) is None:
            problems.append(
                f"Android release workflow must name the {product} APK from its registered artifact prefix"
            )

    android_ci_path = root / ".github/workflows/android.yml"
    android_ci = android_ci_path.read_text(encoding="utf-8")
    android_ci_sources = [android_ci]
    emulator_test_script = root / "scripts/android-emulator-tests.sh"
    if "android-emulator-tests.sh" in android_ci and emulator_test_script.is_file():
        android_ci_sources.append(emulator_test_script.read_text(encoding="utf-8"))
    android_ci_build = "\n".join(android_ci_sources)
    if not any("android-product-identity-guard.py" in source for source in android_ci_sources):
        problems.append("Android CI must aapt-check registered debug APK identities")
    for product in products:
        variant = product[:1].upper() + product[1:]
        for task in (f":app:assemble{variant}Debug", f":app:test{variant}DebugUnitTest"):
            if task not in android_ci_build:
                problems.append(f"Android CI is missing {task} for product {product!r}")
        android_test_tasks = (
            f":app:assemble{variant}DebugAndroidTest",
            f":app:pixel6Api36{variant}DebugAndroidTest",
        )
        if not any(task in android_ci_build for task in android_test_tasks):
            problems.append(
                f"Android CI is missing an instrumented test build/run for product {product!r}"
            )
        apk_guard = re.compile(
            rf"--apk\s+['\"]{re.escape(product)}=\$[A-Za-z_][A-Za-z0-9_]*['\"]"
        )
        if apk_guard.search(android_ci_build) is None and f'--apk "{product}=' not in android_ci_build:
            problems.append(f"Android CI does not aapt-check the {product} debug APK")

    desktop_bundle_ids: set[str] = set()
    for product in desktop_products:
        key = f"desktop.{product}"
        bundle_id = identity[f"{key}.bundle_id"]
        if not PACKAGE_ID.fullmatch(bundle_id):
            problems.append(f"{key}.bundle_id is not a valid bundle identifier")
        elif bundle_id in desktop_bundle_ids:
            problems.append(f"desktop bundle ID is duplicated: {bundle_id}")
        desktop_bundle_ids.add(bundle_id)

        for field, suffix in (("icon_png", ".png"), ("icon_ico", ".ico")):
            icon_path = root / identity[f"{key}.{field}"]
            if icon_path.suffix != suffix or not icon_path.is_file():
                problems.append(f"{key}.{field} does not point to an existing {suffix} asset")

        binary_name = identity[f"{key}.binary_name"]
        cargo_manifest_path = root / f"crates/{binary_name}/Cargo.toml"
        try:
            cargo_package = tomllib.loads(cargo_manifest_path.read_text(encoding="utf-8"))["package"]
            if cargo_package.get("name") != binary_name:
                problems.append(f"{key}.binary_name does not match its Cargo product crate")
            if product == "iced":
                packager = cargo_package.get("metadata", {}).get("packager", {})
                if "version" in packager:
                    problems.append(
                        "Iced Cargo Packager must inherit the Cargo package version, not override it"
                    )
                if packager.get("product-name") != identity[f"{key}.cargo_packager_product_name"]:
                    problems.append("Iced Cargo Packager product-name differs from product identity")
                if packager.get("identifier") != identity[f"{key}.bundle_id"]:
                    problems.append("Iced Cargo Packager identifier differs from product identity")
                packager_icons = packager.get("icons", [])
                if not any(
                    (root / "crates/infiltrator-iced" / icon).resolve()
                    == (root / identity[f"{key}.icon_png"]).resolve()
                    for icon in packager_icons
                ):
                    problems.append("Iced Cargo Packager icon differs from product identity")
        except (OSError, KeyError, tomllib.TOMLDecodeError) as error:
            problems.append(f"{key}.binary_name has no matching Cargo product crate: {error}")

        desktop_path = root / identity[f"{key}.linux_desktop_file"]
        try:
            desktop_source = desktop_path.read_text(encoding="utf-8")
            desktop_rendered = render_template(desktop_source, identity)
            desktop_entry = {
                name: value
                for line in desktop_rendered.splitlines()
                if "=" in line
                for name, value in (line.split("=", 1),)
            }
        except OSError as error:
            problems.append(f"{key}.linux_desktop_file cannot be read: {error}")
            desktop_entry = {}
            desktop_source = ""
        except ValueError as error:
            problems.append(f"{key}.linux_desktop_file has an invalid identity placeholder: {error}")
            desktop_entry = {}
            desktop_source = ""
        for field in (
            "display_name",
            "binary_name",
            "linux_package_id",
            "linux_comment",
            "url_scheme",
        ):
            placeholder = f"@{key}.{field}@"
            if placeholder not in desktop_source:
                problems.append(f"{key} Linux desktop template must use {placeholder}")
        expected_entry_values = {
            "Name": identity[f"{key}.display_name"],
            "Comment": identity[f"{key}.linux_comment"],
            "Exec": f"{binary_name} %U",
            "Icon": identity[f"{key}.linux_package_id"],
            "StartupWMClass": binary_name,
        }
        for field, expected in expected_entry_values.items():
            if desktop_entry.get(field) != expected:
                problems.append(
                    f"{key} Linux desktop {field} must equal {expected!r} from product identity"
                )
        if f"x-scheme-handler/{identity[f'{key}.url_scheme']}" not in desktop_entry.get("MimeType", ""):
            problems.append(f"{key} Linux desktop URL scheme differs from product identity")

        nsis_path = root / identity[f"{key}.windows_nsis_script"]
        try:
            nsis = nsis_path.read_text(encoding="utf-8")
        except OSError as error:
            problems.append(f"{key}.windows_nsis_script cannot be read: {error}")
            nsis = ""
        expected_icon = identity[f"{key}.icon_ico"].replace("/", "\\")
        for token in (
            f'Name "{identity[f"{key}.display_name"]}"',
            f'!define ICON_PATH "{expected_icon}"',
            f'WriteRegStr HKLM "${{ARP_KEY}}" "DisplayName" "{identity[f"{key}.display_name"]}"',
        ):
            if token not in nsis:
                problems.append(f"{key} NSIS metadata no longer matches {token}")
        if identity[f"{key}.windows_installer_prefix"] not in release:
            problems.append(f"{key} Windows installer filename is absent from the release workflow")
        artifact_key = identity[f"{key}.release_matrix_key"]
        if not any(
            line.strip().startswith(f"{artifact_key}:")
            and line.strip().split(":", 1)[1].strip().startswith(
                f"{identity[f'{key}.release_artifact_prefix']}-"
            )
            for line in release.splitlines()
        ):
            problems.append(f"{key} release artifact prefix is missing from the desktop matrix")

    registered_desktop_binaries = {
        identity[f"desktop.{product}.binary_name"] for product in desktop_products
    }
    non_product_binaries = {
        name.strip()
        for name in identity["rust.non_product_binaries"].split(",")
        if name.strip()
    }
    discovered_binaries: set[str] = set()
    for manifest_path in sorted((root / "crates").glob("infiltrator-*/Cargo.toml")):
        try:
            manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
            package = manifest.get("package", {})
            targets = manifest.get("bin", [])
            binary_names = {target.get("name") for target in targets if target.get("name")}
            if not binary_names and (manifest_path.parent / "src/main.rs").is_file():
                binary_names.add(package.get("name", ""))
        except (OSError, tomllib.TOMLDecodeError) as error:
            problems.append(f"cannot inspect executable targets in {manifest_path}: {error}")
            continue
        for binary_name in binary_names:
            if binary_name in discovered_binaries:
                problems.append(f"Rust executable target name is duplicated: {binary_name}")
            discovered_binaries.add(binary_name)
            if binary_name not in registered_desktop_binaries | non_product_binaries:
                problems.append(
                    f"Rust executable {binary_name!r} has no product identity or explicit non-product classification"
                )
    missing_desktop_binaries = registered_desktop_binaries.difference(discovered_binaries)
    for binary_name in sorted(missing_desktop_binaries):
        problems.append(f"registered desktop product binary {binary_name!r} is not a Cargo executable target")
    unneeded_classifications = non_product_binaries.difference(discovered_binaries)
    for binary_name in sorted(unneeded_classifications):
        problems.append(f"non-product executable classification has no Cargo target: {binary_name!r}")

    nsis_builder_path = root / "packaging/windows/nsis/build-nsis.sh"
    nsis_builder = nsis_builder_path.read_text(encoding="utf-8")
    for token in (
        "scripts/product_identity.py",
        'identity_value version.name',
        'desktop.$product.windows_nsis_script',
        'desktop.$product.icon_ico',
        '"-DVERSION=$version"',
        '"-DICON_PATH=$icon_native"',
    ):
        if token not in nsis_builder:
            problems.append(f"NSIS builder no longer consumes product identity through {token}")

    desktop_prefixes = {
        identity[f"desktop.{product}.release_artifact_prefix"] for product in desktop_products
    }
    for line in release.splitlines():
        for field in ("artifact_name", "bevy_artifact_name", "slint_artifact_name"):
            if not line.strip().startswith(f"{field}:"):
                continue
            artifact_name = line.split(":", 1)[1].strip()
            if not any(artifact_name.startswith(f"{prefix}-") for prefix in desktop_prefixes):
                problems.append(f"desktop release matrix contains unregistered artifact {artifact_name!r}")
    for field in ("binary_name", "bevy_binary_name", "slint_binary_name"):
        product_names = {
            identity[f"desktop.{product}.binary_name"] for product in desktop_products
        }
        for line in release.splitlines():
            if line.strip().startswith(f"{field}:"):
                binary_name = line.split(":", 1)[1].strip().removesuffix(".exe")
                if binary_name not in product_names:
                    problems.append(f"desktop release matrix contains unregistered binary {binary_name!r}")

    iced = "desktop.iced"
    bevy_desktop = "desktop.bevy"
    plist_path = root / "packaging/macos/Info.plist"
    try:
        plist_template = plist_path.read_text(encoding="utf-8")
        for product, template in (
            (iced, plist_template),
            (bevy_desktop, plist_template.replace("desktop.iced.", "desktop.bevy.")),
            ("desktop.slint", plist_template.replace("desktop.iced.", "desktop.slint.")),
        ):
            plist = plistlib.loads(render_template(template, identity).encode("utf-8"))
            expected_plist = {
                "CFBundleExecutable": identity[f"{product}.binary_name"],
                "CFBundleIdentifier": identity[f"{product}.bundle_id"],
                "CFBundleName": identity[f"{product}.display_name"],
                "CFBundleDisplayName": identity[f"{product}.display_name"],
                "CFBundleIconFile": identity[f"{product}.macos_icon_filename"],
                "CFBundleShortVersionString": identity["version.name"],
                "CFBundleVersion": identity["version.name"],
            }
            for field, expected in expected_plist.items():
                if plist.get(field) != expected:
                    problems.append(f"macOS {product} Info.plist {field} differs from product identity")
    except (OSError, plistlib.InvalidFileException, ValueError) as error:
        problems.append(f"macOS Info.plist cannot be read: {error}")
    if not (root / "scripts/macos/create-app-icon.sh").is_file():
        problems.append("macOS icon generator is missing while macOS product identity remains registered")
    wix_path = root / "packaging/windows/wix/Package.wxs"
    wix = wix_path.read_text(encoding="utf-8")
    if 'Name="$(var.ProductDisplayName)"' not in wix:
        problems.append("WiX package name must resolve through the registered Iced display name")
    if 'Version="$(var.ProductVersion)"' not in wix:
        problems.append("WiX package version must resolve through the workspace version")
    if 'SourceFile="$(var.ProductIcon)"' not in wix or 'Icon="ApplicationIcon.ico"' not in wix:
        problems.append("WiX installer must embed and use the registered Windows icon")
    for builder in ("appimage", "deb"):
        builder_path = root / f"packaging/linux/{builder}/build-{builder}.sh"
        source = builder_path.read_text(encoding="utf-8")
        for token in (
            "scripts/product_identity.py",
            "identity_value binary_name",
            "identity_value linux_package_id",
            "identity_value linux_desktop_file",
            "identity_value icon_png",
            "scripts/render-product-template.py",
        ):
            if token not in source:
                problems.append(f"Linux {builder} builder does not derive metadata through {token}")
    if "APP_PRODUCT=bevy" not in release:
        problems.append("Linux release workflow must select the Bevy product through the registry")

    return problems


def apk_problems(
    product: str,
    apk_path: Path,
    aapt_path: Path,
    identity: dict[str, str],
) -> list[str]:
    if product not in product_ids(identity):
        return [f"unknown Android product {product!r}"]
    if not apk_path.is_file():
        return [f"{product} APK does not exist: {apk_path}"]
    try:
        result = subprocess.run(
            [str(aapt_path), "dump", "badging", str(apk_path)],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        return [f"cannot inspect {product} APK {apk_path}: {error}"]

    badging = result.stdout
    problems: list[str] = []
    package_line = next(
        (line for line in badging.splitlines() if line.startswith("package:")), ""
    )
    package_match = BADGING_PACKAGE.match(package_line)
    if package_match is None:
        return [f"{product} APK has no readable package/version badging"]

    expected_package = identity[f"{product}.application_id"]
    expected_version = identity["version.name"]
    expected_code = get_value(identity, f"{product}.version_code")
    expected_label = identity[f"{product}.display_name"]
    if package_match.group(1) != expected_package:
        problems.append(
            f"{product} APK package {package_match.group(1)!r} != {expected_package!r}"
        )
    if package_match.group(2) != expected_code:
        problems.append(
            f"{product} APK versionCode {package_match.group(2)!r} != {expected_code!r}"
        )
    if package_match.group(3) != expected_version:
        problems.append(
            f"{product} APK versionName {package_match.group(3)!r} != {expected_version!r}"
        )
    if f"application-label:'{expected_label}'" not in badging:
        problems.append(f"{product} APK launcher label is not {expected_label!r}")

    icon_lines = [line for line in badging.splitlines() if line.startswith("application-icon-")]
    icon_paths = [
        match.group(1)
        for line in icon_lines
        if (match := re.match(r"application-icon-\d+:'([^']+)'", line)) is not None
    ]
    if not icon_paths:
        problems.append(f"{product} APK has no readable launcher icon resource")
    else:
        source_icon_path = ROOT / identity[f"{product}.icon_asset"]
        if not source_icon_path.is_file():
            problems.append(f"{product} launcher icon source is missing: {source_icon_path}")
        else:
            source_icon = source_icon_path.read_bytes()
            source_dimensions = (
                struct.unpack("!II", source_icon[16:24])
                if len(source_icon) >= 24 and source_icon[:8] == b"\x89PNG\r\n\x1a\n"
                else None
            )
            try:
                with ZipFile(apk_path) as archive:
                    for icon_path in sorted(set(icon_paths)):
                        try:
                            packaged_icon = archive.read(icon_path)
                        except KeyError:
                            problems.append(
                                f"{product} APK badging references missing icon resource {icon_path!r}"
                            )
                            continue
                        packaged_dimensions = (
                            struct.unpack("!II", packaged_icon[16:24])
                            if len(packaged_icon) >= 24
                            and packaged_icon[:8] == b"\x89PNG\r\n\x1a\n"
                            else None
                        )
                        if source_dimensions is None or packaged_dimensions != source_dimensions:
                            problems.append(
                                f"{product} APK icon resource {icon_path!r} does not match "
                                "the registered PNG dimensions"
                            )
            except (BadZipFile, OSError) as error:
                problems.append(f"cannot inspect {product} APK icon resources: {error}")
    return problems


def release_tag_problems(identity: dict[str, str], tag: str) -> list[str]:
    expected_tag = f"v{identity['version.name']}"
    if tag == expected_tag:
        return []
    return [f"release tag {tag!r} must match {expected_tag!r}"]


def parse_apk(value: str) -> tuple[str, Path]:
    product, separator, path = value.partition("=")
    if not separator or not product or not path:
        raise argparse.ArgumentTypeError("APK must use PRODUCT=PATH syntax")
    return product, Path(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--apk", action="append", type=parse_apk, default=[])
    parser.add_argument("--aapt", type=Path)
    parser.add_argument("--release-tag")
    arguments = parser.parse_args()
    problems = source_problems()
    identity = read_properties()
    if arguments.release_tag:
        problems.extend(release_tag_problems(identity, arguments.release_tag))
    if arguments.apk and arguments.aapt is None:
        problems.append("--aapt is required when checking APK files")
    elif arguments.aapt is not None:
        for product, path in arguments.apk:
            problems.extend(apk_problems(product, path, arguments.aapt, identity))
    if problems:
        for problem in problems:
            print(f"Product identity: {problem}", file=sys.stderr)
        return 1
    print("Product identity guard: all declared Android and desktop products are aligned")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
