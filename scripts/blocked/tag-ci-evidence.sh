#!/usr/bin/env bash
# Controlled tag CI evidence procedure for the environment-blocked board.
#
# Board group: todos/CODE_TODO_BLOCKED.md §二 "受控 tag CI 回执" (5 entries).
#
# The board's blocker is a *user decision*, not a missing tool: the user chose
# not to push a `v*` tag just to produce CI evidence. This script therefore
# does two things:
#
#   1. It proves, locally and without any tag, that everything the tag
#      pipeline would run is already aligned: the trigger policy that keeps the
#      expensive workflows off ordinary pushes, the release build targets, the
#      product identity against the exact tag, the manifest contract, the
#      Android toolchain baseline and the local release rehearsal.
#   2. It prints the exact tag and the exact run to request, and reports an
#      already existing run for that tag instead of asking for a second one.
#
# A dry run never produces a CI receipt: `--dry-run` stops at the request
# boundary. Without `--dry-run` the script still never pushes a tag or starts a
# workflow on its own — it only reports what exists and what to request.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/blocked/lib.sh
. "$SCRIPT_DIR/lib.sh"

BLOCKED_SCRIPT="tag-ci-evidence"
DRY_RUN=0
WITH_DRY_RUN=0

usage() {
    cat <<'EOF'
usage: bash scripts/blocked/tag-ci-evidence.sh [--dry-run] [--strict] [--with-dry-run]

  --dry-run       validate everything locally and print the exact tag and run
                  to request; never asks for a tag to be pushed
  --with-dry-run  additionally run the local `cargo publish --dry-run`
                  rehearsal of the neutral release chain (network, ~1 minute)
  --strict        treat a repository gap as a failure

Exit codes: 0 = local steps passed, 1 = real defect, 2 = environment missing.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run)
            DRY_RUN=1
            ;;
        --with-dry-run)
            WITH_DRY_RUN=1
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

blocked_step "workflow trigger policy (the guard that pins the v* tag rule)"
blocked_run "ci-trigger-guard.py" python3 scripts/quality/ci-trigger-guard.py || true

blocked_step "release build targets"
blocked_run "release-build-target-guard.py" \
    python3 scripts/quality/release-build-target-guard.py || true

VERSION="$(blocked_identity version.name)"
TAG="v${VERSION}"
blocked_ok "candidate release tag: $TAG"

blocked_step "product identity against the exact release tag"
blocked_run "android-product-identity-guard.py --release-tag $TAG" \
    python3 scripts/quality/android-product-identity-guard.py --release-tag "$TAG" || true

blocked_step "manifest contract guard (BANDROID-019.2 local leg)"
blocked_run "android-manifest-contract-guard.py" \
    python3 scripts/quality/android-manifest-contract-guard.py || true

blocked_step "Android SDK/NDK/Gradle/JDK baseline (BANDROID-020.1 local leg)"
blocked_run "android-toolchain.py check" python3 scripts/android-toolchain.py check || true

blocked_step "workflow inventory and tag gate"
INVENTORY="$EVIDENCE/workflow-inventory.txt"
{
    printf '%-28s %-22s %-8s %s\n' "workflow" "events" "tag-gated" "jobs"
    for workflow in .github/workflows/*.yml; do
        name="$(sed -n 's/^name: *//p' "$workflow" | head -1)"
        events="$(awk '
            /^on:/ { inside = 1; next }
            inside && /^  [A-Za-z_]+:/ { printf "%s ", $1 }
            inside && /^[^ ]/ { exit }
        ' "$workflow")"
        jobs="$(awk '/^jobs:/ { inside = 1; next } inside && /^  [A-Za-z_-]+:/ { printf "%s ", $1 }' "$workflow")"
        gated="$(grep -c "ref_type == 'tag'" "$workflow" || true)"
        printf '%-28s %-22s %-8s %s\n' "$(basename "$workflow")" "${events:-?}" "$gated" "${jobs:-?}"
    done
} | tee "$INVENTORY"
blocked_ok "workflow inventory written to ${INVENTORY#"$REPO_ROOT"/}"

blocked_step "local release rehearsal (offline topology)"
blocked_run "neutral-crate-release.sh structure" \
    bash scripts/neutral-crate-release.sh structure || true

if [ "$WITH_DRY_RUN" -eq 1 ]; then
    blocked_step "local release rehearsal (ordered cargo publish --dry-run)"
    blocked_run "neutral-crate-release.sh dry-run" \
        bash scripts/neutral-crate-release.sh dry-run || true
