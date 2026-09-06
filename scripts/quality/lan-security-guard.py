#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-08 LAN ACL and Basic Auth."""

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
        "`DUAL-02-08` 局域网接入 ACL 与 HTTP 基本认证",
        "lan-allowed-ips",
        "lan-disallowed-ips",
        "skip-auth-prefixes",
        "authentication",
        "不发布密码",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/lan.rs",
        "LanCredentials",
        "LanSecuritySnapshot",
        "skip_serializing",
        "<redacted>",
        "authentication_username",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "SetLanSecurity {",
        "credentials: Option<LanCredentials>",
        "Self::SetLanSecurity { .. }",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/capability.rs",
        "LanAccessControl",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/lan_security.rs",
        "normalize_cidrs",
        "validate_credentials",
        "IpNet",
        "cidrs_are_canonicalized_and_deduplicated",
        "malformed_cidr_and_basic_auth_separators_fail_closed",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/runtime.rs",
        "lan_allowed_ips",
        "lan_disallowed_ips",
        "skip_auth_prefixes",
        "authentication_enabled",
        "authentication_user_count",
        "authentication_username",
    )
    require(
        violations,
        "crates/mihomo-api/src/types.rs",
        "lan-allowed-ips",
        "lan-disallowed-ips",
        "skip-auth-prefixes",
        "pub authentication: Vec<String>",
        "authentication_username",
    )
    require(
        violations,
        "crates/mihomo-api/src/types_test.rs",
        "lan-user:secret-value",
        "authentication_username",
    )
    require(
        violations,
        "crates/infiltrator-application/src/runtime_query_lan.rs",
        "pub async fn set_lan_security",
        "normalize_cidrs",
        "validate_credentials",
        '"lan-allowed-ips": allowed_ips.clone()',
        '"lan-disallowed-ips": disallowed_ips.clone()',
        '"skip-auth-prefixes": skip_auth_prefixes.clone()',
        '"authentication": authentication.clone()',
        "LAN security readback mismatch",
    )
    require(
        violations,
        "crates/infiltrator-application/src/runtime_query_application.rs",
        "lan_security_patch_reads_back_acl_and_only_exposes_auth_summary",
        "lan_security_requires_credentials_when_authentication_is_enabled",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "CommandIntent::SetLanSecurity",
        ".set_lan_security(",
        "credentials.as_ref()",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "lan_security: config.map_or_else",
        "value.authentication_user_count",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub lan_security: LanSecuritySnapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/app.rs",
        "LanSecurityConfig",
        "auth_password",
        "authentication_enabled",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "lan_security_committed",
        "lan_security_dirty",
        "settings.lan_security",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "UpdateLanAllowedIps(String)",
        "UpdateLanDisallowedIps(String)",
        "UpdateLanSkipAuthPrefixes(String)",
        "ToggleLanAuthentication(bool)",
        "UpdateLanAuthPassword(String)",
        "ApplyLanSecurity",
        "LanSecuritySet(",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave5.rs",
        "fn apply_lan_security",
        "split_cidrs",
        ".set_lan_security(",
        "self.runtime.lan_security.auth_password.clear()",
        "self.runtime.lan_security_dirty = false",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/lan_security_card.rs",
        "lan_security_card",
        ".secure(true)",
        "UpdateLanAuthPassword",
        "ApplyLanSecurity",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave5_tests.rs",
        "test_lan_security_readback_updates_iced_acl_and_clears_password",
        "LanSecuritySnapshot::new",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "SetLanSecurity {",
        "CommandIntent::SetLanSecurity",
        "LanCredentials",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_lan.rs",
        "LanAllowedIpsField",
        "LanDisallowedIpsField",
        "LanSkipAuthPrefixesField",
        "LanAuthenticationToggle",
        "LanAuthUsernameField",
        "LanAuthPasswordField",
        "LanSecurityApplyButton",
        "password_field_scene",
        "on_security_apply_activated",
        "UiCommand::SetLanSecurity",
        "TextFieldState::new(\"\")",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "settings_lan::on_security_apply_activated",
        "SettingsLineKind::LanSecurity",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_lan_security_submits_acl_and_redacted_basic_auth_intent",
        "secret-value",
        "LanSecurityApplyButton",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "Capability::LanAccessControl",
    )
    require(
        violations,
        "crates/infiltrator-android/src/runtime.rs",
        "Capability::LanAccessControl",
        "supports(Capability::LanAccessControl)",
    )
    require(
        violations,
        "crates/infiltrator-android/src/composition.rs",
        "CommandApplication",
        ".with_runtime(gateway)",
    )
    require(
        violations,
        "crates/infiltrator-ios/src/lib.rs",
        "Capability::LanAccessControl",
        "iOS controller gateway is not exposed by the native host",
        "!capabilities.supports(Capability::LanAccessControl)",
    )

    if violations:
        for violation in violations:
            print(f"lan-security-guard: {violation}", file=sys.stderr)
        print(f"lan-security-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("lan-security-guard: DUAL-02-08 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
