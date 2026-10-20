#!/usr/bin/env bash
# Shared helpers for the environment-blocked board procedures.
#
# Sourced by every script in scripts/blocked/. Never executed directly.
#
# Result vocabulary (one line per step, always in the same shape):
#
#   [OK]      the step ran on this machine and passed
#   [SKIP]    the step is not applicable in the current mode
#   [PENDING] the step needs the missing environment; it is listed as "remains"
#   [GAP]     a real repository gap that another owner must land first; it is
#             not a script defect and not environment evidence
#   [FAIL]    a real defect; the script exits non-zero
#
# Exit codes used by every script in this directory:
#
#   0  every local step passed (environment legs may still be pending)
#   1  a real defect was found
#   2  the requested (non-dry-run) mode could not start because its
#      environment is absent; nothing was faked
#
# The scripts are idempotent: they only read the repository, write their
# evidence under target/blocked-evidence/ and never mutate tracked files.

BLOCKED_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$BLOCKED_DIR/../.." && pwd)"
export REPO_ROOT

BLOCKED_SCRIPT="${BLOCKED_SCRIPT:-blocked}"
BLOCKED_DRY_RUN=0
BLOCKED_STRICT=0
BLOCKED_FAILURES=0
BLOCKED_GAPS=0
BLOCKED_OK=0
BLOCKED_SKIPS=0
BLOCKED_PENDING=0
BLOCKED_STEP=0
BLOCKED_EVIDENCE_DIR=""
BLOCKED_RESULTS=()

blocked_step() {
    BLOCKED_STEP=$((BLOCKED_STEP + 1))
    printf '\n== %s :: step %d :: %s ==\n' "$BLOCKED_SCRIPT" "$BLOCKED_STEP" "$1"
}

blocked_ok() {
    printf '  [OK]      %s\n' "$*"
    BLOCKED_OK=$((BLOCKED_OK + 1))
    BLOCKED_RESULTS+=("OK|$*")
}

blocked_skip() {
    printf '  [SKIP]    %s\n' "$*"
    BLOCKED_SKIPS=$((BLOCKED_SKIPS + 1))
    BLOCKED_RESULTS+=("SKIP|$*")
}

blocked_pending() {
    printf '  [PENDING] %s\n' "$*"
    BLOCKED_PENDING=$((BLOCKED_PENDING + 1))
    BLOCKED_RESULTS+=("PENDING|$*")
}

blocked_gap() {
    printf '  [GAP]     %s\n' "$*"
    BLOCKED_GAPS=$((BLOCKED_GAPS + 1))
    BLOCKED_RESULTS+=("GAP|$*")
}

blocked_fail() {
    printf '  [FAIL]    %s\n' "$*" >&2
    BLOCKED_FAILURES=$((BLOCKED_FAILURES + 1))
    BLOCKED_RESULTS+=("FAIL|$*")
}

# blocked_run <label> <command...>
# Runs the command, stores its output under the evidence directory and records
# OK/FAIL. The caller decides what to do with a failure; the helper never
# aborts the script by itself.
blocked_run() {
    local label="$1"
    shift
    local slug
    slug="$(printf '%s' "$label" | tr -c 'A-Za-z0-9._-' '-' | cut -c1-80)"
    local log="$BLOCKED_EVIDENCE_DIR/logs/$slug.log"
    mkdir -p "$(dirname "$log")"
    if "$@" >"$log" 2>&1; then
        blocked_ok "$label"
        return 0
    fi
    blocked_fail "$label (log: ${log#"$REPO_ROOT"/})"
    tail -n 20 "$log" >&2 || true
    return 1
}

# blocked_evidence_dir <kind> — a stable, per-script directory so a re-run
# overwrites its own evidence instead of piling up new directories.
blocked_evidence_dir() {
    local kind="$1"
    local base="${BLOCKED_EVIDENCE_ROOT:-$REPO_ROOT/target/blocked-evidence}"
    BLOCKED_EVIDENCE_DIR="$base/$kind"
    mkdir -p "$BLOCKED_EVIDENCE_DIR"
    printf '%s\n' "$BLOCKED_EVIDENCE_DIR"
}

