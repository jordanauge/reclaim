#!/bin/bash
# Code sign macOS app bundle (requires Apple Developer ID)

set -e

APP_BUNDLE="${1:-target/release/Reclaim.app}"
IDENTITY="${APPLE_DEVELOPER_ID:-Developer ID Application}"

echo "✍️  Code signing macOS app bundle"
echo "   Bundle: $APP_BUNDLE"
echo "   Identity: $IDENTITY"
echo ""

# Check if bundle exists
if [ ! -d "$APP_BUNDLE" ]; then
    echo "❌ Error: $APP_BUNDLE not found"
    exit 1
fi

# Check if identity is available
if ! security find-identity -v -p codesigning | grep -q "$IDENTITY"; then
    echo "⚠️  Warning: Code signing identity not found"
    echo "   Available identities:"
    security find-identity -v -p codesigning
    echo ""
    echo "   To sign the app, you need an Apple Developer ID certificate."
    echo "   Set APPLE_DEVELOPER_ID environment variable with your identity."
    echo ""
    echo "   For local testing without signing, you can skip this step."
    exit 0
fi

echo "🔐 Signing app bundle..."
codesign --force --deep --sign "$IDENTITY" \
    --options runtime \
    --entitlements "$(dirname "$0")/../../entitlements.plist" \
    "$APP_BUNDLE"

echo "✅ App signed successfully!"
echo ""

# Verify signature
echo "🔍 Verifying signature..."
codesign --verify --verbose "$APP_BUNDLE"
spctl --assess --verbose "$APP_BUNDLE" || true

echo ""
echo "📍 Next step: Notarize with Apple"
echo "   Run: ./installers/macos/notarize.sh"
