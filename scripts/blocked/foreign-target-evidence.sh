#!/usr/bin/env bash
# Foreign target machine evidence procedure for the environment-blocked board.
#
# Board group: todos/CODE_TODO_BLOCKED.md §五 "异构目标机与被禁平台 target"
# (5 entries: APPID-001.4, GAP-2.1, GAP-2.2, GAP-3.1, GAP-3.2).
#
# Two kinds of legs are involved:
#
#   * Legs this machine can close: the packaging templates (NSIS, DMG, AppImage,
#     deb) are validated locally, the Apple CI deferral is proven, and the
#     aarch64 desktop and Android cross-checks run on this host. These are the
#     real, local part of the evidence.
#   * Legs that need a foreign target machine: the Windows installer must be
#     built and smoke-installed on Windows, the macOS bundle must be built,
#     signed and notarised on macOS, and the iOS target must be built with the
#     Apple toolchain. Those legs are recorded with the exact command to run
#     on the foreign machine; this script never claims them.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/blocked/lib.sh
. "$SCRIPT_DIR/lib.sh"

BLOCKED_SCRIPT="foreign-target-evidence"
DRY_RUN=0
SKIP_CROSS=0
SKIP_ANDROID=0

usage() {
    cat <<'EOF'
usage: bash scripts/blocked/foreign-target-evidence.sh [--dry-run] [--strict]
                                                        [--skip-cross-check]
                                                        [--skip-android-check]

  --dry-run             validate everything this machine can validate and
                        record the foreign legs with their exact commands
  --skip-cross-check    do not run scripts/cross-check.sh (aarch64 desktop legs)
  --skip-android-check  do not run scripts/android-check.sh (aarch64 Android legs)

Exit codes: 0 = local steps passed, 1 = real defect, 2 = environment missing.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run)
            DRY_RUN=1
            ;;
        --skip-cross-check)
            SKIP_CROSS=1
            ;;
        --skip-android-check)
            SKIP_ANDROID=1
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

blocked_step "Apple CI deferral policy (GAP-2.2 / GAP-3.2)"
# The deferral guard is the single source of truth for the Apple policy; this
# procedure only reports its verdict.
if blocked_run "apple-ci-deferral-guard.py" \
    python3 scripts/quality/apple-ci-deferral-guard.py; then
    blocked_ok "the Apple CI deferral is in force: no macOS runner and no Apple target is scheduled"
else
    blocked_fail "the Apple CI deferral guard failed: an Apple runner or target is scheduled"
fi

blocked_step "packaging templates (NSIS / DMG / AppImage / deb)"
blocked_run "verify-packaging.py" python3 scripts/quality/verify-packaging.py || true

blocked_step "release build targets and product identity"
blocked_run "release-build-target-guard.py" \
    python3 scripts/quality/release-build-target-guard.py || true
blocked_run "android-product-identity-guard.py" \
    python3 scripts/quality/android-product-identity-guard.py || true

blocked_step "aarch64 desktop cross-check legs on this machine"
if [ "$SKIP_CROSS" -eq 1 ]; then
    blocked_skip "scripts/cross-check.sh (--skip-cross-check)"
else
    blocked_run "cross-check.sh (aarch64-unknown-linux-gnu, aarch64-pc-windows-msvc)" \
        bash scripts/cross-check.sh || true
fi

blocked_step "aarch64 Android cross-check legs on this machine"
if [ "$SKIP_ANDROID" -eq 1 ]; then
    blocked_skip "scripts/android-check.sh (--skip-android-check)"
else
    blocked_run "android-check.sh (aarch64-linux-android, x86_64-linux-android)" \
        bash scripts/android-check.sh || true
fi

blocked_step "foreign target legs and their exact commands"
LEGS="$EVIDENCE/foreign-legs.md"
VERSION="$(blocked_identity version.name)"
cat > "$LEGS" <<EOF
# Foreign target legs

