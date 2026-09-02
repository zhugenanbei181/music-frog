#!/usr/bin/env bash
# Stable entry point for the Infiltrator Bevy demo screenshot workflow.
# The implementation lives in capture-bevy-matrix.sh (one build, one nested
# niri, sequential scenarios, PID-bound niri-IPC screenshots).
#
# Usage:
#   bash scripts/capture-bevy.sh                    # full matrix (all scenarios)
#   bash scripts/capture-bevy.sh overview-dark      # one scenario
#   bash scripts/capture-bevy.sh overview-dark,overview-light   # a subset
#   INFILTRATOR_CAPTURE_SCENARIOS=overview-dark bash scripts/capture-bevy.sh
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

exec bash "$REPO/scripts/capture-bevy-matrix.sh"
