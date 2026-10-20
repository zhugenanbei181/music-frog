#!/usr/bin/env bash
# crates.io publish-order rehearsal for the environment-blocked board.
#
# Board group: todos/CODE_TODO_BLOCKED.md §四 "crates.io 上架与发布顺序门"
# (5 entries: PUB-001.3, PUB-003.2, PUB-003.3, PUB-006.12, PUB-006.5).
#
# The seven neutral crates are released as one ordered chain:
#
#   lurker-core-contract -> lurker-core-ports -> lurker-engine-runtime
#   -> lurker-profile-store -> lurker-ui-presentation
#   -> mihomo-driver -> singbox-driver
#
# This script runs the local rehearsal of that chain in the enforced order and
# tells the two possible failures apart:
#
#   * `order-gate`   a neutral dependency of the crate is not on crates.io
#                    yet. This is the expected state before the chain is
#                    published and it is *not* a defect of the package.
#   * `packaging-failure`
#                    a genuine packaging or verification defect. The run
#                    stops at the first one, because a broken package must
#                    never reach crates.io.
#
# The classifier is the repository's own release guard; this script only
# sequences it. Nothing is ever uploaded: the real upload is the `publish`
# mode of the guard and needs the crates.io API token.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/blocked/lib.sh
. "$SCRIPT_DIR/lib.sh"

BLOCKED_SCRIPT="crates-publish-order"
DRY_RUN=0
WITH_CHECKSUMS=0
TIMEOUT="${BLOCKED_CARGO_TIMEOUT:-1800}"

usage() {
    cat <<'EOF'
usage: bash scripts/blocked/crates-publish-order.sh [--dry-run] [--strict]
                                                     [--with-checksums]

  --dry-run         run the rehearsal; the script never uploads anything
  --with-checksums  additionally build the `.crate` artifacts and write the
                    SHA-256 checksum manifest of the release candidate baseline
  --strict          treat a repository gap as a failure

Exit codes: 0 = rehearsal passed (order gates are expected), 1 = real defect,
2 = environment missing.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run)
            DRY_RUN=1
            ;;
        --with-checksums)
            WITH_CHECKSUMS=1
            ;;
        --strict)
            BLOCKED_STRICT=1
            ;;
        --help | -h)
            usage
            exit 0
            ;;
        *)
            printf 'unknown argument: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

cd "$REPO_ROOT"
blocked_evidence_dir "$BLOCKED_SCRIPT" >/dev/null
EVIDENCE="$BLOCKED_EVIDENCE_DIR"

blocked_step "blocked-board coverage preflight"
blocked_coverage_preflight || true

blocked_step "derived publish order"
blocked_run "publish-order-guard.py" python3 scripts/quality/publish-order-guard.py || true

blocked_step "release topology of the chain (offline)"
blocked_run "neutral-crate-release.sh structure" \
    bash scripts/neutral-crate-release.sh structure || true

blocked_step "ordered dry-run rehearsal (stops at the first real defect)"
DRY_JSON="$EVIDENCE/dry-run.json"
if blocked_run "publish_chain.py --mode dry-run" \
    python3 "$SCRIPT_DIR/publish_chain.py" --mode dry-run \
    --timeout "$TIMEOUT" --json "$DRY_JSON"; then
    blocked_ok "no packaging defect in the ordered dry-run"
else
    blocked_fail "the ordered dry-run found a packaging defect (see logs/)"
fi

blocked_step "release-candidate checksum manifest"
if [ "$WITH_CHECKSUMS" -eq 1 ]; then
    CHECKSUM_JSON="$EVIDENCE/checksums.json"
    if blocked_run "publish_chain.py --mode checksums" \
        python3 "$SCRIPT_DIR/publish_chain.py" --mode checksums \
        --timeout "$TIMEOUT" --json "$CHECKSUM_JSON"; then
        blocked_ok "checksum manifest written to ${CHECKSUM_JSON#"$REPO_ROOT"/}"
    else
        blocked_fail "the checksum manifest found a packaging defect"
    fi
else
    blocked_skip "checksum manifest (pass --with-checksums to run it; it packages every crate of the chain)"
fi

blocked_step "what the board still needs"
blocked_pending "the crates.io API token (repository secret CARGO_REGISTRY_TOKEN) for the real upload"
blocked_pending "the first upload of lurker-core-contract, after which the remaining six crates can be dry-run in order"
blocked_pending "the 1.0.0-rc.1 baseline freeze of the seven neutral crates and the GPG signing material (PUB-006.12)"
blocked_pending "the v1.0.0 tag, the release notes and the docs.rs build receipts (PUB-006.5)"

REQUEST="$EVIDENCE/request.md"
cat > "$REQUEST" <<'EOF'
# crates.io publish request

## 1. Upload the chain in the enforced order (needs the token)

```bash
export CARGO_REGISTRY_TOKEN=...          # crates.io token with publish-scope
bash scripts/neutral-crate-release.sh publish
```

The publish job waits until crates.io has indexed each new version before the
next crate resolves its dependency. The order is derived from the dependency
graph by `scripts/quality/publish-order-guard.py`; it is never a convention.

## 2. Re-run the rehearsal after the contract is on crates.io

```bash
bash scripts/blocked/crates-publish-order.sh --dry-run
```

Every crate must report `ok`. A crate that still reports `order-gate` means
its predecessor is not indexed yet — wait for the crates.io index and re-run.

## 3. Per-crate receipt required by the board

| Board entry | Receipt |
| --- | --- |
| PUB-001.3 | `cargo publish --dry-run -p lurker-profile-store` reports `ok` |
| PUB-003.2 | the ordered dry-run reports `ok` for `lurker-engine-runtime` |
| PUB-003.3 | the ordered dry-run reports `ok` for `lurker-engine-runtime` |
| PUB-006.12 | the seven crates carry `1.0.0-rc.1`, the checksum manifest is archived and the GPG signature draft exists |
| PUB-006.5 | the `v1.0.0` tag, the release notes, the seven crates.io version receipts and the seven docs.rs build links |

## 4. Release-candidate baseline freeze (PUB-006.12)

The seven neutral crates are versioned from `[workspace.package].version` in
`Cargo.toml`. Freezing them on `1.0.0-rc.1` is a single identity change that
must be done once, in one commit:

```bash
# edit [workspace.package].version to 1.0.0-rc.1
python3 scripts/quality/publish-order-guard.py
python3 scripts/quality/neutral-release-guard.py --mode structure
bash scripts/blocked/crates-publish-order.sh --dry-run
```

The version requirement of every neutral path dependency must match the
frozen version; `neutral-release-guard.py` rejects a mismatch.
EOF
blocked_ok "the publish request is written to ${REQUEST#"$REPO_ROOT"/}"

blocked_finish
exit $?