else
    blocked_skip "ordered cargo publish --dry-run rehearsal (pass --with-dry-run to run it)"
fi

blocked_step "tag state and existing CI runs"
LOCAL_TAG_EXISTS=0
REMOTE_TAG_EXISTS=0
if git rev-parse "refs/tags/$TAG" >/dev/null 2>&1; then
    blocked_ok "the tag $TAG already exists locally"
    LOCAL_TAG_EXISTS=1
else
    blocked_ok "the tag $TAG does not exist yet"
fi
REMOTE_TAGS="$(git ls-remote --tags origin "refs/tags/$TAG" 2>/dev/null || true)"
if [ -z "$REMOTE_TAGS" ]; then
    blocked_skip "the remote could not be reached; the remote tag state is unknown"
elif [ "$(printf '%s\n' "$REMOTE_TAGS" | wc -l | tr -d ' ')" -gt 0 ]; then
    blocked_ok "the tag $TAG already exists on the remote"
    REMOTE_TAG_EXISTS=1
else
    blocked_ok "the tag $TAG is not on the remote yet"
fi

TAG_COMMIT="$(git rev-list -n1 "$TAG" 2>/dev/null || true)"
HEAD_SHA="$(git rev-parse HEAD)"
if [ -n "$TAG_COMMIT" ] && [ "$TAG_COMMIT" != "$HEAD_SHA" ]; then
    blocked_gap "the tag $TAG points at $TAG_COMMIT while HEAD is $HEAD_SHA: a receipt bound to the current code needs a new tag"
elif [ -n "$TAG_COMMIT" ]; then
    blocked_ok "the tag $TAG points at HEAD ($HEAD_SHA)"
fi

