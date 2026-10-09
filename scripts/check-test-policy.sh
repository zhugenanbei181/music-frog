#!/usr/bin/env bash
# Test-policy guard.
#
# Structural and pattern-based: it parses configuration and matches command
# *shapes*, never literal document snapshots. The single authoritative list of
# quality guards is scripts/test.sh / scripts/test-bevy.sh themselves; this
# checker only verifies they keep the required shape.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

failed=0
note() { echo "test-policy: $*" >&2; failed=1; }
require() { grep -Eq -- "$2" "$1" || note "$1 missing required pattern: $2"; }

# A. No raw `cargo test` anywhere (the only supported runner is nextest).
if git grep -n -I -i -E \
  '(^|[^[:alnum:]_-])cargo[[:space:]]+(\+[^[:space:]]+[[:space:]]+)?test([[:space:]]|$)' -- \
  . \
  ':(exclude)scripts/check-test-policy.sh' \
  ':(exclude)**/scripts/check-test-policy.sh' \
  ':(exclude).claude/**'; then
  note "forbidden raw cargo test command found"
fi

# B. nextest must pin the thread count. Parse the TOML instead of text-matching.
if ! python3 - <<'PY'
import sys, tomllib
cfg = tomllib.load(open(".config/nextest.toml", "rb"))
threads = cfg.get("profile", {}).get("default", {}).get("test-threads")
if threads != 4:
    print(f"nextest.toml must pin [profile.default] test-threads = 4 (found {threads!r})", file=sys.stderr)
    sys.exit(1)
PY
then
  note "nextest.toml thread pin missing"
fi

# C/D. Both runners use nextest, the shared structural gate and the dynamic
# evidence resolver, and support the cheap `--guards-only` fast path.
for runner in scripts/test.sh scripts/test-bevy.sh; do
  require "$runner" 'cargo[[:space:]]+nextest[[:space:]]+run'
  require "$runner" 'check-structure\.sh'
  require "$runner" 'resolve_surface_evidence\.py'
  require "$runner" '--guards-only'
done
require scripts/test.sh '--workspace'
require scripts/test.sh '--build-jobs[[:space:]]+4'
require scripts/test.sh '--test-threads[[:space:]]+4'
require scripts/test-bevy.sh '--build-jobs[[:space:]]+4'
require scripts/test-bevy.sh '--test-threads[[:space:]]+4'

# E/F. CI workflows invoke the entry points (shape, not an inlined guard list).
require .github/workflows/test.yml 'check-test-policy\.sh'
require .github/workflows/test.yml 'test\.sh[[:space:]]+--guards-only'
require .github/workflows/test.yml 'test\.sh[[:space:]]+--no-run'
require .github/workflows/bevy.yml 'test-bevy\.sh[[:space:]]+--guards-only'

# G. Retired source-evidence guards must not be re-registered under any runner.
while IFS=$'\t' read -r script replacement; do
  [[ "$script" == script ]] && continue
  for runner in scripts/test.sh scripts/test-bevy.sh scripts/quality/check-structure.sh; do
    if grep -Fq -- "$script" "$runner"; then
      note "retired source-evidence guard registered in $runner: $script"
    fi
  done
done < scripts/parity/retired_source_guards.tsv

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

echo "test policy OK: nextest, workspace-wide, pinned concurrency, single structural gate"
