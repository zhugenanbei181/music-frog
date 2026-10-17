#!/usr/bin/env bash
# Reclaim runner disk before a heavy build.
#
# GitHub-hosted runners ship a ~14 GB root volume. A job that restores the Rust
# cache and then compiles the iced + bevy + slint workspace (or an Android
# Gradle/NDK build) cannot fit, and the failure mode is brutal: the runner
# cannot even write its own `_diag` log, so the job dies without a conclusion.
#
# Delete the image's preinstalled toolchains that this repository never uses in
# CI. Pass --keep-android for jobs that build Android; the SDK/NDK live under
# /usr/local/lib/android and `android-actions/setup-android` reuses them.
set -euo pipefail

keep_android=false
if [[ "${1:-}" == "--keep-android" ]]; then
  keep_android=true
elif [[ $# -gt 0 ]]; then
  echo "usage: bash scripts/ci-free-disk.sh [--keep-android]" >&2
  exit 2
fi

targets=(
  /usr/share/dotnet
  /opt/ghc
  /opt/hostedtoolcache/CodeQL
  /usr/local/share/boost
  /usr/local/.ghcup
  /usr/share/swift
)
if [[ "$keep_android" == false ]]; then
  targets+=(/usr/local/lib/android)
fi

sudo rm -rf "${targets[@]}"
sudo apt-get clean
df -h /
