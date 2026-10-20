#!/usr/bin/env bash
# Run every environment-blocked procedure in its dry-run mode.
#
# This is the single command that proves the blocked board stays executable:
# it runs the coverage checker and then every procedure script in dry-run
# mode, so a broken procedure or a board entry without a procedure fails here.
#
#   bash scripts/blocked/run-all.sh              # dry run of everything
#   bash scripts/blocked/run-all.sh --strict     # repository gaps are fatal
#   bash scripts/blocked/run-all.sh --fast       # skip the heavy cross-check legs
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

STRICT=""
FAST=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --strict)
            STRICT="--strict"
            ;;
        --fast)
            FAST=1
            ;;
        --help | -h)
            sed -n '2,10p' "${BASH_SOURCE[0]}"
            exit 0
            ;;
        *)
            printf 'unknown argument: %s\n' "$1" >&2
            exit 2
            ;;
    esac
    shift
done

SCRIPTS=(
    arm64-device-evidence.sh
    tag-ci-evidence.sh
    release-keystore-evidence.sh
    crates-publish-order.sh
    foreign-target-evidence.sh
)

failed=0
for script in "${SCRIPTS[@]}"; do
    printf '\n########## %s ##########\n' "$script"
    args=(--dry-run)
    [ -n "$STRICT" ] && args+=("$STRICT")
    if [ -n "$FAST" ] && [ "$script" = "foreign-target-evidence.sh" ]; then
        args+=(--skip-cross-check --skip-android-check)
    fi
    if bash "$SCRIPT_DIR/$script" "${args[@]}"; then
        printf '########## %s: OK ##########\n' "$script"
    else
        printf '########## %s: FAILED ##########\n' "$script" >&2
        failed=1
    fi
done

printf '\n########## blocked-board coverage ##########\n'
if python3 "$SCRIPT_DIR/check-blocked-coverage.py"; then
    printf '########## blocked-board coverage: OK ##########\n'
else
    printf '########## blocked-board coverage: FAILED ##########\n' >&2
    failed=1
fi

if [ "$failed" -ne 0 ]; then
    printf '\nrun-all: FAILED\n' >&2
    exit 1
fi
printf '\nrun-all: OK\n'