blocked_identity() {
    python3 "$REPO_ROOT/scripts/product_identity.py" --get "$1"
}

# Resolve the Android SDK the same way the repository does: the environment
# first, then android/local.properties.
blocked_android_sdk_root() {
    local candidate
    for candidate in "${ANDROID_HOME:-}" "${ANDROID_SDK_ROOT:-}"; do
        if [ -n "$candidate" ] && [ -d "$candidate" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    if [ -f "$REPO_ROOT/android/local.properties" ]; then
        candidate="$(grep -E '^sdk\.dir=' "$REPO_ROOT/android/local.properties" | cut -d= -f2- | tr -d '\r' || true)"
        if [ -n "$candidate" ] && [ -d "$candidate" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    fi
    return 1
}

# Locate a JDK that ships `keytool` (used by the keystore procedure).
blocked_java_bin() {
    local candidate
    for candidate in "${JAVA_HOME:-}/bin" "$HOME/android-tools-bevy/jdk/bin" "/usr/lib/jvm/default-java/bin"; do
        if [ -n "$candidate" ] && [ -x "$candidate/keytool" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    if command -v keytool >/dev/null 2>&1; then
        dirname "$(command -v keytool)"
        return 0
    fi
    return 1
}

# The coverage preflight: every blocked board entry must have a procedure.
# A new entry without a procedure fails every script in this directory.
blocked_coverage_preflight() {
    local report
    if report="$(python3 "$BLOCKED_DIR/check-blocked-coverage.py" 2>&1)"; then
        blocked_ok "$(printf '%s' "$report" | tail -n 1)"
        return 0
    fi
    blocked_fail "blocked-board coverage is incomplete"
    printf '%s\n' "$report" >&2
    return 1
}

blocked_summary() {
    printf '\n== %s :: summary ==\n' "$BLOCKED_SCRIPT"
    printf '  ok=%d skip=%d pending=%d gap=%d fail=%d\n' \
        "$BLOCKED_OK" "$BLOCKED_SKIPS" "$BLOCKED_PENDING" "$BLOCKED_GAPS" "$BLOCKED_FAILURES"
    printf '  evidence: %s\n' "${BLOCKED_EVIDENCE_DIR#$REPO_ROOT/}"
    if [ "$BLOCKED_PENDING" -gt 0 ]; then
        printf '  remains (environment required):\n'
        local entry
        for entry in "${BLOCKED_RESULTS[@]}"; do
            case "$entry" in
                PENDING\|*) printf '    - %s\n' "${entry#PENDING|}" ;;
            esac
        done
    fi
    if [ "$BLOCKED_GAPS" -gt 0 ]; then
        printf '  repository gaps (another owner must land these; not environment evidence):\n'
        local entry
        for entry in "${BLOCKED_RESULTS[@]}"; do
            case "$entry" in
                GAP\|*) printf '    - %s\n' "${entry#GAP|}" ;;
            esac
        done
    fi
    if [ "$BLOCKED_FAILURES" -gt 0 ]; then
        printf '  defects:\n'
        local entry
        for entry in "${BLOCKED_RESULTS[@]}"; do
            case "$entry" in
                FAIL\|*) printf '    - %s\n' "${entry#FAIL|}" ;;
            esac
        done
    fi
}

# blocked_finish — the single exit point of every script in this directory.
blocked_finish() {
    blocked_summary
    if [ "$BLOCKED_FAILURES" -gt 0 ]; then
        return 1
    fi
    if [ "$BLOCKED_STRICT" -eq 1 ] && [ "$BLOCKED_GAPS" -gt 0 ]; then
        printf '  strict mode: %d repository gap(s) are fatal\n' "$BLOCKED_GAPS" >&2
        return 1
    fi
    return 0
}

# blocked_common_args — shared option parsing. Every script accepts:
#   --dry-run    run everything that does not need the missing environment
#   --strict     treat a repository gap as a failure
#   --help       print the usage of the calling script
blocked_common_args() {
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --dry-run)
                BLOCKED_DRY_RUN=1
                ;;
            --strict)
                BLOCKED_STRICT=1
                ;;
            --help | -h)
                return 10
                ;;
            *)
                return 11
                ;;
        esac
        shift
    done
    return 0
}
