#!/usr/bin/env bash
set -euo pipefail

if [[ $# -gt 1 || ( $# -eq 1 && "$1" != "--no-run" ) ]]; then
  echo "usage: bash scripts/test.sh [--no-run]" >&2
  exit 2
fi

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
