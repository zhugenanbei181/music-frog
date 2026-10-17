#!/usr/bin/env python3
"""Keep the Lurker neutral crates engine-, platform- and UI-neutral.

The product architecture is fixed: two backends (mihomo / sing-box) × every
platform (Windows/Linux/macOS/Android/iOS) × every UI (Iced/Bevy/Slint/Compose).
A neutral framework crate may describe the domain, but it must never depend on a
product crate or a UI toolkit — and the pure layers must contain no OS-specific
code either. Portable kernels may own per-OS code, but only behind an explicit
extension point, and still may not pull in the product, an engine adapter or a
UI toolkit. Driver crates (`mihomo-driver`, `singbox-driver`) may own per-OS
process code, but still may not pull in the product or a UI toolkit.
"""

from __future__ import annotations

import re
import sys
import tempfile
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]

# Pure logic layers: no product crate, no UI toolkit, no OS-specific code.
PURE_NEUTRAL = (
    "lurker-core-contract",
    "lurker-core-ports",
    "lurker-ui-presentation",
    "lurker-engine-runtime",
)
# Portable kernels: may own per-OS code behind a platform extension point
# (`lurker_profile_store::platform::StorePlatform`), but stay product-, engine-
# adapter- and UI-neutral.
PORTABLE_KERNEL = ("lurker-profile-store",)
# Engine drivers: may own per-OS process code, but stay product/UI neutral.
DRIVERS = ("mihomo-driver", "singbox-driver")
ALL_NEUTRAL = PURE_NEUTRAL + PORTABLE_KERNEL + DRIVERS

UI_TOOLKIT = re.compile(
    r"\b(?:use\s+(?:bevy|iced|slint|winit|egui|zhuge_ui)\b"
    r"|(?:bevy|iced|slint|winit|egui|zhuge_ui)::)"
)
# `cfg(unix)`, `cfg(windows)`, `cfg(target_os = ...)`, `cfg(any(unix, ...))`,
# `cfg(not(windows))` and the raw FFI crates. Pure-neutral crates must never
# grow OS code again.
PLATFORM_CODE = re.compile(
    r"(?:std::os::"
    r"|libc::"
    r"|windows_sys::"
    r"|keyring::"
    r"|dirs::"
    r"|cfg\(\s*(?:[a-z_]+\(\s*)*(?:windows|unix|target_os|target_family)\b)"
)
UI_DEPENDENCIES = {"bevy", "iced", "slint", "winit", "egui", "zhuge-ui"}

# Engine adapter crates are product-side. A driver owns the engine logic itself,
# so depending back on an adapter crate would create a second implementation.
ENGINE_ADAPTER_PREFIXES = ("mihomo-", "singbox-")
ENGINE_ADAPTER_IMPORT = re.compile(
    r"\b(?:mihomo|singbox)_(?:api|config|process|version)::"
)

# Drivers speak plaintext http/ws to a loopback controller only. Linking a TLS
# backend would silently widen the transport contract and drag OpenSSL into
# every target build, so it is rejected outright.
TLS_DEPENDENCIES = {
    "native-tls",
    "openssl",
    "openssl-sys",
    "rustls",
    "rustls-native-certs",
    "schannel",
    "security-framework",
}


