#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [[ $# -gt 1 || ( $# -eq 1 && "$1" != "--no-run" ) ]]; then
  echo "usage: bash scripts/test.sh [--no-run]" >&2
  exit 2
fi

python3 scripts/quality/parity-guard.py --mode enforce
python3 scripts/quality/session-guard.py --mode enforce
python3 scripts/quality/hot-reload-guard.py --mode enforce
python3 scripts/quality/crash-watchdog-guard.py --mode enforce
python3 scripts/quality/core-channel-guard.py --mode enforce
python3 scripts/quality/kernel-integrity-guard.py --mode enforce
python3 scripts/quality/kernel-rollback-guard.py --mode enforce
python3 scripts/quality/controller-auth-guard.py --mode enforce
python3 scripts/quality/core-log-level-guard.py --mode enforce
python3 scripts/quality/service-mode-guard.py --mode enforce
python3 scripts/quality/process-exit-guard.py --mode enforce
python3 scripts/quality/port-conflict-guard.py --mode enforce
python3 scripts/quality/core-resource-guard.py --mode enforce
python3 scripts/quality/offline-startup-guard.py --mode enforce
python3 scripts/quality/lifecycle-sync-guard.py --mode enforce
python3 scripts/quality/lifecycle-matrix-guard.py --mode enforce
python3 scripts/quality/tun-stack-guard.py --mode enforce
python3 scripts/quality/mtu-negotiation-guard.py --mode enforce
python3 scripts/quality/tun-routing-guard.py --mode enforce

nextest_mode=()
if [[ $# -eq 1 ]]; then
  nextest_mode+=("--no-run")
fi

# Keep both dimensions explicit:
# - --build-jobs controls Cargo compilation parallelism.
# - --test-threads (nextest's -j alias) controls concurrent test processes.
exec cargo nextest run \
  --workspace \
  --build-jobs 4 \
  --test-threads 4 \
  "${nextest_mode[@]}"
