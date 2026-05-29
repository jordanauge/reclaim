#!/bin/bash
# Create macOS app bundle for Reclaim

set -e

echo "🔨 Creating Reclaim.app bundle..."

# Build release binary
echo "📦 Building release binary..."
~/.cargo/bin/cargo build --release 2>/dev/null || echo "Using existing binary..."

# Create app bundle structure
APP_NAME="Reclaim"
BUNDLE_DIR="target/release/${APP_NAME}.app"
CONTENTS_DIR="${BUNDLE_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

echo "📁 Creating bundle directories..."
rm -rf "${BUNDLE_DIR}"
mkdir -p "${MACOS_DIR}"
mkdir -p "${RESOURCES_DIR}"

# Copy binary
echo "📋 Copying binary..."
cp target/release/reclaim-gui "${MACOS_DIR}/"

# Copy Info.plist
echo "📋 Copying Info.plist..."
cp Info.plist "${CONTENTS_DIR}/"

# Create PkgInfo
echo "📋 Creating PkgInfo..."
echo "APPL????" > "${CONTENTS_DIR}/PkgInfo"

# Make binary executable
chmod +x "${MACOS_DIR}/reclaim-gui"

echo "✅ App bundle created at: ${BUNDLE_DIR}"
echo ""

# Check if app already exists in /Applications and update it
INSTALLED_APP="/Applications/${APP_NAME}.app"
if [ -d "${INSTALLED_APP}" ]; then
    echo "📱 Found existing app in /Applications, updating binary..."
    echo "   This preserves Full Disk Access permissions!"
    
    # Just replace the binary, keeping the bundle structure and permissions
    sudo cp "${MACOS_DIR}/reclaim-gui" "${INSTALLED_APP}/Contents/MacOS/"
    sudo chmod +x "${INSTALLED_APP}/Contents/MacOS/reclaim-gui"
    
    # Update Info.plist too in case it changed
    sudo cp "${CONTENTS_DIR}/Info.plist" "${INSTALLED_APP}/Contents/"
    
    echo "✅ App updated in /Applications (permissions preserved)"
    echo ""
    echo "🚀 You can now launch the app without re-granting permissions!"
else
    echo "📍 First-time installation steps:"
    echo "1. Move to Applications: sudo mv ${BUNDLE_DIR} /Applications/"
    echo "2. Grant Full Disk Access:"
    echo "   - Open System Settings → Privacy & Security → Full Disk Access"
    echo "   - Add Reclaim.app"
    echo "   - Restart the app"
    echo ""
    echo "Next time, this script will auto-update and preserve permissions."
fi

echo ""
echo "Or run directly from: ${BUNDLE_DIR}/Contents/MacOS/reclaim-gui"
