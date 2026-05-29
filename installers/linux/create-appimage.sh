#!/bin/bash
# Create AppImage for Linux (standalone, portable, auto-update enabled)

set -e

APP_NAME="Reclaim"
VERSION="${1:-0.1.0}"
ARCH="x86_64"
APPIMAGE_NAME="${APP_NAME}-${VERSION}-linux-${ARCH}.AppImage"

echo "🐧 Creating AppImage for Linux"
echo "   Version: ${VERSION}"
echo "   Architecture: ${ARCH}"
echo ""

# Check if binary exists
BINARY="target/release/reclaim-gui"
if [ ! -f "$BINARY" ]; then
    echo "❌ Error: $BINARY not found"
    echo "   Run: cargo build --release first"
    exit 1
fi

# Create AppDir structure
APPDIR="target/release/AppDir"
echo "📁 Creating AppDir structure..."
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"

# Copy binary
echo "📋 Copying binary..."
cp "$BINARY" "$APPDIR/usr/bin/"

# Create desktop file
echo "📄 Creating desktop entry..."
cat > "$APPDIR/usr/share/applications/reclaim.desktop" << 'EOF'
[Desktop Entry]
Type=Application
Name=Reclaim
Comment=Modern Disk Space Analyzer
Exec=reclaim-gui
Icon=reclaim
Categories=Utility;System;
Terminal=false
EOF

# Create icon (simple placeholder - replace with actual icon)
# For now, create a symlink to the app name
ln -sf reclaim.desktop "$APPDIR/usr/share/icons/hicolor/256x256/apps/reclaim.png"

# Create AppRun launcher
echo "🚀 Creating AppRun launcher..."
cat > "$APPDIR/AppRun" << 'EOF'
#!/bin/bash
# AppImage launcher

APPDIR="$(dirname "$(readlink -f "${0}")")"
export LD_LIBRARY_PATH="${APPDIR}/usr/lib:${LD_LIBRARY_PATH}"
export PATH="${APPDIR}/usr/bin:${PATH}"

# Set APPIMAGE environment variable for update detection
export APPIMAGE="${APPIMAGE:-${0}}"

exec "${APPDIR}/usr/bin/reclaim-gui" "$@"
EOF
chmod +x "$APPDIR/AppRun"

# Copy .desktop file to root
cp "$APPDIR/usr/share/applications/reclaim.desktop" "$APPDIR/"

# Download appimagetool if not present
APPIMAGETOOL="target/release/appimagetool-x86_64.AppImage"
if [ ! -f "$APPIMAGETOOL" ]; then
    echo "📥 Downloading appimagetool..."
    curl -L "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" \
        -o "$APPIMAGETOOL"
    chmod +x "$APPIMAGETOOL"
fi

# Build AppImage
echo "🔨 Building AppImage..."
ARCH=$ARCH "$APPIMAGETOOL" "$APPDIR" "target/release/$APPIMAGE_NAME"

echo "✅ AppImage created: target/release/$APPIMAGE_NAME"
echo ""
echo "📍 To distribute:"
echo "   - Upload to GitHub releases"
echo "   - Users can run directly: chmod +x $APPIMAGE_NAME && ./$APPIMAGE_NAME"
echo "   - Auto-update is ENABLED (standalone binary)"
echo ""

# Show size
APPIMAGE_SIZE=$(du -h "target/release/$APPIMAGE_NAME" | cut -f1)
echo "📊 AppImage size: $APPIMAGE_SIZE"
