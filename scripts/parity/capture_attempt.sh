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
