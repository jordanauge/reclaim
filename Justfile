# Default roots if not supplied
ROOTS := "~/repos"

# Build all crates
build:
    cargo build --workspace

# Run clippy across the workspace
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Run all tests
test:
    cargo test --workspace

# Quick scan of ROOTS with the conservative profile (dry-run)
scan:
    cargo run -p reclaim-cli -- scan {{ROOTS}} --profile conservative

# Launch the TUI
tui:
    cargo run -p reclaim-tui -- {{ROOTS}}

# Launch the GUI
gui:
    cargo run -p reclaim-gui

# Release build
release:
    cargo build --workspace --release

# Clean Rust build artifacts (meta: use reclaim to clean other things!)
clean-rust:
    cargo clean
