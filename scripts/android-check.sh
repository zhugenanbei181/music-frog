#!/usr/bin/env bash
# BANDROID-018: L0/L1 Android gate — compile every Android-only surface for both
# shipped ABIs (aarch64 + x86_64) and lint the canonical aarch64 ABI.
#
# L0 = compiles (cargo check), L1 = lint-clean (clippy -D warnings). This is a
# *host-side* gate only: it does NOT build an APK, does NOT launch an emulator
# and does NOT replace real-device / emulator evidence (aapt badging, crash-free
# launch, screenshots). Those remain owned by the packaging + device matrices.
#
# Wired into CI by .github/workflows/android.yml and runnable locally from
# anywhere: ./scripts/android-check.sh
#
# Requires: Rust (rustup) with the aarch64-linux-android and x86_64-linux-android
# targets, plus the NDK for anything that links (the UI crates build script).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "[android-check] repo root: $REPO_ROOT"

# ---------------------------------------------------------------------------
# L0: compile the Android cdylib and the Bevy UI crates for both ABIs.
# The Bevy crates are checked --no-default-features because the Android
# (mobile) surface is compiled without the desktop-only default features.
# ---------------------------------------------------------------------------
for target in aarch64-linux-android x86_64-linux-android; do
  echo "[android-check] cargo check -p infiltrator-android --target $target"
  cargo check -p infiltrator-android --target "$target"

  echo "[android-check] cargo check -p infiltrator-bevy-ui -p infiltrator-bevy-widgets --no-default-features --target $target"
  cargo check -p infiltrator-bevy-ui -p infiltrator-bevy-widgets \
    --no-default-features --target "$target"
done

# ---------------------------------------------------------------------------
# L1: lint the canonical aarch64 ABI with warnings denied.
# ---------------------------------------------------------------------------
echo "[android-check] cargo clippy -p infiltrator-android --target aarch64-linux-android -- -D warnings"
cargo clippy -p infiltrator-android --target aarch64-linux-android -- -D warnings

echo "[android-check] cargo clippy -p infiltrator-bevy-ui -p infiltrator-bevy-widgets --no-default-features --target aarch64-linux-android -- -D warnings"
cargo clippy -p infiltrator-bevy-ui -p infiltrator-bevy-widgets \
  --no-default-features --target aarch64-linux-android -- -D warnings

echo "[android-check] OK"