def effective_features(
    dependency: str, spec: dict, workspace_dependencies: dict
) -> set[str]:
    """Features merged from the workspace spec and the crate-level override."""
    features: set[str] = set()
    if spec.get("workspace") is True:
        workspace_spec = workspace_dependencies.get(dependency, {})
        if isinstance(workspace_spec, dict):
            features.update(workspace_spec.get("features", []))
    features.update(spec.get("features", []))
    return features


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
        workspace_dependencies = tomllib.loads(
            (root / "Cargo.toml").read_text(encoding="utf-8")
        ).get("workspace", {}).get("dependencies", {})
        for dependency, spec in dependencies.items():
            if dependency.startswith("infiltrator-"):
                problems.append(f"{name}: must not depend on product crate `{dependency}`")
            if dependency in UI_DEPENDENCIES:
                problems.append(f"{name}: must not depend on UI toolkit `{dependency}`")
            if name in DRIVERS and dependency.startswith(ENGINE_ADAPTER_PREFIXES):
                problems.append(
                    f"{name}: driver must not depend on engine adapter crate `{dependency}`"
                )
            if name in PORTABLE_KERNEL and dependency.startswith(ENGINE_ADAPTER_PREFIXES):
                problems.append(
                    f"{name}: portable kernel must not depend on engine adapter crate `{dependency}`"
                )
            if not isinstance(spec, dict):
                continue
            if name in DRIVERS:
                if dependency in TLS_DEPENDENCIES:
                    problems.append(
                        f"{name}: driver must not link a TLS backend (`{dependency}`)"
                    )
                features = effective_features(dependency, spec, workspace_dependencies)
                tls_features = sorted(f for f in features if "tls" in f.lower())
                if tls_features:
                    problems.append(
                        f"{name}: dependency `{dependency}` enables TLS feature(s) {tls_features}"
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


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _synthetic_check_self_test() -> None:
    """Exercise `check()` itself on a throw-away tree."""
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        _write(root / "Cargo.toml", "[workspace]\n[workspace.dependencies]\n")
        _write(
            root / "crates/lurker-core-contract/Cargo.toml",
            '[package]\nname = "lurker-core-contract"\nversion = "0.0.0"\n',
        )
        _write(
            root / "crates/lurker-core-contract/src/lib.rs",
            '#[cfg(unix)]\nfn native() {}\n',
        )
        _write(
            root / "crates/lurker-profile-store/Cargo.toml",
            '[package]\nname = "lurker-profile-store"\nversion = "0.0.0"\n'
            "[dependencies]\nlibc = \"0.2\"\n",
        )
        _write(
            root / "crates/lurker-profile-store/src/platform.rs",
            "pub fn native() -> i32 { unsafe { libc::getpid() } }\n",
        )
        problems = check(root)
        assert any(
            "lurker-core-contract" in problem and "platform code" in problem
            for problem in problems
        ), problems
        assert not [
            problem for problem in problems if "lurker-profile-store" in problem
        ], problems

        _write(
            root / "crates/lurker-profile-store/Cargo.toml",
            '[package]\nname = "lurker-profile-store"\nversion = "0.0.0"\n'
            "[dependencies]\ninfiltrator-ports = { path = \"../infiltrator-ports\" }\n",
        )
        assert any(
            "portable kernel" not in problem
            and "must not depend on product crate `infiltrator-ports`" in problem
            for problem in check(root)
        ), check(root)

        _write(
            root / "crates/lurker-profile-store/Cargo.toml",
            '[package]\nname = "lurker-profile-store"\nversion = "0.0.0"\n'
            "[dependencies]\nmihomo-driver = \"0\"\n",
        )
        assert any(
            "portable kernel must not depend on engine adapter crate `mihomo-driver`"
            in problem
            for problem in check(root)
        ), check(root)

        _write(
            root / "crates/lurker-profile-store/Cargo.toml",
            '[package]\nname = "lurker-profile-store"\nversion = "0.0.0"\n'
            "[dependencies]\niced = \"0.14\"\n",
        )
        assert any(
            "must not depend on UI toolkit `iced`" in problem for problem in check(root)
        ), check(root)


def self_test() -> int:
    assert UI_TOOLKIT.search("use bevy::prelude::*;")
    assert UI_TOOLKIT.search("iced::widget::text")
    assert not UI_TOOLKIT.search("// the UI renders text")
    assert PLATFORM_CODE.search("use std::os::unix::fs::PermissionsExt;")
    assert PLATFORM_CODE.search('#[cfg(target_os = "android")]')
    assert PLATFORM_CODE.search('#[cfg(target_family = "wasm")]')
    assert PLATFORM_CODE.search("#[cfg(windows)]")
    assert PLATFORM_CODE.search("#[cfg(unix)]")
    assert PLATFORM_CODE.search("#[cfg(any(unix, windows))]")
    assert PLATFORM_CODE.search("#[cfg(not(windows))]")
    assert PLATFORM_CODE.search("#[cfg(all(unix, target_os = \"linux\"))]")
    assert PLATFORM_CODE.search("use libc::flock;")
    assert PLATFORM_CODE.search("libc::close(descriptor);")
    assert PLATFORM_CODE.search("windows_sys::Win32::Foundation::CloseHandle(handle);")
    assert not PLATFORM_CODE.search("cfg(test)")
    assert not PLATFORM_CODE.search('#[cfg(feature = "serde")]')
    assert not PLATFORM_CODE.search("#[cfg(target_pointer_width = \"64\")]")
    assert ENGINE_ADAPTER_IMPORT.search("mihomo_api::client::Client")
    assert ENGINE_ADAPTER_IMPORT.search("singbox_config::model::Options")
    assert not ENGINE_ADAPTER_IMPORT.search("mihomo_driver::api::Client")
    workspace = {"tokio-tungstenite": {"version": "0.30", "features": ["native-tls"]}}
    merged = effective_features("tokio-tungstenite", {"workspace": True, "features": ["connect"]}, workspace)
    assert "native-tls" in merged and "connect" in merged
    assert any("tls" in feature for feature in merged)
    _synthetic_check_self_test()
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
