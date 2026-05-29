#!/bin/bash
# Create Debian .deb package (NO auto-update - managed by apt)

set -e

APP_NAME="reclaim"
VERSION="${1:-0.1.0}"
ARCH="amd64"
DEB_NAME="${APP_NAME}_${VERSION}_${ARCH}.deb"

echo "📦 Creating Debian package"
echo "   Package: ${APP_NAME}"
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

# Create package structure
PKG_DIR="target/release/debian-pkg"
echo "📁 Creating package structure..."
rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/applications"
mkdir -p "$PKG_DIR/usr/share/doc/reclaim"

# Copy control files
echo "📋 Copying control files..."
cp installers/linux/debian/control "$PKG_DIR/DEBIAN/"
cp installers/linux/debian/postinst "$PKG_DIR/DEBIAN/"
cp installers/linux/debian/prerm "$PKG_DIR/DEBIAN/"
chmod 755 "$PKG_DIR/DEBIAN/postinst"
chmod 755 "$PKG_DIR/DEBIAN/prerm"

# Update version in control file
sed -i.bak "s/Version: .*/Version: $VERSION/" "$PKG_DIR/DEBIAN/control"
rm -f "$PKG_DIR/DEBIAN/control.bak"

# Copy binary
echo "📋 Copying binary..."
cp "$BINARY" "$PKG_DIR/usr/bin/reclaim"
chmod 755 "$PKG_DIR/usr/bin/reclaim"

# Strip binary to reduce size
strip "$PKG_DIR/usr/bin/reclaim"

# Create desktop file
cat > "$PKG_DIR/usr/share/applications/reclaim.desktop" << 'EOF'
[Desktop Entry]
Type=Application
Name=Reclaim
Comment=Modern Disk Space Analyzer
Exec=/usr/bin/reclaim
Icon=reclaim
Categories=Utility;System;
Terminal=false
EOF

# Create copyright file
cat > "$PKG_DIR/usr/share/doc/reclaim/copyright" << 'EOF'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: reclaim
Source: https://github.com/jordanauge/reclaim

Files: *
Copyright: 2026 Jordan Augé
License: MIT or Apache-2.0
EOF

# Create changelog
cat > "$PKG_DIR/usr/share/doc/reclaim/changelog" << EOF
reclaim ($VERSION) unstable; urgency=low

  * Version $VERSION release

 -- Jordan Augé <your@email.com>  $(date -R)
EOF
gzip -9 "$PKG_DIR/usr/share/doc/reclaim/changelog"

# Build package
echo "🔨 Building .deb package..."
dpkg-deb --build "$PKG_DIR" "target/release/$DEB_NAME"

echo "✅ Debian package created: target/release/$DEB_NAME"
echo ""
echo "📍 To install locally:"
echo "   sudo dpkg -i target/release/$DEB_NAME"
echo ""
echo "📍 To distribute:"
echo "   - Upload to GitHub releases"
echo "   - Auto-update is DISABLED (system package)"
echo "   - Users update via: sudo apt update && sudo apt upgrade reclaim"
echo ""

# Show package info
echo "📊 Package info:"
dpkg-deb --info "target/release/$DEB_NAME"
echo ""
echo "📊 Package size:"
du -h "target/release/$DEB_NAME"
