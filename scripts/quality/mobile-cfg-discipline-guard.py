#!/usr/bin/env python3
"""Enforce cross-platform conditional compilation discipline for UI and contract crates.

Discipline rules:
1. Shared ubiquitous tokens and contract types (e.g. `UiPalette`, `Theme`, `Token`,
   `WindowChrome`, `WindowChromeSupport`, `NativeShadow`) MUST NOT be wrapped in
   platform-excluding attributes like `#[cfg(not(target_os = "android"))]`.
2. Capabilities must be reported honestly: `crates/infiltrator-bevy-ui/src/chrome.rs`
   must declare `WindowChromeSupport::Unsupported` for `target_os = "android"`.
3. An unconditioned public function (`pub fn`) MUST NOT reference types in its signature
   that are exclusively imported under `#[cfg(not(target_os = "android"))]`.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[2]

# Shared theme, token and contract types that must remain unconditionally available.
PROTECTED_CROSS_PLATFORM_TYPES = {
    "UiPalette",
    "Theme",
    "Token",
    "ThemePreference",
    "ThemeSkin",
    "WindowChrome",
    "WindowChromeSupport",
    "NativeShadow",
}

CRATES_TO_CHECK = [
    Path("crates/infiltrator-bevy-ui/src"),
    Path("crates/infiltrator-bevy-widgets/src"),
    Path("crates/infiltrator-contract/src"),
]


def check_protected_type_exclusions(file_path: Path, content: str) -> list[str]:
    """Flag any `#[cfg(not(target_os = "android"))]` applied directly to protected imports."""
    problems: list[str] = []
    lines = content.splitlines()
    for idx, line in enumerate(lines):
        line_stripped = line.strip()
        if re.search(r'#\[cfg\(not\(target_os\s*=\s*"android"\)\)\]', line_stripped):
            # Check the next non-empty line for a protected type import
            for lookahead_idx in range(idx + 1, min(idx + 5, len(lines))):
                next_line = lines[lookahead_idx].strip()
                if not next_line:
                    continue
                if next_line.startswith("use "):
                    for ptype in PROTECTED_CROSS_PLATFORM_TYPES:
                        # Matches `use ...::UiPalette;` or `{..., UiPalette, ...}`
                        if re.search(r'\b' + ptype + r'\b', next_line):
                            problems.append(
                                f"{file_path}:{idx + 1}: protected cross-platform type `{ptype}` "
                                f"is excluded by `#[cfg(not(target_os = \"android\"))]` on line {lookahead_idx + 1}"
                            )
                break
    return problems


def check_chrome_capability_honesty(root: Path) -> list[str]:
    """Ensure chrome.rs honestly reports Unsupported on Android."""
    chrome_path = root / "crates/infiltrator-bevy-ui/src/chrome.rs"
    if not chrome_path.exists():
        return [f"{chrome_path}: required chrome capability file does not exist"]

    content = chrome_path.read_text(encoding="utf-8")
    problems: list[str] = []

    # Must contain target_os = "android" branch returning Unsupported
    has_android_unsupported = bool(
        re.search(
            r'target_os\s*=\s*"android".*?WindowChromeSupport::Unsupported',
            content,
            re.DOTALL,
        )
    )
    if not has_android_unsupported:
        problems.append(
            f"{chrome_path}: `support()` must honestly return `WindowChromeSupport::Unsupported` for `target_os = \"android\"`"
        )

    # Must contain not(target_os = "android") branch returning Hosted
    has_desktop_hosted = bool(
        re.search(
            r'not\(target_os\s*=\s*"android"\).*?WindowChromeSupport::Hosted',
            content,
            re.DOTALL,
        )
    )
    if not has_desktop_hosted:
        problems.append(
            f"{chrome_path}: `support()` must return `WindowChromeSupport::Hosted` for desktop (`not(target_os = \"android\")`)"
        )

    return problems


def check_unconditioned_pub_fn_signatures(file_path: Path, content: str) -> list[str]:
    """Check for public functions using types that were conditionally imported only for desktop."""
    problems: list[str] = []
    lines = content.splitlines()

    # Collect types imported ONLY with #[cfg(not(target_os = "android"))]
    desktop_only_types: set[str] = set()
    for idx, line in enumerate(lines):
        line_stripped = line.strip()
        if re.search(r'#\[cfg\(not\(target_os\s*=\s*"android"\)\)\]', line_stripped):
            for lookahead_idx in range(idx + 1, min(idx + 5, len(lines))):
                next_line = lines[lookahead_idx].strip()
                if not next_line:
                    continue
                if next_line.startswith("use "):
                    # Extract single imported identifier at the end, e.g. `use foo::bar::Baz;`
                    match = re.search(r'use\s+.*::(\w+);', next_line)
                    if match:
                        desktop_only_types.add(match.group(1))
                break

    if not desktop_only_types:
        return problems

    # Find unconditioned `pub fn` declarations
    for idx, line in enumerate(lines):
        line_stripped = line.strip()
        if line_stripped.startswith("pub fn ") and "(" in line_stripped:
            # Check attributes immediately preceding this fn declaration
            is_desktop_only_fn = False
            lookback_idx = idx - 1
            while lookback_idx >= 0:
                prev_line = lines[lookback_idx].strip()
                if not prev_line or prev_line.startswith("//") or prev_line.startswith("/*"):
                    lookback_idx -= 1
                    continue
                if prev_line.startswith("#["):
                    if 'not(target_os = "android")' in prev_line:
                        is_desktop_only_fn = True
                        break
                    lookback_idx -= 1
                    continue
                break
            if is_desktop_only_fn:
                continue

            # Check if any desktop_only_type appears in the parameter/return signature
            sig_text = line_stripped
            for dtype in desktop_only_types:
                if re.search(r'\b' + dtype + r'\b', sig_text):
                    problems.append(
                        f"{file_path}:{idx + 1}: public function `{line_stripped.split('(')[0]}` "
                        f"uses type `{dtype}` in its signature, but `{dtype}` is only imported on desktop"
                    )

    return problems


def find_all_problems(root: Path) -> list[str]:
    problems: list[str] = []
    problems.extend(check_chrome_capability_honesty(root))

    for crate_dir in CRATES_TO_CHECK:
        full_dir = root / crate_dir
        if not full_dir.exists():
            continue
        for rs_file in sorted(full_dir.rglob("*.rs")):
            try:
                content = rs_file.read_text(encoding="utf-8")
            except OSError as err:
                problems.append(f"{rs_file}: read error: {err}")
                continue
            problems.extend(check_protected_type_exclusions(rs_file.relative_to(root), content))
            problems.extend(check_unconditioned_pub_fn_signatures(rs_file.relative_to(root), content))

    return problems


def self_test() -> int:
    bad_code_1 = """
