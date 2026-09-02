#!/usr/bin/env bash
# Authenticode signing wrapper for Linux/CI cross-signing with osslsigncode or Windows runners.
set -euo pipefail

TARGET="${1:-}"
CERT="${WINDOWS_CERT_PATH:-}"
PASS="${WINDOWS_CERT_PASSWORD:-}"

if [ -z "$TARGET" ] || [ ! -f "$TARGET" ]; then
    echo "Usage: $0 <path-to-exe-or-dll>" >&2
    exit 1
fi

TSA_SERVERS=(
    "http://timestamp.digicert.com"
    "http://timestamp.sectigo.com"
    "http://tsa.starfieldtech.com"
)

if command -v osslsigncode >/dev/null 2>&1; then
    echo "[sign-windows] Using osslsigncode to sign $TARGET"
    TMP_SIGNED="${TARGET}.signed"
    SIGNED=0
    for tsa in "${TSA_SERVERS[@]}"; do
        echo "[sign-windows] Attempting with TSA: $tsa"
        if [ -n "$PASS" ]; then
            osslsigncode sign -pkcs12 "$CERT" -pass "$PASS" -h sha256 -ts "$tsa" -in "$TARGET" -out "$TMP_SIGNED" && { SIGNED=1; break; }
        else
            osslsigncode sign -pkcs12 "$CERT" -h sha256 -ts "$tsa" -in "$TARGET" -out "$TMP_SIGNED" && { SIGNED=1; break; }
        fi
        sleep 1
    done
    if [ "$SIGNED" -eq 1 ]; then
        mv "$TMP_SIGNED" "$TARGET"
        echo "[sign-windows] Successfully signed $TARGET"
    else
        echo "[sign-windows] Failed to sign $TARGET with all TSA servers" >&2
        rm -f "$TMP_SIGNED"
        exit 1
    fi
else
    echo "[sign-windows] osslsigncode not found, skipping cross-signing."
fi
