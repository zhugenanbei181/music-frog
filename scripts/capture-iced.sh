#!/usr/bin/env bash
# Stable entry point for the Infiltrator iced demo screenshot workflow.
# The implementation lives in capture-iced-matrix.sh (one build, one nested
# niri, sequential scenarios, PID-bound niri-IPC screenshots).
#
# Usage:
#   bash scripts/capture-iced.sh                    # full matrix (all scenarios)
#   bash scripts/capture-iced.sh proxies-dark       # one scenario
#   bash scripts/capture-iced.sh overview-dark,sync-light   # a subset
#   INFILTRATOR_CAPTURE_SCENARIOS=proxies-dark bash scripts/capture-iced.sh
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"

if [ "$#" -gt 1 ]; then
  printf 'usage: %s [scenario[,scenario...]]\n' "$0" >&2
  exit 2
fi
if [ "$#" -eq 1 ]; then
  # An explicit argument wins over a pre-set INFILTRATOR_CAPTURE_SCENARIOS.
  export INFILTRATOR_CAPTURE_SCENARIOS="$1"
fi

exec bash "$REPO/scripts/capture-iced-matrix.sh"
