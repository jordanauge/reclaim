# Reclaim Distribution Guide

This document explains how Reclaim is built, packaged, and distributed across platforms.

## Architecture

Reclaim uses a **smart update detection system**:
- **Standalone binaries** (AppImage, DMG, portable ZIP): Auto-update **ENABLED**
- **System packages** (apt, flatpak, snap, homebrew): Auto-update **DISABLED**

The app detects its installation method at runtime and adapts behavior accordingly.

## Platform-Specific Builds

### macOS

#### Formats
1. **DMG** (Recommended): Drag-to-Applications installer
   - Auto-update: ✅ Yes
   - Location: `/Applications/Reclaim.app`
   - Script: `installers/macos/create-dmg.sh`

2. **Homebrew Cask** (Future)
   - Auto-update: ❌ No (managed by brew)
   - Command: `brew install --cask reclaim`

#### Build Steps
```bash
# 1. Build release binary
cargo build --release --target aarch64-apple-darwin  # Apple Silicon
cargo build --release --target x86_64-apple-darwin   # Intel

# 2. Create app bundle
./create-macos-app.sh

# 3. (Optional) Code sign
./installers/macos/sign.sh

# 4. (Optional) Notarize with Apple
./installers/macos/notarize.sh

# 5. Create DMG
./installers/macos/create-dmg.sh v0.1.0
```

#### Distribution Requirements
- **Full Disk Access**: Required for complete system scanning
- **Code Signing**: Optional but recommended (Apple Developer ID)
- **Notarization**: Required to avoid Gatekeeper warnings

### Linux

#### Formats
1. **AppImage** (Recommended): Portable, standalone
   - Auto-update: ✅ Yes
   - No installation needed
   - Script: `installers/linux/create-appimage.sh`

2. **Debian Package (.deb)**: System package
   - Auto-update: ❌ No (managed by apt)
   - Script: `installers/linux/create-deb.sh`

3. **Flatpak** (Future): Sandboxed system package
   - Auto-update: ❌ No (managed by flatpak)

#### Build Steps
```bash
# AppImage (standalone)
cargo build --release
./installers/linux/create-appimage.sh v0.1.0

# Debian package
cargo build --release
./installers/linux/create-deb.sh v0.1.0
```

#### Dependencies
- `libgtk-3-dev`: GUI framework
- `libxcb-*`: X11 integration
- `fuse`: AppImage runtime

### Windows

#### Formats
1. **Portable ZIP** (Recommended): Extract and run
   - Auto-update: ✅ Yes
   - No installation needed
   - Script: `installers/windows/create-portable.sh`

2. **NSIS Installer**: Traditional setup.exe
   - Auto-update: ❌ No (has uninstaller)
   - Script: `installers/windows/installer.nsi`

#### Build Steps
```bash
# Cross-compile from macOS/Linux
cargo build --release --target x86_64-pc-windows-gnu

# Or build natively on Windows
cargo build --release

# Create portable ZIP
./installers/windows/create-portable.sh v0.1.0

# Create NSIS installer (Windows only)
makensis installers/windows/installer.nsi
```

## Automated Releases (GitHub Actions)

### Workflow
`.github/workflows/release.yml` automatically builds for all platforms on version tags.

### Trigger
```bash
# Create and push version tag
git tag v0.1.0
git push origin v0.1.0
```

### Artifacts
GitHub Actions builds and uploads:
- `Reclaim-v0.1.0-macos-silicon.dmg`
- `Reclaim-v0.1.0-macos-intel.dmg`
- `Reclaim-v0.1.0-linux-x86_64.AppImage`
- `reclaim_0.1.0_amd64.deb`
- `Reclaim-v0.1.0-windows-portable.zip`

### Release Process
1. Workflow runs on tag push
2. Builds all platforms in parallel
3. Creates draft GitHub Release
4. Uploads all artifacts
5. Review draft and publish

## Update Detection Logic

