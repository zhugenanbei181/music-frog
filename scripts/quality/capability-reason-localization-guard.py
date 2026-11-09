#!/usr/bin/env python3
"""Keep capability reasons typed, localized and free of legacy prose."""

from __future__ import annotations

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
CONTRACT = ROOT / "crates/infiltrator-contract/src/capability.rs"
PROJECTION = ROOT / "crates/infiltrator-presentation/src/capability_reason_projection.rs"
SHARED_COPY = ROOT / "crates/infiltrator-contract/src/shared_copy.rs"
SLINT = ROOT / "crates/infiltrator-slint/src/lumen_projection.rs"
LOCALES = (
    ROOT / "crates/infiltrator-shared/src/locales_table_shared_copy_zh.rs",
    ROOT / "crates/infiltrator-shared/src/locales_table_shared_copy_en.rs",
)


def problems() -> list[str]:
    findings: list[str] = []
    contract = CONTRACT.read_text(encoding="utf-8")
    projection = PROJECTION.read_text(encoding="utf-8")
    shared_copy = SHARED_COPY.read_text(encoding="utf-8")
    slint = SLINT.read_text(encoding="utf-8")
    tables = [path.read_text(encoding="utf-8") for path in LOCALES]

    if '"capability not registered"' in contract:
        findings.append("contract still publishes user-facing English prose")
    if "pub enum CapabilityReason" not in contract or "NotRegistered" not in contract:
        findings.append("contract lost the typed capability reason")
    if 'Self::NotRegistered => "capability_not_registered"' not in contract:
        findings.append("typed reason no longer publishes its stable key")
    if "CapabilityReason::ALL" not in projection or "catalog.text(key)" not in projection:
        findings.append("presentation no longer resolves typed reasons through the catalog")
    if "SharedCopyKey::CapabilityNotRegistered" not in shared_copy:
        findings.append("shared-copy registry does not declare the capability reason key")
    if any('"capability_not_registered"' not in table for table in tables):
        findings.append("shared-copy catalogs are missing the capability reason key")
    if "reason_copy(&reason, &lang)" not in slint:
        findings.append("Slint no longer routes capability reasons through the shared projection")
    if "Availability::Unsupported { reason } | Availability::Unavailable { reason } => reason" in slint:
        findings.append("Slint still renders raw capability reasons")
    return findings


def self_test() -> None:
    assert problems() == [], problems()
    sources = {
        CONTRACT: CONTRACT.read_text(encoding="utf-8"),
        PROJECTION: PROJECTION.read_text(encoding="utf-8"),
        SHARED_COPY: SHARED_COPY.read_text(encoding="utf-8"),
        SLINT: SLINT.read_text(encoding="utf-8"),
        LOCALES[0]: LOCALES[0].read_text(encoding="utf-8"),
        LOCALES[1]: LOCALES[1].read_text(encoding="utf-8"),
    }
    original = dict(sources)
    try:
        CONTRACT.write_text(
            sources[CONTRACT].replace(
                "CapabilityReason::NotRegistered.key().to_string()",
                '"capability not registered".to_string()',
            ),
            encoding="utf-8",
        )
        assert any("English prose" in finding for finding in problems())
        CONTRACT.write_text(original[CONTRACT], encoding="utf-8")

        SLINT.write_text(
            sources[SLINT].replace("reason_copy(&reason, &lang)", "reason"),
            encoding="utf-8",
        )
        assert any("shared projection" in finding for finding in problems())
        SLINT.write_text(
            original[SLINT].replace(
                "reason_copy(&reason, &lang)",
                "Availability::Unsupported { reason } | Availability::Unavailable { reason } => reason",
            ),
            encoding="utf-8",
        )
        assert any("raw capability reasons" in finding for finding in problems())
    finally:
        for path, source in original.items():
            path.write_text(source, encoding="utf-8")
    print("capability-reason-localization guard self-test: PASS (3 planted regressions)")


def main() -> int:
    if "--self-test" in sys.argv:
        self_test()
        return 0
    findings = problems()
    for finding in findings:
        print(f"VIOLATION [enforce]: {finding}")
    print(f"capability-reason-localization guard: findings={len(findings)}")
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
