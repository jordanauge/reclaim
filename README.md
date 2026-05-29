# Reclaim

> Modern, intelligent disk space analyzer with smart grouping and native GUI

[![CI](https://github.com/jordanauge/reclaim/workflows/Tests/badge.svg)](https://github.com/jordanauge/reclaim/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE-MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE-APACHE)

Reclaim helps you identify and clean up wasted disk space with an intelligent, cache-first approach and modern user interface.

## 🚀 Quick Start

**Get started in 2 minutes:**

1. **Download** for your platform: [Releases](https://github.com/jordanauge/reclaim/releases)
2. **Install** and grant permissions (macOS: Full Disk Access)
3. **Scan** - Click "Start Scan" button
4. **Review** - Browse results in Table or Treemap view
5. **Clean** - Select items and click "Clean Selected"

📖 **Detailed guide**: [QUICKSTART.md](QUICKSTART.md)

---

## ✨ Features

### 🚀 Performance

- **Instant Results**: SQLite cache loads top 1000 items in <1s
- **Multi-Phase Scanning**: Progressive enhancement from cache → verify → full scan
- **Background Analysis**: Non-blocking disk categorization
- **Parallel Processing**: Leverages all CPU cores via Rayon

### 🧠 Intelligence

- **Smart Grouping**: Automatically finds duplicates, similar files, and related items
- **Intelligent Scoring**: Age, size, and reproducibility combined into single metric
- **Plugin System**: Detects build artifacts, caches, logs, and more
- **Permission-Aware**: Graceful handling of restricted folders

### 🎨 Modern UI

- **Native GUI**: Built with egui - smooth 60 FPS rendering
- **Multiple Views**: Table, Treemap, and Disk Overview
- **Graphical Treemap**: Area-proportional visualization of disk usage
- **Dark Theme**: Modern design with vibrant colors and shadows
- **Interactive**: Click, hover, expand groups

### 📊 Categories

Analyzes disk space into 6 main categories + 17 subcategories:

- System (macOS/Homebrew/pip/npm)
- Media (photos/videos/audio/documents)
- Documents (Office/PDFs/text)
- Code (repos/build/deps/caches)
- Reclaimable (duplicates/logs/caches/old files)
- Other

### 🔧 Detected Artifacts

- Python venv, **pycache**
- Node.js node_modules
- Rust target/ directories
- Docker build caches
- VS Code workspace storage
- Browser caches (Chrome, Firefox, Safari)
- System logs
- Large archives and files
- Cisco-specific logs
- And more...

## 📥 Installation

### macOS

**Apple Silicon (M1/M2/M3)**:

```bash
# Download from releases
curl -LO https://github.com/jordanauge/reclaim/releases/latest/download/Reclaim-macos-silicon.dmg
open Reclaim-macos-silicon.dmg
# Drag to Applications
```

**Intel**:

```bash
curl -LO https://github.com/jordanauge/reclaim/releases/latest/download/Reclaim-macos-intel.dmg
open Reclaim-macos-intel.dmg
```

**First Launch**:

1. Open Reclaim.app
2. System Settings → Privacy & Security → Full Disk Access
3. Add Reclaim.app
4. Restart app

**Auto-update**: ✅ Enabled

### Linux

**AppImage** (Recommended):

```bash
# Download
curl -LO https://github.com/jordanauge/reclaim/releases/latest/download/Reclaim-linux-x86_64.AppImage

# Run
chmod +x Reclaim-linux-x86_64.AppImage
./Reclaim-linux-x86_64.AppImage
```

**Debian/Ubuntu**:

```bash
# Download
curl -LO https://github.com/jordanauge/reclaim/releases/latest/download/reclaim_amd64.deb

# Install
sudo dpkg -i reclaim_amd64.deb

# Run
reclaim
```

**Auto-update**: ✅ AppImage | ❌ .deb (use `sudo apt upgrade reclaim`)

### Windows

**Portable** (Recommended):

```bash
# Download
curl -LO https://github.com/jordanauge/reclaim/releases/latest/download/Reclaim-windows-portable.zip

# Extract and run
unzip Reclaim-windows-portable.zip
.\Reclaim.exe
```

**Auto-update**: ✅ Enabled

## 🛠️ Building from Source

### Prerequisites

- Rust 1.70+ (`rustup`)
- Platform-specific dependencies:
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev`
  - **Windows**: Visual Studio Build Tools

### Build

```bash
# Clone
git clone https://github.com/jordanauge/reclaim.git
cd reclaim

# Build
cargo build --release

# Run
./target/release/reclaim-gui
```

### macOS App Bundle

```bash
# Create .app
./create-macos-app.sh

# Install to Applications
sudo mv target/release/Reclaim.app /Applications/

# Create DMG
./installers/macos/create-dmg.sh v0.1.0
```

### Linux Packages

```bash
# AppImage
./installers/linux/create-appimage.sh v0.1.0

# Debian package
./installers/linux/create-deb.sh v0.1.0
```

## 📖 Usage

### Basic Workflow

1. **Launch** Reclaim
2. **Scan** - Click "Start Scan" (or auto-scans if configured)
3. **Review** - Browse grouped results in Table or Treemap view
4. **Select** - Check items to remove
5. **Clean** - Click "Clean Selected" (dry-run first)

### Views

#### Table View (Default)

- **Grouped**: Smart grouping of related files
- **Expand**: Click groups to see individual items
- **Explore**: 🔍 button opens genealogy window
- **Sort**: By size, score, age, kind

#### Treemap View

- **Visual**: Area-proportional rectangles
- **Colors**: Different for each group type (duplicates, similar names, etc.)
- **Interactive**: Hover for details, click to drill down

#### Disk Overview

- **Categories**: Full disk breakdown by category
- **Pie Chart**: Visual representation
- **Drill Down**: Click categories to explore

### Filters

- **Type**: Filter by artifact kind (venv, node_modules, etc.)
- **Size**: Min/max size range
- **Age**: Min/max age in days
- **Score**: Minimum cleanup score

### Actions

- **Preview**: Dry-run to see what would be removed
- **Clean**: Actually remove selected items
- **Export**: Save results to JSON

## 🔄 Updates

Reclaim uses **smart update detection**:

### Standalone Installs (DMG, AppImage, Portable)

- **Auto-update**: ✅ Enabled
- Built-in updater checks GitHub releases
- One-click download and install

### System Packages (Debian, Homebrew)

- **Auto-update**: ❌ Disabled
- Use system package manager:
  - Debian: `sudo apt update && sudo apt upgrade reclaim`
  - Homebrew: `brew upgrade reclaim`

The app automatically detects how it was installed and adapts.

## 🏗️ Architecture

### Crates

- **reclaim-core**: Library with scanning logic, cache, grouping
- **reclaim-cli**: Terminal UI (not currently active)
- **reclaim-gui**: Native egui GUI (main interface)

### Key Components

- **Scanner**: Multi-threaded filesystem traversal
- **Cache**: SQLite persistent storage (~/.cache/reclaim/)
- **Grouping**: Algorithm to find duplicates and similar files
- **Disk Analyzer**: Full disk categorization system
- **Updater**: Smart install detection and GitHub releases

### Technologies

- **GUI**: egui + eframe (native OpenGL rendering)
- **Async**: crossbeam-channel for thread communication
- **Parallelism**: rayon for parallel scanning
- **Storage**: rusqlite for caching
- **Serialization**: serde for profiles and exports

## 👨‍💻 For Developers

### Documentation

- **[QUICKSTART.md](QUICKSTART.md)** - Get started in 5 minutes
- **[DEVELOPMENT.md](DEVELOPMENT.md)** - Development setup and architecture
- **[CONTRIBUTING.md](CONTRIBUTING.md)** - Contribution guidelines
- **[TESTING.md](TESTING.md)** - Testing and coverage guide
- **[DISTRIBUTION.md](DISTRIBUTION.md)** - Build and release process

### Quick Development Setup

```bash
# Clone
git clone https://github.com/jordanauge/reclaim.git
cd reclaim

# Build
cargo build

# Run GUI
cargo run --bin reclaim-gui

# Run tests
cargo test

# Generate coverage
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

### Project Structure

```
reclaim/
├── crates/
│   ├── reclaim-core/      # Core library
│   ├── reclaim-gui/       # Native GUI
│   └── reclaim-cli/       # CLI (inactive)
├── installers/            # Platform-specific installers
├── docs/                  # Technical documentation
└── .github/workflows/     # CI/CD
```

### Running Tests & Coverage

```bash
# All tests
cargo test

# With coverage
cargo tarpaulin --out Html --output-dir coverage
open coverage/index.html

# CI locally (Act)
act -j test
```

## 🤝 Contributing

Contributions welcome! Please:

1. Fork the repo
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a PR

See [DISTRIBUTION.md](DISTRIBUTION.md) for packaging guidelines.

## 📝 License

Dual-licensed under:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

Choose whichever works for you.

## 🙏 Credits

Built with:

- [egui](https://github.com/emilk/egui) - Immediate mode GUI
- [rayon](https://github.com/rayon-rs/rayon) - Parallel processing
- [rusqlite](https://github.com/rusqlite/rusqlite) - SQLite bindings
- [walkdir](https://github.com/BurntSushi/walkdir) - Directory traversal

## 📊 Project Status

**Current Version**: 0.1.0 (Private Beta)

**Platforms**:

- ✅ macOS (Intel + Apple Silicon)
- ✅ Linux (x86_64)
- ✅ Windows (x86_64)

**Distribution**:

- ✅ DMG (macOS)
- ✅ AppImage (Linux)
- ✅ .deb (Debian/Ubuntu)
- ✅ Portable ZIP (Windows)
- 🚧 Homebrew (planned)
- 🚧 Flatpak (planned)
- 🚧 Windows Installer (planned)

## 🐛 Known Issues

- macOS: Requires Full Disk Access for complete scanning
- Linux: AppImage requires FUSE (`sudo apt install fuse libfuse2`)
- Windows: May show SmartScreen warning (not signed yet)

## 📬 Contact

- GitHub: [@jordanauge](https://github.com/jordanauge)
- Issues: [GitHub Issues](https://github.com/jordanauge/reclaim/issues)

---

**Made with 🦀 Rust**