#[cfg(not(target_os = "android"))]
use infiltrator_bevy_widgets::palette::UiPalette;
"""
    probs_1 = check_protected_type_exclusions(Path("test.rs"), bad_code_1)
    if not probs_1 or "UiPalette" not in probs_1[0]:
        print("Self-test failed: protected type exclusion not detected", file=sys.stderr)
        return 1

    good_code_1 = """
use infiltrator_bevy_widgets::palette::UiPalette;
"""
    probs_good_1 = check_protected_type_exclusions(Path("test.rs"), good_code_1)
    if probs_good_1:
        print("Self-test failed: good code falsely flagged", file=sys.stderr)
        return 1

    bad_sig = """
#[cfg(not(target_os = "android"))]
use crate::desktop::DesktopHandle;

pub fn chrome_bar_scene(handle: &DesktopHandle) -> impl Scene {
}
"""
    probs_sig = check_unconditioned_pub_fn_signatures(Path("test.rs"), bad_sig)
    if not probs_sig or "DesktopHandle" not in probs_sig[0]:
        print("Self-test failed: unconditioned pub fn signature mismatch not detected", file=sys.stderr)
        return 1

    print("Mobile CFG discipline guard self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true", help="Run internal self-tests")
    parser.add_argument("--mode", choices=["enforce", "check"], default="enforce")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    problems = find_all_problems(ROOT)
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1

    print("Mobile conditional compilation discipline guard passed: all cross-platform types and capabilities honest.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
