#!/usr/bin/env bash
# Local rehearsal entry point for the neutral ("Lurker") crates.io release.
#
# The seven neutral crates are published as one ordered chain:
#
#   lurker-core-contract -> lurker-core-ports -> lurker-engine-runtime
#   -> lurker-profile-store -> lurker-ui-presentation
#   -> mihomo-driver -> singbox-driver
#
# The authoritative implementation lives in
# scripts/quality/neutral-release-guard.py; this wrapper only dispatches to it
# so the same sequence can be executed locally and in CI
# (.github/workflows/release-neutral-crates.yml).
#
# Usage:
#   bash scripts/neutral-crate-release.sh structure   # offline topology audit (default)
#   bash scripts/neutral-crate-release.sh dry-run     # `cargo publish --dry-run` in order
#   bash scripts/neutral-crate-release.sh publish      # real upload, needs CARGO_REGISTRY_TOKEN
#
# Extra arguments are forwarded to the guard, e.g.
#   bash scripts/neutral-crate-release.sh dry-run --summary /tmp/summary.md
#
# Repository secret: the crates.io API token is read from the
# CARGO_REGISTRY_TOKEN repository secret (crates.io token scope
# `publish-scope`). The token is never embedded in this repository; the
# publish mode refuses to run without it.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

mode="${1:-structure}"
if [[ $# -gt 0 ]]; then
  shift
fi

case "$mode" in
  structure | dry-run | publish) ;;
  *)
    echo "usage: bash scripts/neutral-crate-release.sh [structure|dry-run|publish] [extra args]" >&2
    exit 2
    ;;
esac

exec python3 scripts/quality/neutral-release-guard.py --mode "$mode" "$@"
