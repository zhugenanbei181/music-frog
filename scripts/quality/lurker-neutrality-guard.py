#!/usr/bin/env python3
"""Keep the Lurker neutral crates engine-, platform- and UI-neutral.

The product architecture is fixed: two backends (mihomo / sing-box) × every
platform (Windows/Linux/macOS/Android/iOS) × every UI (Iced/Bevy/Slint/Compose).
A neutral framework crate may describe the domain, but it must never depend on a
product crate or a UI toolkit — and the pure layers must contain no OS-specific
code either. Driver crates (`mihomo-driver`, `singbox-driver`) may own per-OS process
code, but still may not pull in the product or a UI toolkit.
"""

from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]

# Pure logic layers: no product crate, no UI toolkit, no OS-specific code.
PURE_NEUTRAL = (
    "lurker-core-contract",
    "lurker-core-ports",
    "lurker-profile-store",
    "lurker-engine-runtime",
)
# Engine drivers: may own per-OS process code, but stay product/UI neutral.
DRIVERS = ("mihomo-driver", "singbox-driver")
ALL_NEUTRAL = PURE_NEUTRAL + DRIVERS

UI_TOOLKIT = re.compile(
    r"\b(?:use\s+(?:bevy|iced|slint|winit|egui|zhuge_ui)\b"
    r"|(?:bevy|iced|slint|winit|egui|zhuge_ui)::)"
)
PLATFORM_CODE = re.compile(r"(?:std::os::|cfg\(\s*target_os|keyring::|dirs::)")

# Engine adapter crates are product-side. A driver owns the engine logic itself,
# so depending back on an adapter crate would create a second implementation.
ENGINE_ADAPTER_PREFIXES = ("mihomo-", "singbox-")
ENGINE_ADAPTER_IMPORT = re.compile(
    r"\b(?:mihomo|singbox)_(?:api|config|process|version)::"
)


def check(root: Path = ROOT) -> list[str]:
    problems: list[str] = []
    for name in ALL_NEUTRAL:
        crate = root / "crates" / name
        if not crate.is_dir():
            # Not created yet; milestone order decides when it must exist.
            continue

        manifest = tomllib.loads((crate / "Cargo.toml").read_text(encoding="utf-8"))
        dependencies: dict[str, object] = {}
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            dependencies.update(manifest.get(section, {}))
        for dependency in dependencies:
            if dependency.startswith("infiltrator-"):
                problems.append(f"{name}: must not depend on product crate `{dependency}`")
            if name in DRIVERS and dependency.startswith(ENGINE_ADAPTER_PREFIXES):
                problems.append(
                    f"{name}: driver must not depend on engine adapter crate `{dependency}`"
                )

        for source in sorted(crate.rglob("*.rs")):
            text = source.read_text(encoding="utf-8", errors="ignore")
            relative = source.relative_to(root)
            ui = UI_TOOLKIT.search(text)
            if ui is not None:
                problems.append(f"{relative}: neutral crate references UI toolkit `{ui.group(0)}`")
            if name in PURE_NEUTRAL:
                platform = PLATFORM_CODE.search(text)
                if platform is not None:
                    problems.append(
                        f"{relative}: pure neutral crate contains platform code `{platform.group(0)}`"
                    )
            if name in DRIVERS:
                adapter = ENGINE_ADAPTER_IMPORT.search(text)
                if adapter is not None:
                    problems.append(
                        f"{relative}: driver references engine adapter crate `{adapter.group(0)}`"
                    )
    for name in ("lurker-core-contract",):
        if not (root / "crates" / name).is_dir():
            problems.append(f"crates/{name}: required neutral crate is missing")
    return problems


def self_test() -> int:
    assert UI_TOOLKIT.search("use bevy::prelude::*;")
    assert UI_TOOLKIT.search("iced::widget::text")
    assert not UI_TOOLKIT.search("// the UI renders text")
    assert PLATFORM_CODE.search("use std::os::unix::fs::PermissionsExt;")
    assert PLATFORM_CODE.search("#[cfg(target_os = \"android\")]")
    assert not PLATFORM_CODE.search("cfg(test)")
    assert ENGINE_ADAPTER_IMPORT.search("mihomo_api::client::Client")
    assert ENGINE_ADAPTER_IMPORT.search("singbox_config::model::Options")
    assert not ENGINE_ADAPTER_IMPORT.search("mihomo_driver::api::Client")
    print("lurker neutrality guard self-test: PASS")
    return 0


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()
    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1
    print("lurker neutrality guard: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
