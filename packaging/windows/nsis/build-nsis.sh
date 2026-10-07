#!/usr/bin/env bash
# Compile either peer's installer with explicit native absolute paths.
set -euo pipefail
if [[ $# -ne 4 ]]; then
  echo "usage: $0 iced|bevy VERSION BINARY OUTPUT" >&2
  exit 2
fi
surface="$1"
version="$2"
binary="$3"
output="$4"
case "$surface" in
  iced) installer=infiltrator.nsi ;;
  bevy) installer=infiltrator-bevy.nsi ;;
  *) echo "unknown peer product: $surface" >&2; exit 2 ;;
esac
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"
test -s "$binary"
icon=crates/infiltrator-iced/icons/icon.ico
kernel=vendor/mihomo.exe
bash scripts/fetch-mihomo.sh --target x86_64-pc-windows-msvc
kernel_license=packaging/Mihomo-LICENSE
kernel_metadata=packaging/mihomo-assets.json
test -s "$icon"
test -s "$kernel"
mkdir -p -- "$(dirname -- "$output")"
command -v makensis >/dev/null
# Git Bash's POSIX/native path conversion must not rewrite NSIS -D values.
# cygpath normalizes *all* separators, including GitHub's mixed workspace path.
if command -v cygpath >/dev/null; then
  binary_native="$(cygpath -aw "$binary")"
  output_native="$(cygpath -aw "$output")"
  icon_native="$(cygpath -aw "$icon")"
  kernel_native="$(cygpath -aw "$kernel")"
  license_native="$(cygpath -aw "$kernel_license")"
  metadata_native="$(cygpath -aw "$kernel_metadata")"
  script_native="$(cygpath -aw "packaging/windows/nsis/$installer")"
else
  binary_native="$(realpath "$binary")"
  output_native="$(realpath -m "$output")"
  icon_native="$(realpath "$icon")"
  kernel_native="$(realpath "$kernel")"
  license_native="$(realpath "$kernel_license")"
  metadata_native="$(realpath "$kernel_metadata")"
  script_native="$repo_root/packaging/windows/nsis/$installer"
fi
MSYS2_ARG_CONV_EXCL='*' makensis -NOCD \
  "-DVERSION=$version" "-DARCH=x64" \
  "-DOUTFILE=$output_native" "-DICON_PATH=$icon_native" \
  "-DBINARY_PATH=$binary_native" "-DKERNEL_PATH=$kernel_native" \
  "-DKERNEL_LICENSE_PATH=$license_native" "-DKERNEL_METADATA_PATH=$metadata_native" "$script_native"
test -s "$output"