### Install Method Detection
```rust
pub fn detect_install_method() -> InstallMethod {
    // Linux
    if Path::new("/var/lib/dpkg/info/reclaim.list").exists() {
        return SystemPackage; // Debian package
    }
    if env::var("APPIMAGE").is_ok() {
        return Standalone; // AppImage
    }
    
    // macOS
    if exe_path.contains("/opt/homebrew") {
        return SystemPackage; // Homebrew
    }
    
    // Windows
    if exe_path.contains("Program Files") && uninstaller_exists {
        return SystemPackage; // NSIS installer
    }
    
    // Default
    Standalone
}
```

### Update Behavior
```rust
if install_method.can_auto_update() {
    // Check GitHub releases
    // Download new version
    // Replace binary
    // Restart app
} else {
    // Show message: "Update via system package manager"
}
```

## Distribution Checklist

### Before Release
- [ ] Update version in `Cargo.toml`
- [ ] Update `CHANGELOG.md`
- [ ] Test builds on all platforms
- [ ] Test auto-update on standalone builds
- [ ] Test system package installation

### Release Steps
1. **Create tag**: `git tag v0.1.0 && git push origin v0.1.0`
2. **Wait for CI**: GitHub Actions builds all platforms (~20 min)
3. **Review draft**: Check artifacts in draft release
4. **Edit release notes**: Add highlights, screenshots
5. **Publish release**: Make it public

### Post-Release
- [ ] Test downloads on each platform
- [ ] Verify auto-update works (standalone)
- [ ] Update documentation
- [ ] Announce on social media

## Local Testing

### Test Auto-Update Detection
```bash
# Build and run
cargo build --release
./target/release/reclaim-gui

# Check console output:
# "Detected install method: Standalone"
# "Auto-update: enabled"
```

### Test Package Installs

#### macOS DMG
```bash
./installers/macos/create-dmg.sh v0.1.0-test
open target/release/Reclaim-v0.1.0-test-macos-silicon.dmg
# Drag to Applications, run, check: "Auto-update: enabled"
```

#### Linux AppImage
```bash
./installers/linux/create-appimage.sh v0.1.0-test
chmod +x target/release/Reclaim-*.AppImage
./target/release/Reclaim-*.AppImage
# Check: "Auto-update: enabled"
```

#### Debian Package
```bash
./installers/linux/create-deb.sh v0.1.0-test
sudo dpkg -i target/release/reclaim_*.deb
reclaim
# Check: "Auto-update: disabled"
# Check: "Update via: sudo apt upgrade reclaim"
```

## Troubleshooting

### macOS: "App is damaged" error
- **Cause**: App not signed/notarized
- **Fix**: Right-click → Open, or disable Gatekeeper temporarily
- **Proper fix**: Code sign + notarize

### Linux: AppImage won't run
- **Cause**: Missing FUSE
- **Fix**: `sudo apt install fuse libfuse2`

### Windows: "Windows protected your PC"
- **Cause**: Executable not signed
- **Fix**: Click "More info" → "Run anyway"
- **Proper fix**: Code sign with certificate

### Auto-update not working
- **Check**: Install method detection
- **Debug**: Run with `RUST_LOG=debug` to see updater logs
- **Verify**: GitHub releases exist and are accessible

## Future Enhancements

### Planned
- [ ] Homebrew formula for macOS
- [ ] Flatpak manifest for Linux
- [ ] Windows MSI installer
- [ ] AUR package for Arch Linux
- [ ] Chocolatey package for Windows

### Nice to Have
- [ ] Delta updates (download only changed parts)
- [ ] Background update downloads
- [ ] Rollback on failed update
- [ ] Signed checksums for all artifacts

## Contributing

When adding new distribution formats:
1. Update `detect_install_method()` in `src/updater/mod.rs`
2. Add build script to `installers/`
3. Update `.github/workflows/release.yml`
4. Update this document
5. Test both auto-update and system package manager paths
