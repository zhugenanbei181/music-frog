#!/usr/bin/env bash
# Preserve each completed private capture attempt before its files are reused.
archive_capture_attempt() {
  local scenario_dir="$1" attempt="$2" status="$3" pid="$4" window_id="$5"
  local destination="$scenario_dir/attempt-$attempt" artifact
  mkdir "$destination" || return
  for artifact in app.log marker.log windows.json windows.err action.log image.png diagnostic.png rendered-frame.png; do
    if [[ -f "$scenario_dir/$artifact" ]]; then
      cp -- "$scenario_dir/$artifact" "$destination/$artifact" || return
    fi
  done
  printf 'attempt\tstatus\tapp_pid\twindow_id\n%s\t%s\t%s\t%s\n' \
    "$attempt" "$status" "$pid" "$window_id" >"$destination/receipt.tsv"
}

# Wait until the compositor has finished writing a screenshot, then report it.
#
# `niri msg action screenshot-window --write-to-disk true` returns before the
# PNG is fully flushed, so a harness that accepts the first non-empty read can
# hash a half-written file. A measured instance: a 75920-byte frame was hashed
# at 65536 bytes - exactly 64 KiB, one pipe buffer - and published the complete
# file, so the manifest's digest and byte count described a pixel that no longer
# existed. That breaks the one promise a receipt makes: the index must be the
# bytes it indexes.
#
# The settle rule lives here because all three matrices share this harness; a
# per-surface copy would drift and leave one of them still racing. Size must
# stop changing for two consecutive polls before it is read once, and the value
# returned is the same one the digest is computed from.
#
# Echoes "<bytes> <sha256>" on success, or returns 1 when the file never
# appeared, never settled, or stayed too small to be a real frame.
capture_settled_image() {
  local image="$1" minimum="${2:-5000}"
  local previous=-1 current=0 stable=0 attempt
  for attempt in $(seq 1 40); do
    if [[ -f "$image" ]]; then
      current="$(stat -c%s "$image" 2>/dev/null || printf 0)"
    else
      current=0
    fi
    if [[ "$current" == "$previous" && "$current" -gt "$minimum" ]]; then
      stable=$((stable + 1))
      if [[ "$stable" -ge 2 ]]; then
        printf '%s %s\n' "$current" "$(sha256sum "$image" | cut -d' ' -f1)"
        return 0
      fi
    else
      stable=0
    fi
    previous="$current"
    sleep 0.1
  done
  return 1
}
