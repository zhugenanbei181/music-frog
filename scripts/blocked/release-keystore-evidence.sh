#!/usr/bin/env bash
# Release keystore evidence procedure for the environment-blocked board.
#
# Board group: todos/CODE_TODO_BLOCKED.md §三 "用户 secrets 与持久 keystore"
# (3 entries: BANDROID-039.1, BANDROID-015.2, BANDROID-039.2).
#
# The blocker is a secret the user holds: a persistent release keystore and
# the CI secrets that carry it. This script therefore separates the two halves
# of the evidence:
#
#   * Without the secret it validates the *wiring contract* — which secret
#     names must exist, where the release build must read them from, and that
#     the release path never silently falls back to the debug key — and it
#     prints the exact command to run once the secret is handed over.
#   * With the secret it performs the real signature checks: the keystore
#     certificate fingerprint, the APK signer certificate digest, the aapt /
#     identity check of the release universal APK and the fingerprint equality
#     of two consecutive candidates.
#
# The private key never enters the repository, the logs or the evidence
# directory: the keystore is materialised into a private temporary directory
# that is removed on exit, and the script refuses a keystore path inside the
# repository.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/blocked/lib.sh
. "$SCRIPT_DIR/lib.sh"

BLOCKED_SCRIPT="release-keystore-evidence"
DRY_RUN=0
APK_A=""
APK_B=""

# The single secret contract of the release signing path. The workflow owner
# must use exactly these names so this script and CI agree.
SECRET_BASE64="RELEASE_KEYSTORE_BASE64"
SECRET_STORE_PASSWORD="RELEASE_KEYSTORE_PASSWORD"
SECRET_KEY_ALIAS="RELEASE_KEY_ALIAS"
SECRET_KEY_PASSWORD="RELEASE_KEY_PASSWORD"

usage() {
    cat <<'EOF'
usage: bash scripts/blocked/release-keystore-evidence.sh [--dry-run] [--strict]
                                                         [--apk-a PATH] [--apk-b PATH]

  --dry-run   validate the wiring contract only; never touch a secret
  --apk-a     first release candidate APK (its signer certificate is read back)
  --apk-b     second release candidate APK (fingerprint equality, BANDROID-039.2)
  --strict    treat a repository gap as a failure

Environment (only needed once the user hands the keystore over):
  RELEASE_KEYSTORE_PATH     path to the .jks (never inside the repository)
  RELEASE_KEYSTORE_BASE64   base64 of the .jks (alternative to the path)
  RELEASE_KEYSTORE_PASSWORD keystore store password
  RELEASE_KEY_ALIAS         key alias inside the keystore
  RELEASE_KEY_PASSWORD      key password

Exit codes: 0 = local steps passed, 1 = real defect, 2 = environment missing.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run)
            DRY_RUN=1
            ;;
        --apk-a)
            APK_A="$2"
            shift
            ;;
        --apk-b)
            APK_B="$2"
            shift
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

# apksigner and keytool are Java tools: they need a JDK on PATH.
JAVA_BIN="$(blocked_java_bin || true)"
if [ -n "$JAVA_BIN" ]; then
    export JAVA_HOME="$(cd -- "$JAVA_BIN/.." && pwd)"
    export PATH="$JAVA_BIN:$PATH"
    blocked_ok "JDK for keytool and apksigner: $JAVA_BIN"
else
    blocked_gap "no JDK with keytool was found; the keystore fingerprint and the signer digest cannot be read"
fi

blocked_step "blocked-board coverage preflight"
blocked_coverage_preflight || true

blocked_step "release signing wiring contract"
GRADLE_FILE="android/app/build.gradle.kts"
WORKFLOW_FILE=".github/workflows/release.yml"
CONTRACT="$EVIDENCE/signing-contract.txt"
{
    echo "secret names (must exist as repository secrets before the release job runs):"
    echo "  $SECRET_BASE64      base64 of the persistent release keystore"
    echo "  $SECRET_STORE_PASSWORD  keystore store password"
    echo "  $SECRET_KEY_ALIAS      key alias"
    echo "  $SECRET_KEY_PASSWORD   key password"
    echo
    echo "gradle file: $GRADLE_FILE"
    echo "workflow file: $WORKFLOW_FILE"
} > "$CONTRACT"

if [ ! -f "$GRADLE_FILE" ]; then
    blocked_fail "missing $GRADLE_FILE"
elif ! grep -q "$SECRET_BASE64" "$GRADLE_FILE"; then
    blocked_gap "$GRADLE_FILE does not read $SECRET_BASE64: the release signing config is not wired yet"
else
    blocked_ok "$GRADLE_FILE reads the release keystore from the environment"
fi

