#!/usr/bin/env python3
"""Guard the Core-first PublicIpProbe owner chain.

Usage:
    python3 scripts/quality/public-ip-probe-owner-guard.py --self-test
    python3 scripts/quality/public-ip-probe-owner-guard.py --mode enforce
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

CONTRACT = ROOT / "crates/infiltrator-contract/src/public_ip_probe.rs"
APPLICATION = ROOT / "crates/infiltrator-application/src/public_ip_probe_application.rs"
DISPATCH = ROOT / "crates/infiltrator-application/src/command_application/dispatch.rs"
SURFACE_READER = ROOT / "crates/infiltrator-composition/src/surface_reader.rs"
COMPOSITION = ROOT / "crates/infiltrator-composition/src/surface_reader/composition.rs"

FORBIDDEN = {
    DISPATCH: [
        (r"CommandIntent::RefreshPublicIpProbe\s*\|\s*CommandIntent::", "RefreshPublicIpProbe fell into an unsupported group"),
    ],
    SURFACE_READER: [
        (r"public_ip:\s*PublicIpProbeSnapshot::default\(\)", "surface reader fabricates the public IP readback"),
    ],
    APPLICATION: [
        (r"PublicIpProbeStatus::Probing\s*\{", "application uses a struct status instead of the sealed state"),
    ],
}

TIMESTAMP_SCAN_ROOTS = ()
TIMESTAMP_SCAN_FILES = (APPLICATION,)

REQUIRED = {
    CONTRACT: [
        r"pub enum PublicIpProbeState",
        r"Probing\s*\{\s*stamp:\s*PublicIpProbeStamp,\s*\}",
        r"pub struct PublicIpProbeStamp",
        r"PublicIpProbeRouteStamp",
        r"pub fn accept\(",
    ],
    APPLICATION: [
        r"guard\.begin\(stamp\)",
        r"\.accept\(",
        r"PublicIpProbeAnswer::Failed",
    ],
    DISPATCH: [
        r"CommandIntent::RefreshPublicIpProbe\s*=>",
        r"owner\.refresh\(owner\.refresh_route\(\)\)",
    ],
    SURFACE_READER: [
        r"public_ip_probe",
        r"\.snapshot\(\)",
    ],
    COMPOSITION: [
        r"pub fn with_public_ip_probe",
    ],
}


def findings(files: dict[pathlib.Path, str]) -> list[str]:
    violations: list[str] = []
    for path, text in files.items():
        relative = path.relative_to(ROOT).as_posix() if path.is_absolute() else path.as_posix()
        under_root = any(str(path).startswith(str(root)) for root in TIMESTAMP_SCAN_ROOTS)
        if (under_root or path in TIMESTAMP_SCAN_FILES) and path.name != "public_ip_probe_test.rs" and "1741300000000" in text:
            violations.append(f"{relative}: production hardcodes the public IP check timestamp")
    for path, patterns in FORBIDDEN.items():
        text = files.get(path, (path.read_text(encoding="utf-8") if path.exists() else ""))
        for pattern, message in patterns:
            if re.search(pattern, text, re.S):
                violations.append(f"{path.relative_to(ROOT)}: {message}")
    for path, patterns in REQUIRED.items():
        text = files.get(path, (path.read_text(encoding="utf-8") if path.exists() else ""))
        for pattern in patterns:
            if not re.search(pattern, text, re.S):
                violations.append(f"{path.relative_to(ROOT)}: missing required owner-chain marker `{pattern}`")
    return violations


def baseline() -> dict[pathlib.Path, str]:
    paths = set(REQUIRED.keys()) | set(FORBIDDEN.keys())
    for root in TIMESTAMP_SCAN_ROOTS:
        paths.update(root.rglob("*.rs"))
    paths.update(TIMESTAMP_SCAN_FILES)
    return {path: path.read_text(encoding="utf-8") for path in paths}


def planted_cases() -> list[tuple[str, dict[pathlib.Path, str]]]:
    cases: list[tuple[str, dict[pathlib.Path, str]]] = []
    source = baseline()

    files = dict(source)
    files[DISPATCH] = files[DISPATCH].replace(
        "CommandIntent::RefreshPublicIpProbe => {",
        "CommandIntent::RefreshPublicIpProbe | CommandIntent::ClearLogs => {",
        1,
    )
    cases.append(("unsupported dispatch group", files))

    files = dict(source)
    files[SURFACE_READER] = files[SURFACE_READER].replace(
        "            public_ip,\n",
        "            public_ip: PublicIpProbeSnapshot::default(),\n",
        1,
    )
    cases.append(("default readback", files))

    files = dict(source)
    files[APPLICATION] = files[APPLICATION] + "\nconst LEGACY: Option<u64> = Some(1741300000000);\n"
    cases.append(("hardcoded timestamp", files))

    files = dict(source)
    files[APPLICATION] = files[APPLICATION].replace(
        "            match guard.begin(stamp) {",
        "            match Err(PublicIpProbeError::NoActiveRequest) {",
        1,
    )
    cases.append(("no Probing production write", files))

    files = dict(source)
    files[COMPOSITION] = files[COMPOSITION].replace(
        "    pub fn with_public_ip_probe(",
        "    fn __retired_with_public_ip_probe(",
        1,
    )
    cases.append(("no composition owner installation", files))

    files = dict(source)
    files[CONTRACT] = files[CONTRACT].replace(
        "    Probing {\n        stamp: PublicIpProbeStamp,\n    },",
        "    __RetiredProbing {\n        stamp: PublicIpProbeStamp,\n    },",
    ).replace(
        "pub fn accept(self, answer: PublicIpProbeAnswer) -> Result<Self, PublicIpProbeError> {",
        "pub fn __retired_accept(self, answer: PublicIpProbeAnswer) -> Result<Self, PublicIpProbeError> {",
        1,
    )
    cases.append(("sealed Probing state removed", files))
    return cases


def self_test() -> bool:
    clean = findings(baseline())
    if clean:
        print("public-ip-probe owner guard self-test: clean baseline failed", file=sys.stderr)
        for violation in clean:
            print(f"  {violation}", file=sys.stderr)
        return False
    for name, files in planted_cases():
        found = findings(files)
        if not found:
            print(f"public-ip-probe owner guard self-test: planted case not detected: {name}", file=sys.stderr)
            return False
    print(f"public-ip-probe owner guard self-test: PASS ({len(planted_cases())} planted regressions)")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    args = parser.parse_args()
    if args.self_test:
        return 0 if self_test() else 1
    violations = findings(baseline())
    if violations:
        print("public-ip-probe owner guard violations:", file=sys.stderr)
        for violation in violations:
            print(f"  {violation}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("public-ip-probe owner guard: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
