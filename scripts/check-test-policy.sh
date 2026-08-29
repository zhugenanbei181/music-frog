#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

failed=0

# Keep the repository's commands and documentation on the single supported
# test runner. This checker is excluded because it contains the forbidden
# pattern as the thing it checks for.
forbidden_pattern='(^|[^[:alnum:]_-])cargo[[:space:]]+(\+[^[:space:]]+[[:space:]]+)?test([[:space:]]|$)'
if forbidden_matches="$(rg -n -I -i --hidden \
  --glob '!.git/**' \
  --glob '!**/target/**' \
  --glob '!**/node_modules/**' \
  --glob '!**/dist/**' \
  --glob '!**/coverage/**' \
  --glob '!vendor/**' \
  --glob '!scripts/check-test-policy.sh' \
  -e "$forbidden_pattern" .)"; then
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

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

echo "test policy OK: cargo nextest, workspace-wide, 4 build jobs, 4 test threads"
