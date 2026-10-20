#!/usr/bin/env bash
# Drive one Android flavor on the disposable emulator and write its receipts.
#
# The flow never selects a physical device and never touches the network: it
# drives the emulator through adb, keeps the guest in airplane mode and proves
# every interaction with the screenshot, the logcat window, the accessibility
# assertion and the owner state readback.
#
# Usage:
#   bash scripts/android/android-interaction-capture.sh --flavor compose
#   bash scripts/android/android-interaction-capture.sh --flavor bevy --serial emulator-5554
#   bash scripts/android/android-interaction-capture.sh --merge .evidence/android/captures/<stamp>
#   bash scripts/android/android-interaction-capture.sh --probe
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
exec python3 "$REPO_ROOT/scripts/android/android_interaction_capture.py" "$@"
