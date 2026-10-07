#!/usr/bin/env bash
# setsid runs after the parent receives $!: wait for the private group instead
# of accepting the inherited group from that short startup race.
process_group() {
  local pid="$1" pgid
  for _ in $(seq 1 50); do
    pgid="$(ps -o pgid= -p "$pid" 2>/dev/null | tr -d '[:space:]')"
    if [ "$pgid" = "$pid" ]; then
      printf '%s' "$pgid"
      return 0
    fi
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.02
  done
  # An empty receipt keeps the caller fail-closed. Never return another group.
  return 0
}