| Board entry | Target | What must run there | Exact command |
| --- | --- | --- | --- |
| APPID-001.4 | Windows x86_64 | NSIS installer build, then install/uninstall smoke | \`bash packaging/windows/nsis/build-nsis.sh iced $VERSION target/x86_64-pc-windows-msvc/release/infiltrator-iced.exe dist/Infiltrator-Setup-x86_64.exe\` |
| APPID-001.4 | Windows x86_64 | Bevy NSIS installer | \`bash packaging/windows/nsis/build-nsis.sh bevy $VERSION target/x86_64-pc-windows-msvc/release/infiltrator-bevy-ui.exe dist/Infiltrator-Bevy-Setup-x86_64.exe\` |
| APPID-001.4 | Windows x86_64 | Slint NSIS installer | \`bash packaging/windows/nsis/build-nsis.sh slint $VERSION target/x86_64-pc-windows-msvc/release/infiltrator-slint.exe dist/Infiltrator-Slint-Setup-x86_64.exe\` |
| APPID-001.4 | Windows aarch64 | the ARM64 Windows binaries | \`cargo build --locked --release --target aarch64-pc-windows-msvc -p infiltrator-iced --bin infiltrator-iced\` |
| GAP-2.1 | Windows x86_64 | install, tray and TUN smoke of the installed package | \`dist/Infiltrator-Setup-x86_64.exe /S\` then the tray and TUN observation |
| GAP-2.1 | Windows x86_64 | uninstall smoke | \`dist/Infiltrator-Setup-x86_64.exe /S /uninstall\` |
| GAP-2.2 | macOS aarch64 | bundle build | \`cargo build --locked --release --target aarch64-apple-darwin -p infiltrator-iced --bin infiltrator-iced\` |
| GAP-2.2 | macOS aarch64 | DMG | \`bash packaging/macos/create-dmg.sh dist/Infiltrator-macOS.dmg "dist/MusicFrog Infiltrator.app"\` |
| GAP-2.2 | macOS aarch64 | signing and notarisation | \`codesign --sign "<Developer ID Application>" --options runtime --timestamp dist/"MusicFrog Infiltrator.app"\` then \`xcrun notarytool submit\` |
| GAP-3.1 | any | APK build and identity receipt | \`cd android && ./gradlew :app:assembleComposeRelease :app:assembleBevyRelease :app:assembleSlintRelease\` |
| GAP-3.2 | iOS aarch64 | the real iOS target build | \`cargo build --locked --release --target aarch64-apple-ios\` |
| GAP-3.2 | iOS aarch64 | app bundle and NetworkExtension package structure | package inspection of the produced \`.app\` / \`.appex\` |

## Prerequisites of the foreign legs

- **Windows**: a Windows host or runner with the NSIS toolchain
  (\`choco install nsis\`) and the Rust \`x86_64-pc-windows-msvc\` /
  \`aarch64-pc-windows-msvc\` targets. The installer smoke (install, tray,
  TUN, uninstall) must run on the real Windows system.
- **macOS**: a macOS host or runner with the Apple toolchain and a Developer ID
  identity. The Apple CI deferral guard must be lifted *deliberately* before a
  macOS runner is added to a workflow; the current decision keeps it closed.
- **iOS**: the same deferral decision plus the \`aarch64-apple-ios\` target and
  the NetworkExtension skeleton. Today the repository only holds a host stub.

## Legs that this machine closes

| Leg | Local command | Result |
| --- | --- | --- |
| packaging templates | \`python3 scripts/quality/verify-packaging.py\` | the NSIS/DMG/AppImage/deb templates are validated |
| Apple deferral | \`python3 scripts/quality/apple-ci-deferral-guard.py\` | no macOS runner and no Apple target is scheduled |
| aarch64 desktop | \`bash scripts/cross-check.sh\` | the aarch64 Linux and Windows product crates compile |
| aarch64 Android | \`bash scripts/android-check.sh\` | the aarch64 and x86_64 Android crates compile and lint |
EOF
blocked_ok "foreign leg table written to ${LEGS#"$REPO_ROOT"/}"

blocked_step "what still needs the environment"
blocked_pending "a Windows target machine for the NSIS build and the install/tray/TUN/uninstall smoke (APPID-001.4, GAP-2.1)"
blocked_pending "a macOS target machine and the Apple CI deferral decision for the bundle, signing and notarisation (GAP-2.2)"
blocked_pending "the Apple target unblocking decision and the macOS toolchain for the aarch64-apple-ios build (GAP-3.2)"
blocked_pending "the tag-run APK build and its identity receipt for the Android packaging fix (GAP-3.1)"

blocked_finish
exit $?
