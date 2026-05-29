# Changelog

All notable changes to Reclaim will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-05-29

### Added

- Initial release of Reclaim
- **Multi-platform support**: macOS (Intel + Apple Silicon), Linux (x86_64), Windows (x86_64)
- **Native GUI** with egui: Modern dark theme, 60 FPS rendering
- **Three view modes**:
  - Table view with smart grouping
  - Treemap view with area-proportional rectangles
  - Disk Overview with pie chart and category breakdown
- **Smart grouping**: Automatically detects duplicates, similar names, same directory
- **SQLite cache**: Instant load of top 1000 items (<1s), persistent between runs
- **Multi-phase scanning**: Cache → Verify → Full Scan → Disk Analysis
- **Plugin system**: 11+ detectors for common artifacts
  - Python venv, **pycache**
  - Node.js node_modules
  - Rust target/ directories
  - Docker build caches
  - VS Code storage and chats
  - Browser caches
  - System logs
  - Large files and archives
  - Cisco-specific logs
- **Intelligent scoring**: Combines age, size, and reproducibility
- **Permission awareness**: Graceful handling of restricted folders
- **Full Disk Access** support on macOS
- **Auto-update system**: Smart detection of install method
  - Enabled for standalone installs (DMG, AppImage, portable)
  - Disabled for system packages (apt, homebrew)
- **Disk categorization**: 6 main categories, 17 subcategories
- **Dry-run mode**: Preview changes before applying
- **JSON export**: Save scan results for analysis
- **Background threads**: Non-blocking scanning and verification
- **Genealogy explorer**: View folder hierarchy and siblings

### Distribution

- **macOS**: DMG installer with drag-to-Applications
- **Linux**: AppImage (standalone) + Debian package (.deb)
- **Windows**: Portable ZIP
- **GitHub Actions**: Automated multi-platform builds on release tags
- **Code signing scripts**: For macOS (sign.sh, notarize.sh)

### Documentation

- Comprehensive README with installation instructions
- DISTRIBUTION.md with packaging guidelines
- MACOS_PERMISSIONS.md for Full Disk Access setup
- Multiple technical design docs in docs/

[Unreleased]: https://github.com/jordanauge/reclaim/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/jordanauge/reclaim/releases/tag/v0.1.0
