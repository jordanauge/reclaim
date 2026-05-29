# Developer Guide

## Architecture

```
reclaim/
├── crates/
│   ├── reclaim-core/       # Pure library — no I/O side-effects at the API boundary
│   │   └── src/
│   │       ├── candidate.rs    # Candidate data model + human_bytes()
│   │       ├── profile.rs      # Profile/TargetConfig TOML deserialization
│   │       ├── scanner.rs      # Parallel scan dispatcher (rayon)
│   │       ├── strategy.rs     # Scoring + default action assignment
│   │       ├── report.rs       # Aggregated report + grouping
│   │       └── targets/        # One module per artifact type
│   │           ├── mod.rs      # Re-exports + shared dir_size()
│   │           ├── venv.rs
│   │           ├── build.rs
│   │           ├── docker.rs
│   │           ├── pip.rs
│   │           ├── brew.rs     # macOS only (#[cfg(target_os = "macos")])
│   │           ├── npm.rs
│   │           └── logs.rs
│   │
│   ├── reclaim-cli/        # Thin CLI wrapper — clap, table rendering, prompts
│   └── reclaim-tui/        # ratatui interactive interface
│
├── profiles/               # Built-in TOML profiles (embedded at compile time later)
├── docs/
└── Justfile
```

## Adding a New Target

1. Create `crates/reclaim-core/src/targets/<name>.rs`.
2. Implement `pub fn scan(root: &Path, profile: &Profile) -> anyhow::Result<Vec<Candidate>>`.
3. Set appropriate `TargetKind`, `reproducibility` (0.0–1.0), and initial `tags`.
4. Add `pub mod <name>;` to `targets/mod.rs`.
5. Call `targets::<name>::scan(root, profile)?` in `scanner::scan_root`.
6. Add a `[targets.<name>]` section to each profile TOML.
7. Add a `TargetKind::<Name>` variant to `candidate.rs`.

## Scoring Formula

`score = age_score × 0.4 + size_score × 0.3 + reproducibility × 0.3`

- **age_score** — 0 if younger than `profile.min_age_days`; scales to 1.0 at 365 d
- **size_score** — log-scale; ~100 MB → 0.5, ~10 GB → 1.0
- **reproducibility** — set per target type (e.g. venv = 0.95, docker image = 0.80)

Candidates with `score >= 0.7` default to `Action::Delete` and are pre-selected in
the TUI.  Candidates with `score < 0.4` default to `Action::Skip` and are shown
dimmed.

## Profile TOML Format

```toml
name        = "my-profile"
description = "Description shown in TUI profile picker"
min_age_days    = 30     # global minimum; per-target can override
min_size_bytes  = 104857600   # 100 MB
exclude_paths   = ["~/repos/active/**", "~/.venv"]

[targets.venv]
enabled     = true
min_age_days = 60          # overrides global
default_action = "delete"  # "delete" | "archive" | "skip"

[targets.build]
enabled = true

[targets.brew_cache]
enabled = true
min_age_days = 0
```

## Running Tests

```bash
cargo test --workspace
# Run only core tests
cargo test -p reclaim-core
```

## Release Build

```bash
cargo build --workspace --release
# Binaries at:
#   target/release/reclaim        (CLI)
#   target/release/reclaim-tui    (TUI)
```

## Cross-compilation (future)

Use `cargo-dist` for multi-platform release artefacts.  Windows target:
`x86_64-pc-windows-gnu` via `cross`.

## Code Style

- Follow standard `rustfmt` formatting (`cargo fmt --all`).
- All public types and functions must have doc comments.
- No `unwrap()` in library code — use `anyhow::Result` or `thiserror`.
- Target modules must not shell out (no `std::process::Command` in core).
  CLI/TUI layers handle external tool integration (docker, brew).
