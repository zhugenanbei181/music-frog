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

# Both entry points use the same structural checks and dynamic evidence resolver.
require_text "scripts/test.sh" "check-structure.sh"
require_text "scripts/test-bevy.sh" "check-structure.sh"
require_text "scripts/test.sh" "resolve_surface_evidence.py"
require_text "scripts/test-bevy.sh" "resolve_surface_evidence.py"
require_text "scripts/test.sh" "--guards-only"
require_text "scripts/test-bevy.sh" "--guards-only"
require_text ".github/workflows/test.yml" "bash scripts/check-test-policy.sh"
require_text ".github/workflows/test.yml" "bash scripts/test.sh --guards-only"
require_text ".github/workflows/test.yml" "bash scripts/test.sh --no-run"
require_text ".github/workflows/test.yml" "bash scripts/test.sh"
require_text ".github/workflows/bevy.yml" "bash scripts/test-bevy.sh --guards-only"
require_text ".github/workflows/bevy.yml" "bash scripts/test-bevy.sh"
require_text "scripts/test-bevy.sh" "cargo nextest run"
require_text "TESTING.md" "line-guard.py"
# Prevent re-registering retired text guards under either runner or the shared gate.
while IFS=$'\t' read -r script replacement; do
  [[ "$script" == script ]] && continue
  for runner in scripts/test.sh scripts/test-bevy.sh scripts/quality/check-structure.sh; do
    if grep -Fq -- "$script" "$runner"; then
      echo "retired source-evidence guard registered in $runner: $script" >&2
      failed=1
    fi
  done
done < scripts/parity/retired_source_guards.tsv

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

echo "test policy OK: cargo nextest, workspace-wide, 4 build jobs, 4 test threads"
