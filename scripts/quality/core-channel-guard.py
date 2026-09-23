#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-04 core release channel parity."""

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
        "crates/infiltrator-contract/src/version.rs",
        "CoreReleaseChannel",
        "Alpha",
        "MetaCore",
        "pub const ALL: [Self; 3]",
        "CoreChannelStatus",
        "CoreVersionSnapshot",
    )
    require(
        violations,
        "crates/mihomo-version/src/channel.rs",
        "Channel::Alpha",
        "Channel::MetaCore",
        "Prerelease-Alpha",
        "fetch_latest_alpha_picks_named_prerelease_over_http",
        "fetch_latest_meta_core_uses_published_meta_release_over_http",
    )
    require(
        violations,
        "crates/infiltrator-core/src/version_port.rs",
        "CoreReleaseChannel::Alpha => Channel::Alpha",
        "CoreReleaseChannel::MetaCore => Channel::MetaCore",
    )
    require(
        violations,
        "crates/infiltrator-application/src/version_application.rs",
        "probe_channels",
        "CoreChannelStatus::Ready",
        "channel_probe_keeps_successes_when_one_channel_fails",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_versions",
        "read_versions",
        "CoreChannelStatus::Ready",
        "versions,",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_test.rs",
        "surface_reader_publishes_and_caches_all_core_channel_results",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "VersionApplication",
        ".with_versions(versions)",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub versions: CoreVersionSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "CoreVersionSnapshot",
        "core_versions = snapshot.versions.clone()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/kernels.rs",
        "CoreReleaseChannel::Alpha",
        "CoreReleaseChannel::MetaCore",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings/format.rs",
        "CoreChannelStatus",
        "format_core_versions",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings/kernel.rs",
        "Online channels",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "shared_core_channel_probe_updates_the_iced_kernel_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "SettingsLineKind::CoreChannel",
        "SettingsLineKind::CoreVersions",
        "settings_core::format_core_versions",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "CoreChannelStatus",
        "format_core_versions",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "core_versions: snapshot.versions.clone()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_tests.rs",
        "CoreReleaseChannel::Alpha",
        "alpha=Prerelease-Alpha",
    )

    if violations:
        for violation in violations:
            print(f"core-channel-guard: {violation}", file=sys.stderr)
        print(f"core-channel-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("core-channel-guard: DUAL-01-04 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