if grep -qE 'signingConfig = signingConfigs\.getByName\("debug"\)' "$GRADLE_FILE"; then
    blocked_gap "$GRADLE_FILE signs the release buildType with the debug key: that is the silent debug fallback the board forbids"
else
    blocked_ok "the release buildType does not use the debug signing config"
fi

if ! grep -qE "$SECRET_BASE64|$SECRET_STORE_PASSWORD|$SECRET_KEY_ALIAS|$SECRET_KEY_PASSWORD" "$WORKFLOW_FILE"; then
    blocked_gap "$WORKFLOW_FILE carries no release keystore secret: the publish job cannot fail closed on a missing secret yet"
else
    blocked_ok "$WORKFLOW_FILE carries the release keystore secrets"
fi

if grep -qE 'signingConfigs\.create\("release"\)|signingConfig = signingConfigs\.getByName\("release"\)' "$GRADLE_FILE"; then
    blocked_ok "a dedicated release signing config exists"
else
    blocked_gap "$GRADLE_FILE has no dedicated release signing config"
fi
blocked_ok "the signing contract is recorded in ${CONTRACT#"$REPO_ROOT"/}"

blocked_step "secret availability"
KEYSTORE=""
KEYSTORE_DIR=""
cleanup() {
    if [ -n "$KEYSTORE_DIR" ] && [ -d "$KEYSTORE_DIR" ]; then
        rm -rf "$KEYSTORE_DIR"
    fi
}
trap cleanup EXIT

if [ -n "${RELEASE_KEYSTORE_PATH:-}" ]; then
    if [ ! -f "$RELEASE_KEYSTORE_PATH" ]; then
        blocked_fail "RELEASE_KEYSTORE_PATH does not exist: $RELEASE_KEYSTORE_PATH"
        blocked_finish
        exit $?
    fi
    case "$(cd "$(dirname "$RELEASE_KEYSTORE_PATH")" && pwd)/$(basename "$RELEASE_KEYSTORE_PATH")" in
        "$REPO_ROOT"/*)
            blocked_fail "the keystore must not live inside the repository"
            blocked_finish
            exit $?
            ;;
    esac
    KEYSTORE="$(cd "$(dirname "$RELEASE_KEYSTORE_PATH")" && pwd)/$(basename "$RELEASE_KEYSTORE_PATH")"
    blocked_ok "keystore supplied through RELEASE_KEYSTORE_PATH"
elif [ -n "${RELEASE_KEYSTORE_BASE64:-}" ]; then
    KEYSTORE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/blocked-keystore.XXXXXX")"
    chmod 700 "$KEYSTORE_DIR"
    KEYSTORE="$KEYSTORE_DIR/release.jks"
    printf '%s' "$RELEASE_KEYSTORE_BASE64" | base64 -d > "$KEYSTORE"
    chmod 600 "$KEYSTORE"
    blocked_ok "keystore supplied through $SECRET_BASE64 and materialised in a private temporary directory"
else
    blocked_ok "no release keystore is available: the fail-closed path is verified"
    blocked_pending "the persistent release keystore and its four CI secrets ($SECRET_BASE64, $SECRET_STORE_PASSWORD, $SECRET_KEY_ALIAS, $SECRET_KEY_PASSWORD)"
fi

blocked_step "keystore certificate fingerprint"
FINGERPRINT=""
if [ -n "$KEYSTORE" ]; then
    if [ -z "${RELEASE_KEYSTORE_PASSWORD:-}" ] || [ -z "${RELEASE_KEY_ALIAS:-}" ]; then
        blocked_fail "$SECRET_STORE_PASSWORD and $SECRET_KEY_ALIAS must be set to read the keystore"
    else
        JAVA_BIN="$(blocked_java_bin || true)"
        if [ -z "$JAVA_BIN" ]; then
            blocked_fail "no JDK with keytool was found"
        else            KEYTOOL_OUT="$EVIDENCE/keystore-certificate.txt"
            if "$JAVA_BIN/keytool" -list -v \
                -keystore "$KEYSTORE" \
                -alias "$RELEASE_KEY_ALIAS" \
                -storepass "${RELEASE_KEYSTORE_PASSWORD}" > "$KEYTOOL_OUT" 2>&1; then
                FINGERPRINT="$(grep -E 'SHA256:' "$KEYTOOL_OUT" | head -1 | sed 's/.*SHA256: *//' | tr -d '\r')"
                if [ -n "$FINGERPRINT" ]; then
                    blocked_ok "release certificate SHA-256 fingerprint: $FINGERPRINT"
                    printf '%s\n' "$FINGERPRINT" > "$EVIDENCE/release-fingerprint.txt"
                else
                    blocked_fail "the keystore certificate has no SHA-256 fingerprint (see keystore-certificate.txt)"
                fi
            else
                blocked_fail "keytool could not read the keystore (see keystore-certificate.txt)"
            fi
        fi
    fi