RUNS_FILE="$EVIDENCE/existing-runs.txt"
: > "$RUNS_FILE"
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    for workflow in release.yml android.yml bevy.yml parity.yml release-neutral-crates.yml; do
        printf '## %s\n' "$workflow" >> "$RUNS_FILE"
        gh run list --workflow "$workflow" --branch "$TAG" --limit 10 \
            --json databaseId,conclusion,status,headSha,event,url,createdAt \
            >> "$RUNS_FILE" 2>/dev/null || printf '[]\n' >> "$RUNS_FILE"
        latest="$(gh run list --workflow "$workflow" --branch "$TAG" --limit 1 \
            --json databaseId,conclusion,headSha,url \
            --jq '.[0] | if . == null then "" else "\(.databaseId) \(.conclusion) \(.headSha) \(.url)" end' 2>/dev/null || true)"
        if [ -n "$latest" ]; then
            blocked_ok "$workflow: the latest run of that tag is $latest"
        else
            blocked_skip "$workflow: no run recorded for that tag"
        fi
    done
    blocked_ok "existing workflow runs recorded in ${RUNS_FILE#"$REPO_ROOT"/}"

    PUBLISHED="$EVIDENCE/published-workflows.txt"
    : > "$PUBLISHED"
    for workflow in .github/workflows/*.yml; do
        name="$(basename "$workflow")"
        if gh api "repos/:owner/:repo/contents/.github/workflows/$name" >/dev/null 2>&1; then
            printf '%s published\n' "$name" >> "$PUBLISHED"
        else
            printf '%s NOT published on the default branch\n' "$name" >> "$PUBLISHED"
        fi
    done
    while read -r state; do
        case "$state" in
            *NOT\ published*)
                blocked_gap "$state: a tag push today would not start this workflow; the file is on the local main branch, so pushing main publishes it"
                ;;
        esac
    done < "$PUBLISHED"
    blocked_ok "workflow publication on the default branch recorded in ${PUBLISHED#"$REPO_ROOT"/}"
else
    blocked_skip "gh is not available or not authenticated; the run inventory is not collected"
fi

blocked_step "the exact tag and run to request"
REQUEST="$EVIDENCE/request.md"
{
    echo "# Controlled tag CI request"
    echo
    echo "Candidate release tag: \`$TAG\` (product version \`$VERSION\`, from the product identity registry)."
    echo
    echo "Tag state: local=$LOCAL_TAG_EXISTS remote=$REMOTE_TAG_EXISTS commit=${TAG_COMMIT:-<none>} HEAD=$HEAD_SHA"
    echo
    echo "The user decision recorded on the board is \"no tag just for CI evidence\". The"
    echo "commands below are the exact request; they are printed, never executed by this"
    echo "script."
    echo
    if [ "$LOCAL_TAG_EXISTS" -eq 1 ] && [ -n "$TAG_COMMIT" ] && [ "$TAG_COMMIT" != "$HEAD_SHA" ]; then
        echo "> The tag \`$TAG\` already exists and points at an older commit. The"
        echo "> \`release\` workflow is tag-push only, so a receipt bound to the current"
        echo "> code needs a tag whose commit is HEAD. Because the product identity"
        echo "> registry pins the release version to \`$VERSION\`, that means either a"
        echo "> version bump in the same commit as the tag, or a deliberate decision to"
        echo "> move the existing tag. Both are release decisions, not script decisions."
        echo
    fi
    echo "## 1. Product release pipeline (tag push)"
    echo
    echo '```bash'
    echo "git tag -a \"$TAG\" -m \"release $TAG\""
    echo "git push origin \"$TAG\""
    echo "gh run list --workflow release.yml --branch \"$TAG\""
    echo "gh run watch --workflow release.yml"
    echo '```'
    echo
    echo "Evidence to archive from that run:"
    echo
    echo "| Board entry | Job / step of the run | Evidence |"
    echo "| --- | --- | --- |"
    echo "| BANDROID-018.2 | \`release\` / \`Release test gate\` + \`Build (target)\` + \`Build Android APKs\` | the run URL and the run id |"
    echo "| BANDROID-019.2 | \`Build Android APKs\` / \`Verify and collect registered Android APKs\` | the aapt identity output per flavor |"
    echo "| BANDROID-038.2 | \`Build Android APKs\` / \`Verify and collect registered Android APKs\` | the APK SHA-256 per flavor |"
    echo "| BANDROID-020.1 | \`Build Android APKs\` / \`Verify pinned Android toolchain\` | the toolchain check output |"
    echo "| BANDROID-024.1 | \`Build Android APKs\` / \`Build all Android release flavors\` | the Gradle build output |"
    echo "| GAP-3.1 | \`Build Android APKs\` | the APK artifact and its identity receipt |"
    echo
    echo "## 2. Android gate (manual dispatch on the tag)"
    echo
    echo '```bash'
    echo "gh workflow run android.yml --ref \"$TAG\" -f flavor=all -f run_emulator=true"
    echo "gh run list --workflow android.yml --branch \"$TAG\""
    echo '```'
    echo
    echo "## 3. Neutral crates release chain (dry-run rehearsal, no upload)"
    echo
    echo '```bash'
    echo "gh workflow run release-neutral-crates.yml --ref \"$TAG\""
    echo "gh run list --workflow release-neutral-crates.yml --branch \"$TAG\""
    echo '```'
    echo
    echo "A dispatched run of this workflow only reaches the \`dry-run\` job; the"
    echo "\`publish\` job requires a \`v*\` tag push and the \`CARGO_REGISTRY_TOKEN\`"
    echo "secret. The dispatch therefore produces the ordered \`cargo publish --dry-run\`"
    echo "receipt without uploading anything and without any tag. The workflow must be"
    echo "published on the default branch before a dispatch can find it."
    echo
    echo "## 4. Bevy and parity gates (manual dispatch on the tag)"
    echo
    echo '```bash'
    echo "gh workflow run bevy.yml --ref \"$TAG\""
    echo "gh workflow run parity.yml --ref \"$TAG\""
    echo '```'
    echo
    echo "## 5. ARM64 device evidence (self-hosted runner labelled \`android-arm64\`)"
    echo
    echo '```bash'
    echo "gh workflow run android-device.yml --ref \"$TAG\" -f background_seconds=28800"
    echo '```'
    echo
    echo "This workflow needs a self-hosted runner with a physical ARM64 device; it is"
    echo "the CI counterpart of \`scripts/blocked/arm64-device-evidence.sh\`."
} > "$REQUEST"
blocked_ok "the request is written to ${REQUEST#"$REPO_ROOT"/}"

blocked_step "what still needs the environment"
blocked_pending "a controlled tag push whose commit is HEAD, and the workflow run of that tag"
if [ "$DRY_RUN" -eq 1 ]; then
    blocked_ok "dry run: nothing was pushed, no workflow was started"
else
    blocked_ok "no tag was pushed and no workflow was started by this script"
fi

blocked_finish
exit $?
