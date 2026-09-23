#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-02 adaptive TUN MTU negotiation."""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    for marker in markers:
        if marker not in text:
            violations.append(f"{path} missing {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "`DUAL-02-02` 物理与虚拟网卡 MTU 自适应协商",
        "physical-link → domain negotiation → live Mihomo TUN MTU",
        "Android/iOS",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/mtu.rs",
        "pub struct PhysicalMtuSnapshot",
        "pub struct MtuNegotiationSnapshot",
        "applied_tun_mtu",
        "with_applied_tun_mtu",
        "with_failure",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/mtu_optimizer.rs",
        "negotiate_tun_mtu",
        "DEFAULT_TUN_OVERHEAD_BYTES",
        "test_negotiate_tun_mtu_from_physical_link",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/mtu_probe.rs",
        "pub trait MtuProbePort",
        "probe_physical_mtu",
    )
    require(
        violations,
        "crates/infiltrator-application/src/mtu_application.rs",
        "pub const MTU_PROBE_CACHE_TTL",
        "pub async fn probe_cached",
        "pub async fn probe_and_apply",
        "RuntimeQueryApplication::new",
    )
    require(
        violations,
        "crates/infiltrator-application/src/runtime_query_application.rs",
        "pub async fn set_tun_mtu",
        '"tun": { "mtu": mtu }',
        "TUN MTU readback mismatch",
        "tun_mtu_patch_is_read_back_before_success",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/runtime.rs",
        "pub mtu: Option<u32>",
    )
    require(
        violations,
        "crates/mihomo-api/src/types.rs",
        "pub mtu: Option<u32>",
        "mtu: tun.mtu",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "fn mtu_probe_port",
        "Option<Arc<dyn MtuProbePort>>",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/mtu.rs",
        "pub struct DesktopMtuProbe",
        "NetworkInterfaceWatcher::poll_interfaces",
        "default_route_interface",
        "parse_linux_link_mtu",
        "parse_linux_default_route",
        "parse_macos_default_route",
        "parse_windows_default_route",
        "parse_ifconfig_mtu",
        "parse_netsh_mtu",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime/ports.rs",
        "fn mtu_probe_port",
        "DesktopMtuProbe::new",
    )
    require(
        violations,
        "crates/mihomo-platform/src/android_bridge.rs",
        "fn physical_mtu",
    )
    require(
        violations,
        "crates/infiltrator-android/src/runtime.rs",
        "impl<B> MtuProbePort",
        "android-active-link",
        "android_host_keeps_mtu_probe_unsupported_without_native_link_metrics",
    )
    require(
        violations,
        "crates/infiltrator-ios/src/lib.rs",
        "fn physical_mtu",
        "impl MtuProbePort for IosHostAdapter",
        "ios-active-link",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "merge_applied_mtu",
        "runtime_config.as_ref()",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_services.rs",
        "probe_cached",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "pub struct MtuProbeCompletion",
        "session_token: Option<SessionToken>",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave5.rs",
        "probe_and_apply(gateway)",
        "MtuProbeCompletion",
        "completion.generation",
        "completion.session_token",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/tun_stack_card.rs",
        "applied_tun_mtu",
        "MTU: {name} / applied",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "ProbeTunMtu",
        "CommandIntent::ProbeTunMtu",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "ProbeTunMtuButton",
        "探测并协商",
        "applied_tun_mtu",
        "on_mtu_probe_activated",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave5_tests.rs",
        "test_tun_mtu_probe_result_updates_the_shared_iced_state",
        "MtuProbeCompletion",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_mtu_probe_submits_shared_application_command",
        "physical=1500",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_test.rs",
        "mtu_calls",
        "assert_eq!(mtu_calls.load(Ordering::SeqCst), 1)",
    )

    if violations:
        for violation in violations:
            print(f"mtu-negotiation-guard: {violation}", file=sys.stderr)
        print(f"mtu-negotiation-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("mtu-negotiation-guard: DUAL-02-02 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
