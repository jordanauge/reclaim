#!/bin/bash
# Create portable Windows ZIP (standalone, auto-update enabled)

set -e

APP_NAME="Reclaim"
VERSION="${1:-0.1.0}"
ZIP_NAME="${APP_NAME}-${VERSION}-windows-portable.zip"

echo "🪟 Creating portable Windows ZIP"
echo "   Version: ${VERSION}"
echo ""

# Check if Windows binary exists (cross-compiled or built on Windows)
BINARY="target/x86_64-pc-windows-gnu/release/reclaim-gui.exe"
if [ ! -f "$BINARY" ]; then
    BINARY="target/release/reclaim-gui.exe"
    if [ ! -f "$BINARY" ]; then
        echo "❌ Error: Windows binary not found"
        echo "   Build with: cargo build --release --target x86_64-pc-windows-gnu"
        echo "   Or build on Windows directly"
        exit 1
    fi
fi

# Create portable structure
PORTABLE_DIR="target/release/windows-portable"
echo "📁 Creating portable structure..."
rm -rf "$PORTABLE_DIR"
mkdir -p "$PORTABLE_DIR"

# Copy binary
echo "📋 Copying binary..."
cp "$BINARY" "$PORTABLE_DIR/Reclaim.exe"

# Create README
cat > "$PORTABLE_DIR/README.txt" << 'EOF'
Reclaim - Portable Windows Version
===================================

This is a portable version of Reclaim.
No installation required - just run Reclaim.exe

Features:
- Fast disk space analysis
- Smart duplicate detection
- Graphical treemap view
- Auto-update enabled

To update:
- Built-in updater checks for new versions
- Or download latest from GitHub releases

For support: https://github.com/jordanauge/reclaim
EOF

# Create ZIP
echo "🗜️  Creating ZIP archive..."
cd "$PORTABLE_DIR"
zip -r "../../$ZIP_NAME" .
cd ../..

echo "✅ Portable ZIP created: target/release/$ZIP_NAME"
echo ""
echo "📍 To distribute:"
echo "   - Upload to GitHub releases"
echo "   - Users extract and run Reclaim.exe"
echo "   - Auto-update is ENABLED (portable binary)"
echo ""

# Show size
ZIP_SIZE=$(du -h "target/release/$ZIP_NAME" | cut -f1)
echo "📊 ZIP size: $ZIP_SIZE"

# Cleanup
rm -rf "$PORTABLE_DIR"
