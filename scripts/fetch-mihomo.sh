#!/usr/bin/env bash
# Restore the platform assets pinned by packaging/mihomo-assets.json.
set -euo pipefail
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$script_dir/fetch-mihomo.py" "$@"
