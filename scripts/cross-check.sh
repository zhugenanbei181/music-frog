#!/usr/bin/env bash
# scripts/cross-check.sh
# GAP-1.1: Product-level x86_64 / aarch64 cross-check script.
#
# Validates that product-level crates:
#   - infiltrator-core
#   - infiltrator-composition
#   - infiltrator-desktop
#   - infiltrator-iced
#   - infiltrator-bevy-desktop-host
#   - infiltrator-slint
# compile cleanly for the canonical aarch64 desktop targets:
#   1. aarch64-unknown-linux-gnu (via zig + cargo-zigbuild)
#   2. aarch64-pc-windows-msvc   (via clang + cargo-xwin)
#
# Android aarch64 (aarch64-linux-android) is validated via scripts/android-check.sh.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

echo "[cross-check] repo root: $repo_root"

# 1. Toolchain presence checks
for cmd in zig cargo-zigbuild cargo-xwin clang; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[cross-check] ERROR: required tool '$cmd' not found in PATH" >&2
    exit 1
  fi
done
echo "[cross-check] OK: zig, cargo-zigbuild, cargo-xwin, clang present"

# 2. Rustup installed target checks
installed_targets="$(rustup target list --installed)"
for target in aarch64-unknown-linux-gnu aarch64-pc-windows-msvc; do
  if ! echo "$installed_targets" | grep -q "^$target$"; then
    echo "[cross-check] installing missing rustup target: $target"
    rustup target add "$target"
  fi
done
echo "[cross-check] OK: rustup aarch64 cross targets verified"

TARGET_CRATES=(
  -p infiltrator-core
  -p infiltrator-composition
  -p infiltrator-desktop
  -p infiltrator-iced
  -p infiltrator-bevy-desktop-host
  -p infiltrator-slint
)

# 3. aarch64-unknown-linux-gnu check (Linux desktop arm64)
echo "[cross-check] 1/2: cargo-zigbuild check --target aarch64-unknown-linux-gnu"
PKG_CONFIG_ALLOW_CROSS=1 cargo-zigbuild check \
  "${TARGET_CRATES[@]}" \
  --target aarch64-unknown-linux-gnu

# 4. aarch64-pc-windows-msvc check (Windows desktop arm64)
echo "[cross-check] 2/2: cargo-xwin check --target aarch64-pc-windows-msvc"
CARGO_FEATURE_NO_NEON=1 cargo-xwin check --cross-compiler clang \
  "${TARGET_CRATES[@]}" \
  --target aarch64-pc-windows-msvc

echo "[cross-check] OK: all desktop aarch64 product cross-target checks passed"
