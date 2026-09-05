#!/usr/bin/env python3
"""Fail-closed parity guard for the two primary UI surfaces.

This guard intentionally checks architecture facts that a compiler cannot
prove: the canonical page vocabulary is present on both surfaces, each page
has a shared projection adapter, production Bevy routing never fabricates a
page projection, and the delivery evidence template exists.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[2]

PAGES = {
    "Overview": {"snake": "overview", "iced": ["view/overview.rs"], "bevy": "overview.rs", "projection": "projection.rs"},
    "Proxies": {"snake": "proxies", "iced": ["view/proxies.rs", "view/runtime.rs"], "bevy": "proxies.rs", "projection": "proxies.rs"},
    "Profiles": {"snake": "profiles", "iced": ["view/profiles.rs"], "bevy": "profiles.rs", "projection": "profiles.rs"},
    "Rules": {"snake": "rules", "iced": ["view/rules.rs"], "bevy": "rules.rs", "projection": "rules.rs"},
    "Connections": {"snake": "connections", "iced": ["view/runtime/connections.rs", "view/runtime.rs"], "bevy": "connections.rs", "projection": "connections.rs"},
    "Logs": {"snake": "logs", "iced": ["view/runtime/logs.rs", "view/runtime.rs"], "bevy": "logs.rs", "projection": "logs.rs"},
    "Dns": {"snake": "dns", "iced": ["view/dns.rs"], "bevy": "dns.rs", "projection": "dns.rs"},
    "Doctor": {"snake": "doctor", "iced": ["view/doctor.rs"], "bevy": "doctor.rs", "projection": "doctor.rs"},
    "AppRouting": {"snake": "app_routing", "iced": ["view/app_routing.rs"], "bevy": "app_routing.rs", "projection": "app_routing.rs"},
    "Sync": {"snake": "sync", "iced": ["view/sync.rs"], "bevy": "sync.rs", "projection": "sync.rs"},
    "Settings": {"snake": "settings", "iced": ["view/settings.rs"], "bevy": "settings.rs", "projection": "settings.rs"},
}

REQUIRED_INTENTS = {
    "StartCore",
    "StopCore",
    "RestartCore",
    "PrepareServiceMode",
    "SetCoreLogLevel",
    "SwitchProfile",
    "SetProxyMode",
    "SelectProxyNode",
    "TestDelay",
    "UpdateProfile",
    "DeleteProfile",
    "RefreshRuleProviders",
    "CloseConnection",
    "CloseAllConnections",
    "ClearLogs",
    "SetLogLevelFilter",
    "ClearDnsCache",
    "TestDnsLatency",
    "RunDoctorDiagnostics",
    "RepairDoctorIssue",
    "RepairAllDoctorIssues",
    "ToggleTun",
    "SetSystemProxy",
    "ToggleAppRouting",
    "SetAppRoutingMode",
    "ToggleIncludeSystemApps",
    "SetAppRule",
    "SyncNow",
    "CreateBackupSnapshot",
    "ResolveConflictKeepLocal",
    "ResolveConflictTakeRemote",
    "RestoreSnapshot",
    "RollbackCore",
    "UpdateSetting",
    "CheckUpdates",
}


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def fail(violations: list[str], message: str) -> None:
    violations.append(message)


def check_page_vocabulary(violations: list[str]) -> None:
    contract = read("crates/infiltrator-contract/src/surface_snapshot.rs")
    bevy_route = read("crates/infiltrator-bevy-ui/src/route.rs")
    iced_route = read("crates/infiltrator-iced/src/types/app.rs")
    bevy_surface = read("crates/infiltrator-bevy-ui/src/surface.rs")

    for name, info in PAGES.items():
        if not re.search(rf"\b{name}\b", contract):
            fail(violations, f"contract page missing: {name}")
        if not re.search(rf"Route::{name}\b", bevy_route):
            fail(violations, f"Bevy route missing: {name}")
        iced_name = "Runtime" if name in {"Connections", "Logs"} else name
        if not re.search(rf"\b{iced_name}\b", iced_route):
            fail(violations, f"Iced route missing: {name}")
        projection_file = ROOT / "crates/infiltrator-bevy-ui/src" / info["projection"]
        projection_text = projection_file.read_text(encoding="utf-8") if projection_file.exists() else ""
        if not re.search(rf"(?:fn {info['snake']}_projection|pub struct {name}Projection\b)", bevy_surface + projection_text):
            fail(violations, f"Bevy shared projection adapter missing: {name}")
        bevy_page = ROOT / "crates/infiltrator-bevy-ui/src/pages" / info["bevy"]
        if not bevy_page.exists():
            fail(violations, f"Bevy page file missing: {bevy_page}")
        else:
            page_text = bevy_page.read_text(encoding="utf-8")
            projection_decl = page_text if name != "Overview" else bevy_surface + projection_text
            if not re.search(rf"pub struct {name}Projection\b", projection_decl):
                fail(violations, f"Bevy page projection missing: {name}")
            if not re.search(rf"pub struct {name}ProjectionUpdated\b", page_text):
                fail(violations, f"Bevy page update event missing: {name}")
        if not any((ROOT / "crates/infiltrator-iced/src" / path).exists() for path in info["iced"]):
            fail(violations, f"Iced page view missing: {name}")

    all_page_entries = re.findall(r"\b(Self::[A-Za-z]+),", contract[contract.find("pub const ALL"):contract.find("pub const ALL") + 1200])
    if len(all_page_entries) != len(PAGES):
        fail(violations, f"contract PageId::ALL expected {len(PAGES)} entries, found {len(all_page_entries)}")


def check_intents(violations: list[str]) -> None:
    contract = read("crates/infiltrator-contract/src/command.rs")
    bevy_command = read("crates/infiltrator-bevy-ui/src/command.rs")
    for intent in sorted(REQUIRED_INTENTS):
        if not re.search(rf"\b{intent}\b", contract):
            fail(violations, f"shared CommandIntent missing: {intent}")
        if not re.search(rf"\b{intent}\b", bevy_command):
            fail(violations, f"Bevy command mapping missing: {intent}")


def check_production_bevy_boundary(violations: list[str]) -> None:
    route = read("crates/infiltrator-bevy-ui/src/route.rs")
    if "Projection::demo()" in route or "::demo()" in route:
        fail(violations, "production Bevy route contains a demo projection")
    for symbol in [
        "SurfaceSourceHandle",
        "SurfaceSnapshotUpdated",
        "LatestSurfaceSnapshot",
        "trigger_page_projection_events",
    ]:
        if symbol not in route:
            fail(violations, f"production Bevy route missing shared surface seam: {symbol}")

    surface_source = read("crates/infiltrator-bevy-ui/src/surface.rs")
    for symbol in ["ApplicationSurfaceSource", "SurfaceDrainPlugin", "UnavailableSurfaceSource"]:
        if symbol not in surface_source:
            fail(violations, f"Bevy surface source seam missing: {symbol}")


def check_shared_application_seam(violations: list[str]) -> None:
    contract = read("crates/infiltrator-contract/src/surface_snapshot.rs")
    ports = read("crates/infiltrator-ports/src/surface.rs")
    application = read("crates/infiltrator-application/src/surface_application.rs")
    reader = read("crates/infiltrator-application/src/surface_reader.rs")
    for symbol in ["PageStatus", "CapabilitySnapshot", "SurfaceEvent", "SurfaceSnapshot"]:
        if symbol not in contract:
            fail(violations, f"shared surface contract symbol missing: {symbol}")
    if "trait SurfaceReader" not in ports:
        fail(violations, "runtime-neutral SurfaceReader port is missing")
    for symbol in ["SurfacePump", "SurfacePumpBridge", "SNAPSHOT_CAPACITY"]:
        if symbol not in application:
            fail(violations, f"application surface pump symbol missing: {symbol}")
    for page in PAGES.values():
        field = f"pages.{page['snake']}"
        if field not in reader:
            fail(violations, f"application surface reader does not assemble {field}")


def check_test_evidence(violations: list[str]) -> None:
    bevy_test_path = ROOT / "crates/infiltrator-bevy-ui/tests/headless/surface_tests.rs"
    iced_test_path = ROOT / "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs"
    if not bevy_test_path.exists():
        fail(violations, f"Bevy shared surface test missing: {bevy_test_path}")
    elif "shared_snapshot_reaches_all_eleven_page_lanes" not in bevy_test_path.read_text(encoding="utf-8"):
        fail(violations, "Bevy shared surface fan-out test missing")
    if not iced_test_path.exists():
        fail(violations, f"Iced shared surface test missing: {iced_test_path}")
    else:
        iced_tests = iced_test_path.read_text(encoding="utf-8")
        for marker in ["stale_surface_events_are_rejected", "surface_bridge_translates"]:
            if marker not in iced_tests:
                fail(violations, f"Iced shared surface test missing: {marker}")


def check_host_entries(violations: list[str]) -> None:
    desktop_surface = read("crates/infiltrator-desktop/src/surface.rs")
    iced_lib = read("crates/infiltrator-iced/src/lib.rs")
    bevy_lib = read("crates/infiltrator-bevy-ui/src/lib.rs")
    for symbol in ["desktop_capabilities", "application_surface_reader", "surface_pump"]:
        if symbol not in desktop_surface:
            fail(violations, f"desktop surface composition symbol missing: {symbol}")
    if "run_with_surface_pump" not in iced_lib:
        fail(violations, "Iced public surface pump entry is missing")
    if "run_with_application_surface_pump" not in bevy_lib:
        fail(violations, "Bevy public surface pump entry is missing")


def check_iced_surface_boundary(violations: list[str]) -> None:
    iced_surface = read("crates/infiltrator-iced/src/surface.rs")
    iced_state = read("crates/infiltrator-iced/src/state.rs")
    iced_boot = read("crates/infiltrator-iced/src/desktop_composition.rs")
    for symbol in ["SurfaceModel", "SurfaceBridge", "SurfaceSnapshotUpdated", "subscription"]:
        if symbol not in iced_surface:
            fail(violations, f"Iced shared surface adapter missing: {symbol}")
    if "pub surface:" not in iced_state:
        fail(violations, "Iced AppState does not retain the shared surface model")
    if "surface_bridge: Option" not in iced_state or "attach_surface_bridge" not in iced_state:
        fail(violations, "Iced AppState does not expose the composed surface bridge")
    if "pub fn run()" not in iced_boot:
        fail(violations, "Iced desktop composition root is missing")
    if "run_with_surface_pump" not in iced_boot:
        fail(violations, "Iced desktop composition has no surface pump entry")

    allowed_host_files = {
        "host.rs",
        "desktop_composition.rs",
        "admin_server.rs",
        "notify.rs",
    }
    iced_root = ROOT / "crates/infiltrator-iced/src"
    for path in iced_root.rglob("*.rs"):
        if path.name in allowed_host_files or "tray" in path.parts:
            continue
        text = path.read_text(encoding="utf-8")
        if "infiltrator_desktop::" in text or "infiltrator_admin::" in text:
            fail(violations, f"Iced UI module reaches concrete host directly: {path.relative_to(ROOT)}")


def check_runtime_neutrality(violations: list[str]) -> None:
    for crate in [
        "infiltrator-domain",
        "infiltrator-contract",
        "infiltrator-ports",
        "infiltrator-application",
        "infiltrator-bevy-ui",
    ]:
        manifest = read(f"crates/{crate}/Cargo.toml")
        if crate in {"infiltrator-domain", "infiltrator-contract", "infiltrator-ports", "infiltrator-application", "infiltrator-bevy-ui"}:
            document = tomllib.loads(manifest)
            deps = set(document.get("dependencies", {}))
            forbidden = {"tokio", "reqwest", "mihomo-api"} if crate == "infiltrator-bevy-ui" else {"tokio", "reqwest", "bevy", "iced", "mihomo-api"}
            leaked = sorted(forbidden.intersection(deps))
            if leaked:
                fail(violations, f"{crate} direct boundary dependencies: {', '.join(leaked)}")


def check_delivery_template(violations: list[str]) -> None:
    template = ROOT / "docs/DUAL_SURFACE_DELIVERY_TEMPLATE.md"
    if not template.exists():
        fail(violations, "dual-surface delivery template is missing")
        return
    text = template.read_text(encoding="utf-8")
    for marker in [
        "shared/application",
        "Iced",
        "Bevy",
        "headless",
        "host evidence",
        "parity-ready",
    ]:
        if marker not in text:
            fail(violations, f"delivery template missing required marker: {marker}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []
    for check in [
        check_page_vocabulary,
        check_intents,
        check_production_bevy_boundary,
        check_shared_application_seam,
        check_test_evidence,
        check_host_entries,
        check_iced_surface_boundary,
        check_runtime_neutrality,
        check_delivery_template,
    ]:
        check(violations)
    if violations:
        for violation in violations:
            print(f"parity-guard: {violation}", file=sys.stderr)
        print(f"parity-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print(f"parity-guard: pages={len(PAGES)} intents={len(REQUIRED_INTENTS)} violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
