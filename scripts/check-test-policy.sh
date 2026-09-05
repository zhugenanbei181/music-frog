#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

failed=0

# Keep the repository's commands and documentation on the single supported
# test runner. This checker is excluded because it contains the forbidden
# pattern as the thing it checks for.
forbidden_pattern='(^|[^[:alnum:]_-])cargo[[:space:]]+(\+[^[:space:]]+[[:space:]]+)?test([[:space:]]|$)'
if forbidden_matches="$(git grep -n -I -i -E "$forbidden_pattern" -- \
  . \
  ':(exclude)scripts/check-test-policy.sh' \
  ':(exclude)**/scripts/check-test-policy.sh' \
  ':(exclude).claude/**')"; then
  echo "forbidden raw cargo test command found:" >&2
  echo "$forbidden_matches" >&2
  failed=1
fi

require_text() {
  local path="$1"
  local text="$2"
  if ! grep -Fq -- "$text" "$path"; then
    echo "missing required test policy text '$text' in $path" >&2
    failed=1
  fi
}

require_text ".config/nextest.toml" "test-threads = 4"
require_text "scripts/test.sh" "cargo nextest run"
require_text "scripts/test.sh" "--workspace"
require_text "scripts/test.sh" "--build-jobs 4"
require_text "scripts/test.sh" "--test-threads 4"
require_text ".github/workflows/test.yml" "bash scripts/check-test-policy.sh"
require_text ".github/workflows/test.yml" "bash scripts/test.sh --no-run"
require_text ".github/workflows/test.yml" "bash scripts/test.sh"
require_text ".github/workflows/test.yml" "line-guard.py"
require_text "scripts/test.sh" "parity-guard.py"
require_text "scripts/test.sh" "session-guard.py"
require_text "scripts/test.sh" "hot-reload-guard.py"
require_text "scripts/test.sh" "crash-watchdog-guard.py"
require_text "scripts/test.sh" "core-channel-guard.py"
require_text "scripts/test.sh" "kernel-integrity-guard.py"
require_text "scripts/test.sh" "kernel-rollback-guard.py"
require_text "scripts/test.sh" "controller-auth-guard.py"
require_text "scripts/test.sh" "core-log-level-guard.py"
require_text "scripts/test.sh" "service-mode-guard.py"
require_text "scripts/test.sh" "process-exit-guard.py"
require_text "scripts/test.sh" "port-conflict-guard.py"
require_text "scripts/test.sh" "core-resource-guard.py"
require_text "scripts/test.sh" "offline-startup-guard.py"
require_text ".github/workflows/test.yml" "parity-guard.py"
require_text ".github/workflows/test.yml" "session-guard.py"
require_text ".github/workflows/test.yml" "hot-reload-guard.py"
require_text ".github/workflows/test.yml" "crash-watchdog-guard.py"
require_text ".github/workflows/test.yml" "core-channel-guard.py"
require_text ".github/workflows/test.yml" "kernel-integrity-guard.py"
require_text ".github/workflows/test.yml" "kernel-rollback-guard.py"
require_text ".github/workflows/test.yml" "controller-auth-guard.py"
require_text ".github/workflows/test.yml" "core-log-level-guard.py"
require_text ".github/workflows/test.yml" "service-mode-guard.py"
require_text ".github/workflows/test.yml" "process-exit-guard.py"
require_text ".github/workflows/test.yml" "port-conflict-guard.py"
require_text ".github/workflows/test.yml" "core-resource-guard.py"
require_text ".github/workflows/test.yml" "offline-startup-guard.py"
require_text "TESTING.md" "line-guard.py"

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

echo "test policy OK: cargo nextest, workspace-wide, 4 build jobs, 4 test threads"