else
    blocked_skip "no keystore to read the fingerprint from"
fi

blocked_step "release APK signature and identity"
SDK_ROOT="$(blocked_android_sdk_root || true)"
BUILD_TOOLS="$(python3 scripts/android-toolchain.py get android.build.tools 2>/dev/null || true)"
APKSIGNER=""
if [ -n "$SDK_ROOT" ] && [ -n "$BUILD_TOOLS" ] && [ -x "$SDK_ROOT/build-tools/$BUILD_TOOLS/apksigner" ]; then
    APKSIGNER="$SDK_ROOT/build-tools/$BUILD_TOOLS/apksigner"
fi

signer_digest() {
    local apk="$1"
    "$APKSIGNER" verify --print-certs "$apk" 2>/dev/null \
        | sed -n 's/^Signer #1 certificate SHA-256 digest: *//p' \
        | head -1 | tr -d '\r' || true
}

DIGEST_A=""
DIGEST_B=""
CANDIDATES=0
if [ -z "$APK_A" ] && [ -z "$APK_B" ]; then
    blocked_skip "no release candidate APK was given (--apk-a / --apk-b)"
fi
for pair in "A:$APK_A" "B:$APK_B"; do
    label="${pair%%:*}"
    apk="${pair#*:}"
    [ -n "$apk" ] || continue
    CANDIDATES=$((CANDIDATES + 1))
    if [ ! -f "$apk" ]; then
        blocked_fail "the $label candidate APK does not exist: $apk"
        continue
    fi
    apk_sha="$(sha256sum "$apk" | awk '{print $1}')"
    blocked_ok "candidate $label ${apk#"$REPO_ROOT"/} sha256=$apk_sha"
    if [ -z "$APKSIGNER" ]; then
        blocked_gap "apksigner is not available; the signer certificate of the $label candidate cannot be read back"
        continue
    fi
    digest="$(signer_digest "$apk" || true)"
    if [ -z "$digest" ]; then
        blocked_fail "apksigner could not read the signer certificate of the $label candidate"
        continue
    fi
    printf '%s\n' "$digest" > "$EVIDENCE/signer-digest-$label.txt"
    if [ "$label" = "A" ]; then
        DIGEST_A="$digest"
    else
        DIGEST_B="$digest"
    fi
    blocked_ok "candidate $label signer certificate SHA-256 digest: $digest"
    if [ -n "$FINGERPRINT" ] && [ "$digest" != "$FINGERPRINT" ]; then
        blocked_fail "candidate $label is not signed with the supplied release keystore"
    fi
    if [ -n "$SDK_ROOT" ] && [ -n "$BUILD_TOOLS" ]; then
        product=""
        case "$apk" in
            *compose*) product="compose" ;;
            *bevy*) product="bevy" ;;
            *slint*) product="slint" ;;
        esac
        if [ -z "$product" ]; then
            blocked_gap "the product of the $label candidate cannot be derived from its path; the aapt identity check was skipped"
        else
            blocked_run "android-product-identity-guard.py --apk $product" \
                python3 scripts/quality/android-product-identity-guard.py \
                --aapt "$SDK_ROOT/build-tools/$BUILD_TOOLS/aapt" \
                --apk "$product=$apk" || true
        fi
    fi
done

blocked_step "two consecutive candidates share the signer"
if [ "$CANDIDATES" -eq 0 ]; then
    blocked_skip "no release candidate APK was given (--apk-a / --apk-b)"
elif [ -n "$DIGEST_A" ] && [ -n "$DIGEST_B" ]; then
    if [ "$DIGEST_A" = "$DIGEST_B" ]; then
        blocked_ok "both candidates are signed with the same certificate ($DIGEST_A)"
    else
        blocked_fail "the two candidates have different signer certificates"
    fi
else
    blocked_pending "the upgrade leg needs two signed release candidates (--apk-a and --apk-b)"
fi

blocked_step "the upgrade leg on a device"
if command -v adb >/dev/null 2>&1 || { SDK_ROOT="$(blocked_android_sdk_root || true)" && [ -x "${SDK_ROOT:-}/platform-tools/adb" ]; }; then
    blocked_pending "adb install -r of the previous candidate over the new one (physical ARM64 device)"
else
    blocked_pending "a physical ARM64 device for the 'adb install -r' upgrade verification"
fi

blocked_step "what still needs the environment"
blocked_pending "the dedicated release signing config and the fail-closed publish job (repository change, see the report)"
if [ "$DRY_RUN" -eq 1 ] && [ -n "$KEYSTORE" ]; then
    blocked_ok "dry run finished; the keystore was only read, never copied into the repository"
fi

blocked_finish
exit $?
