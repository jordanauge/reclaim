# Development Guide

This guide covers setting up Reclaim for development, architecture overview, and development workflows.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Platform Setup](#platform-setup)
- [Building](#building)
- [Architecture](#architecture)
- [Development Workflow](#development-workflow)
- [Testing](#testing)
- [Debugging](#debugging)
- [Profiling](#profiling)

## Prerequisites

### Required

- **Rust**: 1.70 or later
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- **Git**: Version control

### Platform-Specific

#### macOS
```bash
# Xcode Command Line Tools
xcode-select --install
```

#### Linux (Ubuntu/Debian)
```bash
sudo apt update
sudo apt install build-essential pkg-config libssl-dev \
    libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev \
    libxcb-xfixes0-dev libxkbcommon-dev libfontconfig-dev
```

#### Linux (Fedora)
```bash
sudo dnf install gcc pkg-config openssl-devel \
    gtk3-devel libxcb-devel fontconfig-devel
```

#### Windows
- Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/)
- Or use [rustup](https://rustup.rs/) which handles MSVC automatically

### Optional Tools

```bash
# Code coverage
cargo install cargo-tarpaulin

# Benchmarking
cargo install cargo-criterion

# Flamegraph profiling
cargo install flamegraph

# Watch for changes
cargo install cargo-watch
```

## Platform Setup

### Clone Repository

```bash
git clone git@github.com:jordanauge/reclaim.git
cd reclaim
```

### Install Dependencies

The project uses workspace dependencies defined in root `Cargo.toml`:

```bash
cargo fetch  # Download all dependencies
```

## Building

### Debug Build

```bash
# Build all crates
cargo build

# Build specific crate
cargo build -p reclaim-gui
cargo build -p reclaim-core
```

### Release Build

```bash
# Optimized build
cargo build --release

# Release binary location
ls -lh target/release/reclaim-gui
```

### Run Without Building

```bash
# GUI (debug)
cargo run --bin reclaim-gui

# GUI (release)
cargo run --release --bin reclaim-gui
```

### Platform-Specific Builds

#### macOS App Bundle

```bash
# Create .app bundle
./create-macos-app.sh

# Install to Applications
sudo mv target/release/Reclaim.app /Applications/

# Or run directly
open target/release/Reclaim.app
```

#### Linux AppImage

```bash
# Build AppImage
./installers/linux/create-appimage.sh v0.1.0-dev

# Run
./target/release/Reclaim-*-linux-*.AppImage
```

#### Cross-Compilation

```bash
# Add target
rustup target add x86_64-pc-windows-gnu

# Build for Windows from macOS/Linux
cargo build --release --target x86_64-pc-windows-gnu
```

## Architecture

### Crate Structure

```
reclaim/
├── crates/
│   ├── reclaim-core/     # Core library
│   │   ├── cache.rs      # SQLite persistent cache
│   │   ├── scanner.rs    # Multi-threaded scanning
│   │   ├── grouping.rs   # Smart grouping algorithm
│   │   ├── targets/      # Plugin system
│   │   └── ...
│   ├── reclaim-gui/      # Native GUI (egui)
│   │   ├── main.rs       # Application logic
│   │   └── updater/      # Update detection
│   ├── reclaim-cli/      # CLI (inactive)
│   └── reclaim-tui/      # TUI (inactive)
```

### Core Components

#### 1. Scanner (`reclaim-core/scanner.rs`)

Multi-threaded filesystem traversal using `rayon`:

```rust
pub fn scan(roots: &[PathBuf], profile: &Profile) -> Vec<Candidate> {
    // Parallel scan with plugin system
}
```

**Plugins**: Each target type has a plugin in `targets/`:
- `venv.rs` - Python virtual environments
- `npm.rs` - Node.js node_modules
- `build.rs` - Build directories (Rust, C++, Java)
- `docker.rs` - Docker caches
- etc.

#### 2. Cache (`reclaim-core/cache.rs`)

SQLite-based persistent cache at `~/.cache/reclaim/scan-cache.db`:

```sql
CREATE TABLE cached_entries (
    path TEXT PRIMARY KEY,
    size_bytes INTEGER,
    kind TEXT,
    score REAL,
    metadata TEXT,  -- JSON
    last_verified INTEGER,  -- Unix timestamp
    hash TEXT  -- Metadata hash for change detection
);
```

**Cache workflow:**
1. **Load**: Read cached entries (instant)
2. **Verify**: Check if still valid (1-5s)
3. **Scan**: Find new items (10-60s)
4. **Merge**: Combine cached + new

#### 3. Grouping (`reclaim-core/grouping.rs`)

Smart grouping algorithm:

```rust
pub fn group_candidates(candidates: &[Candidate]) -> Vec<CandidateGroup> {
    // 1. Find duplicates (same name + size)
    // 2. Find similar names (pattern extraction)
    // 3. Find common ancestors
    // 4. Singles (filtered out in UI)
}
```

#### 4. Disk Analyzer (`reclaim-core/disk_analyzer.rs`)

Categorizes entire disk:

```rust
pub enum DiskCategory {
    System,     // OS, brew, pip, npm
    Media,      // Photos, videos, audio
    Documents,  // Office, PDFs
    Code,       // Repos, build, deps
    Reclaimable,// Duplicates, caches, logs
    Other,      // Everything else
}
```

#### 5. GUI (`reclaim-gui/main.rs`)

Native GUI using [egui](https://github.com/emilk/egui):

```rust
struct ReclaimApp {
    // State
    candidates: Vec<CandidateState>,
    scan_status: ScanStatus,
    
    // Channels for background work
    scan_receiver: Option<Receiver<ScanMessage>>,
    verification_receiver: Option<Receiver<VerificationMessage>>,
    
    // UI state
    view_mode: ViewMode,
    groups: Vec<CandidateGroup>,
    // ...
}

impl eframe::App for ReclaimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Render UI
    }
}
```

### Data Flow

```
User clicks "Scan"
  ↓
GUI spawns background thread
  ↓
Scanner loads cache (instant)
  ↓
GUI displays cached results
  ↓
Scanner verifies cached items (1-5s)
  ↓
GUI updates with verified items
  ↓
Scanner finds new items (10-60s)
  ↓
GUI updates with new items
  ↓
Scanner analyzes disk (optional, 30-120s)
  ↓
GUI shows disk overview
```

### Threading Model

- **Main thread**: GUI rendering (egui)
- **Scan thread**: Filesystem traversal
- **Verify thread**: Cache verification
- **Discover thread**: Hot paths discovery
- **Disk analysis thread**: Full disk categorization

**Communication**: `crossbeam-channel` for thread messages

## Development Workflow

### Watch Mode

Automatically rebuild on changes:

```bash
# Watch and run GUI
cargo watch -x 'run --bin reclaim-gui'

# Watch and test
cargo watch -x test
```

### Incremental Development

```bash
# 1. Make changes
vim crates/reclaim-core/src/targets/new_plugin.rs

# 2. Build (incremental)
cargo build

# 3. Test
cargo test targets::new_plugin

# 4. Run
cargo run --bin reclaim-gui
```

### Hot Reload (GUI)

egui doesn't support true hot reload, but you can:

1. Keep GUI running
2. Make code changes
3. Rebuild: `cargo build`
4. Restart GUI: `Cmd+Q` then re-run

### Adding a New Plugin

1. **Create plugin file:**
   ```bash
   touch crates/reclaim-core/src/targets/my_plugin.rs
   ```

2. **Implement scanner:**
   ```rust
   use crate::candidate::{Action, Candidate, TargetKind};
   use std::path::Path;
   
   pub fn scan(root: &Path) -> Vec<Candidate> {
       let mut candidates = vec![];
       // Detection logic
       candidates
   }
   ```

3. **Add to `targets/mod.rs`:**
   ```rust
   pub mod my_plugin;
   ```

4. **Register in `scanner.rs`:**
   ```rust
   scan_target!(root, "MyPlugin", targets::my_plugin::scan);
   ```

5. **Test:**
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_my_plugin() {
           let root = Path::new("/test/path");
           let results = scan(root);
           assert!(!results.is_empty());
       }
   }
   ```

## Testing

### Unit Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Run in specific crate
cargo test -p reclaim-core
```

### Integration Tests

```bash
# Run integration tests only
cargo test --test '*'
```

### Doc Tests

```bash
# Run doc tests
cargo test --doc
```

### Coverage

```bash
# Generate HTML coverage report
cargo tarpaulin --out Html --output-dir coverage

# Open in browser
open coverage/index.html
```

**Coverage goals:**
- Core library: >80%
- Critical paths: >90%
- GUI: >50% (harder to test)

## Debugging

### Logging

Add to `Cargo.toml` (already included):
```toml
[dependencies]
env_logger = "0.11"
log = "0.4"
```

Enable logging:
```bash
RUST_LOG=debug cargo run --bin reclaim-gui
RUST_LOG=reclaim_core=trace cargo run
```

In code:
```rust
use log::{debug, info, warn, error};

info!("Starting scan");
debug!("Found {} candidates", count);
warn!("Permission denied: {}", path.display());
error!("Failed to open cache: {}", err);
```

### Debugger

#### VS Code

Create `.vscode/launch.json`:
```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug GUI",
      "cargo": {
        "args": ["build", "--bin=reclaim-gui"]
      },
      "args": [],
      "cwd": "${workspaceFolder}"
    }
  ]
}
```

#### CLI

```bash
# Install rust-lldb
rustup component add lldb-preview

# Debug
rust-lldb target/debug/reclaim-gui
(lldb) run
(lldb) breakpoint set -n main
(lldb) continue
```

### Memory Profiling

```bash
# macOS
cargo install cargo-instruments
cargo instruments --template Allocations

# Linux
valgrind --tool=massif ./target/release/reclaim-gui
```

## Profiling

### CPU Profiling

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --bin reclaim-gui

# Open flamegraph.svg in browser
```

### Benchmarking

```bash
# Run benchmarks
cargo bench

# Specific benchmark
cargo bench scan_performance
```

Create benchmark in `benches/`:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn scan_benchmark(c: &mut Criterion) {
    c.bench_function("scan_home", |b| {
        b.iter(|| {
            // Benchmark code
            black_box(reclaim_core::scanner::scan(...))
        });
    });
}

criterion_group!(benches, scan_benchmark);
criterion_main!(benches);
```

## Common Issues

### Build Errors

**Error: `cannot find -lgtk-3`**
```bash
# Linux: Install GTK3
sudo apt install libgtk-3-dev
```

**Error: `linker 'cc' not found`**
```bash
# Install build tools
# macOS: xcode-select --install
# Linux: sudo apt install build-essential
```

### Runtime Issues

**GUI won't start:**
- Check GPU drivers (egui needs OpenGL)
- Try software rendering: `LIBGL_ALWAYS_SOFTWARE=1 ./reclaim-gui`

**Slow scans:**
- Check disk I/O (spinning HDD vs SSD)
- Reduce scan scope
- Check for permission errors

**Cache not working:**
- Check `~/.cache/reclaim/` permissions
- Delete cache and rescan: `rm -rf ~/.cache/reclaim/`

## Tools & Scripts

### Justfile

Use `just` for common tasks:

```bash
# Install just
cargo install just

# List recipes
just --list

# Build
just build

# Test
just test

# Format
just fmt

# Lint
just lint
```

### Pre-commit Hooks

```bash
# Install
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
set -e
cargo fmt --check
cargo clippy -- -D warnings
cargo test
EOF
chmod +x .git/hooks/pre-commit
```

## Documentation

### Generate Docs

```bash
# Build documentation
cargo doc --no-deps

# Open in browser
cargo doc --no-deps --open

# Include private items
cargo doc --no-deps --document-private-items
```

### Update Docs

- Code docs: Update `///` comments
- User docs: Update README.md, QUICKSTART.md
- Technical docs: Update files in `docs/`

## Release Workflow

See [DISTRIBUTION.md](DISTRIBUTION.md) for full release process.

Quick reference:
```bash
# 1. Update version
vim Cargo.toml

# 2. Update changelog
vim CHANGELOG.md

# 3. Commit
git commit -am "chore: Bump version to 0.2.0"

# 4. Tag
git tag v0.2.0

# 5. Push
git push origin main --tags

# GitHub Actions handles the rest
```

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [egui Documentation](https://docs.rs/egui/)
- [rayon Documentation](https://docs.rs/rayon/)
- [rusqlite Documentation](https://docs.rs/rusqlite/)

## Questions?

Open an issue or discussion on GitHub!

---

Happy coding! 🦀
