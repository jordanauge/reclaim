#!/bin/bash
# Create macOS DMG with drag-to-Applications layout

set -e

APP_NAME="Reclaim"
VERSION="${1:-0.1.0}"
BUNDLE="target/release/${APP_NAME}.app"
DMG_NAME="${APP_NAME}-${VERSION}-macos-silicon.dmg"
TEMP_DMG="temp.dmg"
VOLUME_NAME="${APP_NAME} ${VERSION}"

echo "🍎 Creating macOS DMG for version ${VERSION}"

# Check if app bundle exists
if [ ! -d "$BUNDLE" ]; then
    echo "❌ Error: $BUNDLE not found. Run ./create-macos-app.sh first."
    exit 1
fi

echo "📁 Creating temporary DMG..."
mkdir -p target/release/dmg-staging
cp -R "$BUNDLE" target/release/dmg-staging/
ln -sf /Applications target/release/dmg-staging/Applications

# Calculate required size (bundle size + 10MB overhead)
BUNDLE_SIZE=$(du -sm "$BUNDLE" | cut -f1)
DMG_SIZE=$((BUNDLE_SIZE + 10))

echo "📦 Creating DMG image (${DMG_SIZE}MB)..."
hdiutil create -size ${DMG_SIZE}m -fs HFS+ -volname "$VOLUME_NAME" "$TEMP_DMG"

echo "🔗 Mounting temporary DMG..."
MOUNT_DIR=$(hdiutil attach "$TEMP_DMG" | grep "/Volumes" | awk '{print $3}')

echo "📋 Copying contents..."
cp -R target/release/dmg-staging/* "$MOUNT_DIR/"

echo "⏏️  Unmounting..."
hdiutil detach "$MOUNT_DIR"

echo "🗜️  Converting to compressed read-only DMG..."
hdiutil convert "$TEMP_DMG" -format UDZO -o "target/release/$DMG_NAME"
rm -f "$TEMP_DMG"

echo "🧹 Cleaning up staging..."
rm -rf target/release/dmg-staging

echo "✅ DMG created: target/release/$DMG_NAME"
echo ""
echo "📍 To distribute:"
echo "   - Upload to GitHub releases"
echo "   - Users drag Reclaim.app to Applications folder"
echo "   - Grant Full Disk Access on first launch"

# Optional: Show DMG size
DMG_SIZE_MB=$(du -h "target/release/$DMG_NAME" | cut -f1)
echo ""
echo "📊 DMG size: $DMG_SIZE_MB"
