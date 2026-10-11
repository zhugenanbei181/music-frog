#!/usr/bin/env python3
"""Validate the production Android manifest contract for every installed flavor."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
ANDROID_NS = "http://schemas.android.com/apk/res/android"
ANDROID = f"{{{ANDROID_NS}}}"
ANDROID_NAME = f"{ANDROID}name"
ANDROID_VALUE = f"{ANDROID}value"

sys.path.insert(0, str(ROOT / "scripts"))
from product_identity import product_ids, read_properties


REQUIRED_PERMISSIONS = frozenset(
    {
        "android.permission.INTERNET",
        "android.permission.ACCESS_NETWORK_STATE",
        "android.permission.FOREGROUND_SERVICE",
        "android.permission.POST_NOTIFICATIONS",
        "android.permission.RECEIVE_BOOT_COMPLETED",
        "android.permission.FOREGROUND_SERVICE_SYSTEM_EXEMPTED",
    }
)

REQUIRED_CONFIG_CHANGES = frozenset(
    {
        "orientation",
        "screenSize",
        "screenLayout",
        "keyboardHidden",
        "mnc",
        "colorMode",
        "density",
        "fontScale",
        "fontWeightAdjustment",
        "keyboard",
        "layoutDirection",
        "locale",
        "navigation",
        "smallestScreenSize",
        "touchscreen",
        "uiMode",
    }
)


@dataclass(frozen=True)
class SourceManifest:
    source_set: str
    path: Path
    root: ET.Element


@dataclass(frozen=True)
class ServiceContract:
    relative_name: str
    attributes: tuple[tuple[str, str], ...]
    actions: frozenset[str] = frozenset()
    metadata: tuple[tuple[str, str], ...] = ()
    forbidden_attributes: tuple[str, ...] = ()


SERVICE_CONTRACTS = (
    ServiceContract(
        ".MihomoControlService",
        (("process", ":vpn"), ("exported", "false")),
        forbidden_attributes=("permission", "foregroundServiceType"),
    ),
    ServiceContract(
        ".MihomoVpnService",
        (
            ("process", ":vpn"),
            ("exported", "true"),
            ("permission", "android.permission.BIND_VPN_SERVICE"),
            ("foregroundServiceType", "systemExempted"),
        ),
        actions=frozenset({"android.net.VpnService"}),
        metadata=(("android.net.VpnService.SUPPORTS_ALWAYS_ON", "false"),),
    ),
    ServiceContract(
        ".InfiltratorTileService",
        (
            ("exported", "true"),
            ("permission", "android.permission.BIND_QUICK_SETTINGS_TILE"),
        ),
        actions=frozenset({"android.service.quicksettings.action.QS_TILE"}),
    ),
)

RECEIVER_CONTRACT = (".BootReceiver",)


def component_name(raw_name: str | None, namespace: str) -> str:
    if not raw_name:
        return "<missing android:name>"
    if raw_name.startswith("."):
        return f"{namespace}{raw_name}"
    if "." not in raw_name:
        return f"{namespace}.{raw_name}"
    return raw_name


def source_set_for(path: Path, app_src: Path) -> str:
    return path.parent.relative_to(app_src).as_posix()


def production_manifest_paths(app_src: Path) -> set[Path]:
    return {
        path
        for path in app_src.rglob("AndroidManifest.xml")
        if not {"test", "androidTest"}.intersection(path.relative_to(app_src).parts)
    }


def read_manifest(path: Path, source_set: str, problems: list[str]) -> SourceManifest | None:
    try:
        root = ET.parse(path).getroot()
    except (OSError, ET.ParseError) as error:
        problems.append(f"{path}: cannot read Android manifest: {error}")
        return None
    if root.tag != "manifest":
        problems.append(f"{path}: root element must be <manifest>")
        return None
    return SourceManifest(source_set, path, root)


def application_for(manifest: SourceManifest, problems: list[str]) -> ET.Element | None:
    applications = manifest.root.findall("application")
    if len(applications) != 1:
        problems.append(
            f"{manifest.path}: production manifest must contain exactly one <application>"
        )
        return None
    return applications[0]


def actions_for(component: ET.Element) -> list[str]:
    return [
        action.get(ANDROID_NAME, "")
        for intent_filter in component.findall("intent-filter")
        for action in intent_filter.findall("action")
    ]


def check_attributes(
    component: ET.Element,
    expected: tuple[tuple[str, str], ...],
    label: str,
    problems: list[str],
) -> None:
    for attribute, expected_value in expected:
        actual = component.get(f"{ANDROID}{attribute}")
        if actual != expected_value:
            problems.append(
                f"{label} android:{attribute} must be {expected_value!r}, got {actual!r}"
            )


def identity_contract(root: Path, problems: list[str]) -> tuple[str, tuple[str, ...], dict[str, Path], dict[str, str]] | None:
    try:
        identity = read_properties(root / "product-identity.properties")
        products = product_ids(identity)
        namespace = identity["android.namespace"]
        launchers = {
            product: identity[f"{product}.launcher_activity"]
            for product in products
        }
        manifests = {
            product: root / identity[f"{product}.launch_manifest"]
            for product in products
        }
    except (KeyError, OSError, ValueError) as error:
        problems.append(f"Android product identity cannot define manifest contract: {error}")
        return None

    for product in products:
        expected_path = root / "android" / "app" / "src" / product / "AndroidManifest.xml"
        if manifests[product] != expected_path:
            problems.append(
                f"product identity {product}.launch_manifest must point to {expected_path.relative_to(root)}"
            )
    if len(set(manifests.values())) != len(manifests):
        problems.append("each Android product must own a distinct launcher manifest")
    return namespace, products, manifests, launchers


def source_problems(root: Path = ROOT) -> list[str]:
    problems: list[str] = []
    contract = identity_contract(root, problems)
    if contract is None:
        return problems
    namespace, products, launcher_paths, launcher_names = contract
    app_src = root / "android" / "app" / "src"
    expected_paths = {app_src / "main" / "AndroidManifest.xml", *launcher_paths.values()}

    for path in sorted(expected_paths):
        if not path.is_file():
            problems.append(f"required production manifest is missing: {path.relative_to(root)}")
    for path in sorted(production_manifest_paths(app_src) - expected_paths):
        problems.append(
            f"unregistered production source-set manifest would alter the APK: {path.relative_to(root)}"
        )

    manifests: list[SourceManifest] = []
    for path in sorted(expected_paths):
        if not path.is_file():
            continue
        source_set = source_set_for(path, app_src)
        manifest = read_manifest(path, source_set, problems)
        if manifest is not None:
            manifests.append(manifest)

    parsed_by_source = {manifest.source_set: manifest for manifest in manifests}
    main_manifest = parsed_by_source.get("main")
    if main_manifest is None:
        return problems

    check_permissions(manifests, problems)
    applications = {
        manifest.source_set: application_for(manifest, problems)
        for manifest in manifests
    }
    main_application = applications.get("main")
    if main_application is not None:
        check_main_application(main_application, main_manifest, namespace, problems)
    check_services(applications, manifests, namespace, root, problems)
    check_boot_receiver(applications, manifests, namespace, problems)
    check_product_activities(
        applications,
        manifests,
        products,
        launcher_paths,
        launcher_names,
        namespace,
        root,
        problems,
    )
    return problems


def check_permissions(manifests: list[SourceManifest], problems: list[str]) -> None:
    occurrences: dict[str, list[tuple[str, Path]]] = {}
    for manifest in manifests:
        for tag in ("uses-permission", "uses-permission-sdk-23"):
            for permission in manifest.root.findall(tag):
                name = permission.get(ANDROID_NAME)
                if not name:
                    problems.append(f"{manifest.path}: <{tag}> must declare android:name")
                    continue
                occurrences.setdefault(name, []).append((manifest.source_set, manifest.path))

    actual = set(occurrences)
    for permission in sorted(REQUIRED_PERMISSIONS - actual):
        problems.append(f"required Android permission is missing: {permission}")
    for permission in sorted(actual - REQUIRED_PERMISSIONS):
        problems.append(f"unregistered Android permission declaration: {permission}")
    for permission, declarations in sorted(occurrences.items()):
        if len(declarations) > 1:
            paths = ", ".join(str(path) for _, path in declarations)
            problems.append(f"Android permission {permission} is declared more than once: {paths}")
        if declarations[0][0] != "main":
            problems.append(f"Android permission {permission} must be owned by the main manifest")


def check_main_application(
    application: ET.Element,
    manifest: SourceManifest,
    namespace: str,
    problems: list[str],
) -> None:
    expected_application = component_name(".MihomoApplication", namespace)
    actual_application = component_name(application.get(ANDROID_NAME), namespace)
    if actual_application != expected_application:
        problems.append(
            f"{manifest.path}: application android:name must be {expected_application!r}"
        )
    if application.get(f"{ANDROID}enableOnBackInvokedCallback") != "true":
        problems.append(
            f"{manifest.path}: application must enable Android system back callbacks"
        )


def validate_actions(
    component: ET.Element,
    expected: frozenset[str],
    label: str,
    problems: list[str],
) -> None:
    actions = actions_for(component)
    actual = set(actions)
    if len(actions) != len(actual):
        problems.append(f"{label} repeats an intent action")
    if actual != expected:
        problems.append(
            f"{label} intent actions must be {sorted(expected)!r}, got {sorted(actual)!r}"
        )


def validate_metadata(
    component: ET.Element,
    expected: tuple[tuple[str, str], ...],
    label: str,
    problems: list[str],
) -> None:
    actual: dict[str, list[str]] = {}
    for item in component.findall("meta-data"):
        name = item.get(ANDROID_NAME)
        if name:
            actual.setdefault(name, []).append(item.get(ANDROID_VALUE, ""))
    for name, expected_value in expected:
        values = actual.get(name, [])
        if values != [expected_value]:
            problems.append(
                f"{label} metadata {name!r} must appear once with value {expected_value!r}"
            )


def check_services(
    applications: dict[str, ET.Element | None],
    manifests: list[SourceManifest],
    namespace: str,
    root: Path,
    problems: list[str],
) -> None:
    expected_names = {
        component_name(contract.relative_name, namespace): contract
        for contract in SERVICE_CONTRACTS
    }
    occurrences: dict[str, list[tuple[str, ET.Element, Path]]] = {}
    manifest_by_source = {manifest.source_set: manifest for manifest in manifests}
    for source_set, application in applications.items():
        if application is None:
            continue
        manifest = manifest_by_source[source_set]
        for service in application.findall("service"):
            name = component_name(service.get(ANDROID_NAME), namespace)
            occurrences.setdefault(name, []).append((source_set, service, manifest.path))

    for name, declarations in sorted(occurrences.items()):
        if name not in expected_names:
            problems.append(f"stale or unregistered Android service declaration: {name}")
        if len(declarations) > 1:
            paths = ", ".join(str(path) for _, _, path in declarations)
            problems.append(f"Android service {name} is declared more than once: {paths}")

    main_path = root / "android" / "app" / "src" / "main" / "AndroidManifest.xml"
    for name, service_contract in expected_names.items():
        declarations = occurrences.get(name, [])
        if not declarations:
            problems.append(f"required Android service declaration is missing: {name}")
            continue
        for source_set, _, path in declarations:
            if source_set != "main" or path != main_path:
                problems.append(
                    f"{path.relative_to(root)} service {name} must be owned by the main manifest"
                )
        if len(declarations) != 1:
            continue
        source_set, service, path = declarations[0]
        label = f"{path.relative_to(root)} service {name}"
        check_attributes(
            service,
            service_contract.attributes,
            label,
            problems,
        )
        for attribute in service_contract.forbidden_attributes:
            if service.get(f"{ANDROID}{attribute}") is not None:
                problems.append(f"{label} must not declare android:{attribute}")
        if service_contract.actions:
            validate_actions(service, service_contract.actions, label, problems)
        if service_contract.metadata:
            validate_metadata(service, service_contract.metadata, label, problems)

        if name == component_name(".InfiltratorTileService", namespace):
            check_tile_identity(service, root, label, problems)


def check_tile_identity(service: ET.Element, root: Path, label: str, problems: list[str]) -> None:
    try:
        identity = read_properties(root / "product-identity.properties")
        icon_resources = {identity[f"{product}.icon_resource"] for product in product_ids(identity)}
        name_resources = {identity[f"{product}.name_resource"] for product in product_ids(identity)}
    except (KeyError, OSError, ValueError) as error:
        problems.append(f"Android product identity cannot validate quick-settings tile: {error}")
        return
    if len(icon_resources) != 1 or len(name_resources) != 1:
        problems.append("Android product flavors must share resource names used by the common quick-settings tile")
        return
    expected_icon = f"@drawable/{next(iter(icon_resources))}"
    expected_label = f"@string/{next(iter(name_resources))}"
    check_attributes(
        service,
        (("icon", expected_icon), ("label", expected_label)),
        label,
        problems,
    )


def check_boot_receiver(
    applications: dict[str, ET.Element | None],
    manifests: list[SourceManifest],
    namespace: str,
    problems: list[str],
) -> None:
    expected_name = component_name(RECEIVER_CONTRACT[0], namespace)
    occurrences: dict[str, list[tuple[str, ET.Element, Path]]] = {}
    manifest_by_source = {manifest.source_set: manifest for manifest in manifests}
    for source_set, application in applications.items():
        if application is None:
            continue
        manifest = manifest_by_source[source_set]
        for receiver in application.findall("receiver"):
            name = component_name(receiver.get(ANDROID_NAME), namespace)
            occurrences.setdefault(name, []).append((source_set, receiver, manifest.path))

    for name, declarations in sorted(occurrences.items()):
        if name != expected_name:
            problems.append(f"stale or unregistered Android receiver declaration: {name}")
        if len(declarations) > 1:
            paths = ", ".join(str(path) for _, _, path in declarations)
            problems.append(f"Android receiver {name} is declared more than once: {paths}")

    declarations = occurrences.get(expected_name, [])
    if not declarations:
        problems.append(f"required Android boot receiver declaration is missing: {expected_name}")
        return
    if len(declarations) != 1:
        return
    source_set, receiver, path = declarations[0]
    label = f"{path} receiver {expected_name}"
    if source_set != "main":
        problems.append(f"{label} must be owned by the main manifest")
    check_attributes(
        receiver,
        (("exported", "true"), ("enabled", "true")),
        label,
        problems,
    )
    validate_actions(receiver, frozenset({"android.intent.action.BOOT_COMPLETED"}), label, problems)
    categories = [
        category.get(ANDROID_NAME, "")
        for intent_filter in receiver.findall("intent-filter")
        for category in intent_filter.findall("category")
    ]
    if categories != ["android.intent.category.DEFAULT"]:
        problems.append(
            f"{label} must declare only android.intent.category.DEFAULT, got {categories!r}"
        )


def is_launcher_activity(activity: ET.Element) -> bool:
    return any(
        "android.intent.action.MAIN"
        in {action.get(ANDROID_NAME, "") for action in intent_filter.findall("action")}
        and "android.intent.category.LAUNCHER"
        in {category.get(ANDROID_NAME, "") for category in intent_filter.findall("category")}
        for intent_filter in activity.findall("intent-filter")
    )


def check_product_activities(
    applications: dict[str, ET.Element | None],
    manifests: list[SourceManifest],
    products: tuple[str, ...],
    launcher_paths: dict[str, Path],
    launcher_names: dict[str, str],
    namespace: str,
    root: Path,
    problems: list[str],
) -> None:
    manifest_by_source = {manifest.source_set: manifest for manifest in manifests}
    all_activities: dict[str, list[tuple[str, ET.Element, Path]]] = {}
    for source_set, application in applications.items():
        if application is None:
            continue
        manifest = manifest_by_source[source_set]
        activities = application.findall("activity")
        aliases = application.findall("activity-alias")
        if aliases:
            problems.append(f"{manifest.path}: product launcher aliases are not part of the manifest contract")
        if source_set == "main" and activities:
            problems.append(f"{manifest.path}: launcher activities must live in their product flavor manifest")
        if source_set in products and len(activities) != 1:
            problems.append(
                f"{manifest.path}: product flavor must declare exactly its registered launcher activity"
            )
        for activity in activities:
            name = component_name(activity.get(ANDROID_NAME), namespace)
            all_activities.setdefault(name, []).append((source_set, activity, manifest.path))

    for name, declarations in sorted(all_activities.items()):
        if len(declarations) > 1:
            paths = ", ".join(str(path) for _, _, path in declarations)
            problems.append(f"Android activity {name} is declared more than once: {paths}")
        if not any(is_launcher_activity(activity) for _, activity, _ in declarations):
            problems.append(f"stale or unregistered Android activity declaration: {name}")

    launchers_by_source: dict[str, list[tuple[str, ET.Element, Path]]] = {}
    for name, declarations in all_activities.items():
        for source_set, activity, path in declarations:
            if is_launcher_activity(activity):
                launchers_by_source.setdefault(source_set, []).append((name, activity, path))

    for product in products:
        source_set = launcher_paths[product].parent.name
        expected_name = component_name(launcher_names[product], namespace)
        declarations = all_activities.get(expected_name, [])
        if len(declarations) != 1:
            problems.append(
                f"{product} must declare exactly one registered activity {expected_name!r}"
            )
            continue
        declared_source, activity, path = declarations[0]
        if declared_source != source_set:
            problems.append(
                f"{product} activity {expected_name!r} must be declared in source set {source_set!r}"
            )
        if path != launcher_paths[product]:
            problems.append(f"{product} launcher activity must be in {launcher_paths[product].relative_to(root)}")
        source_launchers = launchers_by_source.get(source_set, [])
        if len(source_launchers) != 1 or source_launchers[0][0] != expected_name:
            problems.append(
                f"{product} manifest must expose exactly one MAIN/LAUNCHER activity named {expected_name!r}"
            )
        label = f"{path.relative_to(root)} activity {expected_name}"
        check_attributes(
            activity,
            (
                ("exported", "true"),
                ("label", "@string/app_name"),
                ("theme", "@style/Theme.Infiltrator"),
            ),
            label,
            problems,
        )
        actual_config_changes = set(
            (activity.get(f"{ANDROID}configChanges") or "").split("|")
        )
        if actual_config_changes != REQUIRED_CONFIG_CHANGES:
            problems.append(
                f"{label} android:configChanges must be {sorted(REQUIRED_CONFIG_CHANGES)!r}, "
                f"got {sorted(actual_config_changes)!r}"
            )
        if product == "compose":
            check_attributes(activity, (("launchMode", "singleTask"),), label, problems)
        if product == "bevy":
            check_metadata_value(
                activity,
                "android.app.lib_name",
                "infiltrator_bevy_android",
                label,
                problems,
            )


def check_metadata_value(
    component: ET.Element,
    name: str,
    expected_value: str,
    label: str,
    problems: list[str],
) -> None:
    values = [
        item.get(ANDROID_VALUE, "")
        for item in component.findall("meta-data")
        if item.get(ANDROID_NAME) == name
    ]
    if values != [expected_value]:
        problems.append(
            f"{label} metadata {name!r} must appear once with value {expected_value!r}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root")
    args = parser.parse_args()
    problems = source_problems(args.root.resolve())
    if problems:
        print("Android manifest contract violations:", file=sys.stderr)
        for problem in problems:
            print(f"- {problem}", file=sys.stderr)
        return 1
    print("Android manifest contract passed for all registered production flavors.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
