#!/usr/bin/env bash
# Verification runner for all cross-platform packaging templates, scripts, and release manifests.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

echo "=== Verifying Cross-Platform Packaging Templates & Release Tools ==="
python3 scripts/quality/verify-packaging.py "$@"
