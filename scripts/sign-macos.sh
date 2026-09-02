#!/usr/bin/env bash
# macOS Code Signing, Hardened Runtime, Notarization, and Stapling Automation.
set -euo pipefail

TARGET_APP="${1:-dist/Infiltrator.app}"
IDENTITY="${MACOS_SIGNING_IDENTITY:-Developer ID Application: MusicFrog}"
APPLE_ID="${APPLE_ID:-}"
TEAM_ID="${APPLE_TEAM_ID:-}"
APP_PWD="${APPLE_APP_SPECIFIC_PASSWORD:-}"
ENTITLEMENTS="packaging/macos/entitlements.plist"

if [ ! -d "$TARGET_APP" ]; then
    echo "Error: Target app bundle not found: $TARGET_APP" >&2
    exit 1
fi

echo "[sign-macos] Signing app bundle with Hardened Runtime..."
codesign --force --deep --options runtime --timestamp \
    --entitlements "$ENTITLEMENTS" \
    --sign "$IDENTITY" \
    "$TARGET_APP"

echo "[sign-macos] Verifying app signature..."
codesign --verify --deep --strict --verbose=2 "$TARGET_APP"

if [ -n "$APPLE_ID" ] && [ -n "$APP_PWD" ] && [ -n "$TEAM_ID" ]; then
    echo "[sign-macos] Creating zip for Apple Notarization..."
    ZIP_PATH="/tmp/Infiltrator-notarize.zip"
    ditto -c -k --keepParent "$TARGET_APP" "$ZIP_PATH"

    echo "[sign-macos] Submitting to Apple notarytool..."
    xcrun notarytool submit "$ZIP_PATH" \
        --apple-id "$APPLE_ID" \
        --team-id "$TEAM_ID" \
        --password "$APP_PWD" \
        --wait

    rm -f "$ZIP_PATH"

    echo "[sign-macos] Stapling ticket to $TARGET_APP..."
    xcrun stapler staple "$TARGET_APP"

    echo "[sign-macos] Gatekeeper assessment..."
    spctl --assess -vv --type execute "$TARGET_APP"
    echo "[sign-macos] Apple Notarization and Stapling COMPLETE"
else
    echo "[sign-macos] Apple credentials not supplied, skipping notarization."
fi
