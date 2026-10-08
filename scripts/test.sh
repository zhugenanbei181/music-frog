#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
if [[ $# -gt 1 || ( $# -eq 1 && "$1" != "--no-run" && "$1" != "--guards-only" ) ]]; then
  echo "usage: bash scripts/test.sh [--no-run|--guards-only]" >&2
  exit 2
fi
bash scripts/quality/check-structure.sh
if [[ "${1:-}" == "--guards-only" ]]; then exit 0; fi
# Compile the Rust authority and resolve all declared anchors against real discovery.
python3 scripts/parity/resolve_surface_evidence.py --report-json target/parity/evidence.json
nextest_mode=()
if [[ "${1:-}" == "--no-run" ]]; then nextest_mode+=("--no-run"); fi
exec cargo nextest run --workspace --build-jobs 4 --test-threads 4 "${nextest_mode[@]}"
