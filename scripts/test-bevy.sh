#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
export INFILTRATOR_LANG="${INFILTRATOR_LANG:-zh-CN}"
if [[ $# -gt 1 || ( $# -eq 1 && "$1" != "--guards-only" ) ]]; then
  echo "usage: bash scripts/test-bevy.sh [--guards-only]" >&2
  exit 2
fi
bash scripts/quality/check-structure.sh
if [[ "${1:-}" == "--guards-only" ]]; then exit 0; fi
# Peer evidence remains a two-surface check even when only Bevy tests are run.
python3 scripts/parity/resolve_surface_evidence.py --report-json target/parity/evidence.json
for crate_name in infiltrator-bevy-widgets infiltrator-bevy-ui; do
  cargo nextest run -p "$crate_name" --build-jobs 4 --test-threads 4
  cargo clippy -p "$crate_name" --all-targets -- -D warnings
  cargo fmt -p "$crate_name" --check
done
