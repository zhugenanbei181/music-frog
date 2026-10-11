#!/usr/bin/env bash
# Convert a registered square PNG launcher asset into a macOS ICNS bundle.
set -euo pipefail
if [[ $# -ne 2 ]]; then
  echo "usage: $0 SOURCE.png OUTPUT.icns" >&2
  exit 2
fi
source_icon="$1"
output_icon="$2"
test -s "$source_icon"
command -v sips >/dev/null
command -v iconutil >/dev/null
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/musicfrog-icon.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT
iconset="$work_dir/MusicFrog.iconset"
mkdir -p "$iconset" "$(dirname "$output_icon")"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$source_icon" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  retina_size=$((size * 2))
  sips -z "$retina_size" "$retina_size" "$source_icon" \
    --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$output_icon"
test -s "$output_icon"
