#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-05 kernel artifact integrity."""

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
        "crates/mihomo-version/src/verify.rs",
        "pub fn verify_bytes",
        "no SHA-256 digest published",
        "SHA-256 mismatch",
        "missing_digest_fails_closed",
        "tampered_bytes_fail",
    )
    require(
        violations,
        "crates/mihomo-version/src/download.rs",
        "verify::verify_bytes(archive, expected_digest, label)?",
        "install_archive_rejects_digest_mismatch_without_touching_disk",
        "install_archive_fails_closed_without_digest",
    )
    require(
        violations,
        "crates/mihomo-version/src/manager.rs",
        "fetch_asset_digest(version).await?",
        "Some(&expected_digest)",
        "digest verified, smoke check passed",
    )
    require(
        violations,
        "crates/infiltrator-core/src/version_port.rs",
        "CoreArtifactVerification",
        "fn verification",
        "CoreArtifactVerification::Verified",
        "CoreArtifactVerification::Rejected",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/version.rs",
        "CoreArtifactVerification",
        "pub verification: CoreArtifactVerification",
    )
    require(
        violations,
        "crates/infiltrator-application/src/version_application.rs",
        "verification: self.port.verification()",
        "probe_channels",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_versions",
        "surface_reader_publishes_and_caches_all_core_channel_results",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        ".with_versions(versions)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "core_integrity: CoreArtifactVerification",
        "core_integrity = snapshot.versions.verification.clone()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/kernels.rs",
        "CoreArtifactVerification::Verified",
        "CoreArtifactVerification::Rejected",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings.rs",
        "format_integrity",
        "Artifact integrity",
        "CoreArtifactVerification::Verified",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "shared_core_channel_probe_updates_the_iced_kernel_projection",
        "CoreArtifactVerification::Verified",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "SettingsLineKind::CoreIntegrity",
        "制品完整性 (SHA-256)",
        "format_integrity",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "CoreArtifactVerification::Verified",
        "CoreArtifactVerification::Rejected",
        "sanitize_text",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "core_integrity: snapshot.versions.verification.clone()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_tests.rs",
        "SettingsLineKind::CoreIntegrity",
        "已验证 (v1.19.30)",
    )

    if violations:
        for violation in violations:
            print(f"kernel-integrity-guard: {violation}", file=sys.stderr)
        print(f"kernel-integrity-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("kernel-integrity-guard: DUAL-01-05 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
