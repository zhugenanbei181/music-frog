#!/usr/bin/env bash
# BANDROID-044.5: the Android static analysis quality gate.
#
# The gate runs, in order:
#
#   1. the architecture red-line guard (self-test, then enforce),
#   2. the Kotlin file budget guard (self-test, then enforce),
#   3. the Gradle gates: ktlint, Detekt and Android Lint for the Compose,
#      Bevy and Slint Debug variants.
#
# Every gate is a real gate: no baseline file, no per-file suppression, no
# raised issue threshold. The architecture red-line guard also verifies that
# the gate configuration cannot be weakened behind its back.
#
# Usage:
#   scripts/quality/android-quality-gates.sh              # run every gate
#   scripts/quality/android-quality-gates.sh --skip-gradle
#   scripts/quality/android-quality-gates.sh --self-test  # positive + negative
set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

SKIP_GRADLE=0
SELF_TEST=0
for argument in "$@"; do
    case "$argument" in
        --skip-gradle) SKIP_GRADLE=1 ;;
        --self-test) SELF_TEST=1 ;;
        *)
            printf 'unknown argument: %s\n' "$argument" >&2
            exit 2
            ;;
    esac
done

REDLINE_GUARD="scripts/quality/android-architecture-redline-guard.py"
LINE_GUARD="scripts/quality/kt-line-guard.py"

run_self_test() {
    printf '[android-quality-gates] self-test: the positive case must pass\n'
    python3 "$REDLINE_GUARD" --self-test
    python3 "$LINE_GUARD" --self-test
    python3 "$REDLINE_GUARD" --mode enforce
    python3 "$LINE_GUARD" --mode enforce

    printf '[android-quality-gates] self-test: a deliberate violation must be rejected\n'
    local probe
    probe="$(mktemp -d)"
    trap 'rm -rf "$probe"' RETURN
    mkdir -p "$probe/android/app/src/main/java/com/musicfrog/infiltrator"
    mkdir -p "$probe/android/app/src/main/res/values"
    mkdir -p "$probe/android/config/detekt"
    cp android/app/build.gradle.kts "$probe/android/app/build.gradle.kts"
    cp android/app/lint.xml "$probe/android/app/lint.xml"
    cp android/config/detekt/detekt.yml "$probe/android/config/detekt/detekt.yml"
    cp android/.editorconfig "$probe/android/.editorconfig"
    cp android/app/src/main/AndroidManifest.xml \
        "$probe/android/app/src/main/AndroidManifest.xml"
    cat >"$probe/android/app/src/main/java/com/musicfrog/infiltrator/RedlineProbe.kt" <<'KOTLIN'
package com.musicfrog.infiltrator

internal class RedlineProbe {
    fun read(value: String?): String = value!!
}
KOTLIN
    local output
    if output="$(python3 "$REDLINE_GUARD" --root "$probe" --mode enforce 2>&1)"; then
        printf '[android-quality-gates] self-test FAILED: the deliberate violation was accepted\n' >&2
        printf '%s\n' "$output" >&2
        return 1
    fi
    if ! printf '%s' "$output" | grep -q '\[ARCH006\]'; then
        printf '[android-quality-gates] self-test FAILED: the guard did not report ARCH006\n' >&2
        printf '%s\n' "$output" >&2
        return 1
    fi
    printf '[android-quality-gates] self-test: the deliberate violation was rejected\n'
}

run_gradle_gates() {
    local gradle_dir="$REPO_ROOT/android"
    if [[ -z "${JAVA_HOME:-}" ]]; then
        JAVA_HOME="$(python3 scripts/android-toolchain.py resolve-java)"
        export JAVA_HOME
        export PATH="$JAVA_HOME/bin:$PATH"
    fi
    printf '[android-quality-gates] ktlint\n'
    (cd "$gradle_dir" && ./gradlew --console=plain :app:ktlintCheck)
    printf '[android-quality-gates] Detekt\n'
    (cd "$gradle_dir" && ./gradlew --console=plain :app:detekt)
    printf '[android-quality-gates] Android Lint (Compose, Bevy and Slint Debug)\n'
    (cd "$gradle_dir" && ./gradlew --console=plain :app:androidLintGate)
}

if [[ "$SELF_TEST" == "1" ]]; then
    run_self_test
    printf '[android-quality-gates] self-test: PASS\n'
    exit 0
fi

printf '[android-quality-gates] architecture red-line guard\n'
python3 "$REDLINE_GUARD" --self-test
python3 "$REDLINE_GUARD" --mode enforce

printf '[android-quality-gates] Kotlin file budget guard\n'
python3 "$LINE_GUARD" --self-test
python3 "$LINE_GUARD" --mode enforce

if [[ "$SKIP_GRADLE" == "0" ]]; then
    run_gradle_gates
fi

printf '[android-quality-gates] OK\n'
