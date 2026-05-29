#!/bin/bash
# Notarize macOS app with Apple (required for macOS Gatekeeper)

set -e

APP_BUNDLE="${1:-target/release/Reclaim.app}"
APPLE_ID="${APPLE_ID:-your@email.com}"
TEAM_ID="${APPLE_TEAM_ID:-YOURTEAMID}"
APP_SPECIFIC_PASSWORD="${APPLE_APP_PASSWORD:-@keychain:AC_PASSWORD}"

echo "📝 Notarizing macOS app with Apple"
echo "   Bundle: $APP_BUNDLE"
echo "   Apple ID: $APPLE_ID"
echo "   Team ID: $TEAM_ID"
echo ""

# Check if bundle exists
if [ ! -d "$APP_BUNDLE" ]; then
    echo "❌ Error: $APP_BUNDLE not found"
    exit 1
fi

# Check if app is signed
if ! codesign --verify "$APP_BUNDLE" 2>/dev/null; then
    echo "❌ Error: App must be signed before notarization"
    echo "   Run: ./installers/macos/sign.sh first"
    exit 1
fi

# Create ZIP for notarization (DMG can also be used)
ZIP_FILE="target/release/Reclaim.zip"
echo "📦 Creating ZIP archive..."
ditto -c -k --keepParent "$APP_BUNDLE" "$ZIP_FILE"

echo "📤 Uploading to Apple for notarization..."
echo "   This may take several minutes..."

# Submit for notarization
xcrun notarytool submit "$ZIP_FILE" \
    --apple-id "$APPLE_ID" \
    --team-id "$TEAM_ID" \
    --password "$APP_SPECIFIC_PASSWORD" \
    --wait

# Check notarization status
echo ""
echo "🔍 Checking notarization status..."
xcrun notarytool info <REQUEST_ID> \
    --apple-id "$APPLE_ID" \
    --team-id "$TEAM_ID" \
    --password "$APP_SPECIFIC_PASSWORD"

# Staple the notarization ticket
echo ""
echo "📌 Stapling notarization ticket to app..."
xcrun stapler staple "$APP_BUNDLE"

echo "✅ Notarization complete!"
echo ""
echo "📍 Your app is now:"
echo "   - Code signed with Developer ID"
echo "   - Notarized by Apple"
echo "   - Ready for distribution"
echo ""
echo "   Users can now run it without Gatekeeper warnings."

# Cleanup
rm -f "$ZIP_FILE"
