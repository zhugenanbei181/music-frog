#!/usr/bin/env python3
"""Cross-architecture build automation guard (PUB-006.10).

The neutral crates and the product lines are released for four desktop target
triples and two Android triples. The cross build automation is spread over
three entry points, and this guard proves that none of them loses a triple:

1. `.github/workflows/release.yml` builds the four desktop triples
   (x86_64 and aarch64 for Linux and for Windows);
2. `scripts/cross-check.sh` compiles the two aarch64 desktop triples locally,
   with zig and with the Windows cross toolchain;
3. `scripts/android-check.sh` compiles the two Android triples.

The guard is offline: it reads the three entry points and never invokes cargo.

Usage
-----
    python3 scripts/quality/cross-build-guard.py [--mode report|enforce]
    python3 scripts/quality/cross-build-guard.py --self-test
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# The four desktop triples of the release matrix.
DESKTOP_TRIPLES = (
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
)

# The two Android triples of the local Android check.
ANDROID_TRIPLES = (
    "aarch64-linux-android",
    "x86_64-linux-android",
)

# The entry point that owns each group of triples.
RELEASE_WORKFLOW = Path(".github/workflows/release.yml")
CROSS_CHECK = Path("scripts/cross-check.sh")
ANDROID_CHECK = Path("scripts/android-check.sh")

# The Apple targets stay deferred; the Apple guard owns that decision.
APPLE_TARGETS = ("aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-apple-ios")


def _read(path: Path) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def collect_violations() -> list[str]:
    violations: list[str] = []

    release = _read(RELEASE_WORKFLOW)
    for triple in DESKTOP_TRIPLES:
        if triple not in release:
            violations.append(
                f"{RELEASE_WORKFLOW}: the release matrix does not build `{triple}`"
            )

    cross_check = _read(CROSS_CHECK)
    for triple in ("aarch64-unknown-linux-gnu", "aarch64-pc-windows-msvc"):
        if triple not in cross_check:
            violations.append(
                f"{CROSS_CHECK}: the local cross check does not compile `{triple}`"
            )

    android_check = _read(ANDROID_CHECK)
    for triple in ANDROID_TRIPLES:
        if triple not in android_check:
            violations.append(
                f"{ANDROID_CHECK}: the Android check does not compile `{triple}`"
            )

    for triple in APPLE_TARGETS:
        for path in (RELEASE_WORKFLOW, CROSS_CHECK, ANDROID_CHECK):
            if triple in _read(path):
                violations.append(
                    f"{path}: the Apple target `{triple}` stays deferred and must not "
                    "appear in the cross build automation"
                )

    return violations


def self_test() -> int:
    assert len(DESKTOP_TRIPLES) == 4
    assert len(ANDROID_TRIPLES) == 2
    assert re.match(r"^[a-z0-9_]+(-[a-z0-9_]+)+$", DESKTOP_TRIPLES[0])
    print("cross build guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    violations = collect_violations()
    if not violations:
        print(
            "cross build guard: OK (four desktop triples, two aarch64 cross legs, "
            "two Android legs)"
        )
        return 0
    for violation in violations:
        print(f"cross build guard: {violation}", file=sys.stderr)
    print(f"cross build guard: violations={len(violations)}", file=sys.stderr)
    return 1 if args.mode == "enforce" else 0


if __name__ == "__main__":
    raise SystemExit(main())
