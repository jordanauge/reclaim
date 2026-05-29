# Roadmap

## Milestone 0.1 — Scaffold ✅

- Workspace layout: `reclaim-core`, `reclaim-cli`, `reclaim-tui`
- `Candidate` data model with scoring fields
- `Profile` TOML config (conservative / aggressive / dev)
- Target module stubs: venv, build, pip, npm, brew, docker, logs
- Strategy scoring formula (size × age × reproducibility)
- `Report` with by-kind aggregation

## Milestone 0.2 — First Working Scan

- Implement `venv` target: detect `.venv/`, `venv/`, measure size, atime/mtime
- Implement `build` target: `target/` (Rust), `build/` (CMake), `dist/`, `__pycache__`
- Implement `pip_cache` target: `~/.cache/pip`, `~/Library/Caches/pip`
- Implement `brew_cache` target (macOS): `~/Library/Caches/Homebrew`
- Wire up `scanner::scan` with `rayon` parallelism
- CLI `scan` command: table output via `comfy-table` or `tabled`

## Milestone 0.3 — CLI Polish

- `--group-by kind|path|none` flag
- `--output json|csv|table` flag
- `--min-size`, `--min-age` overrides
- Dry-run `clean` command with detailed diff
- `--apply` flag with confirmation prompt
- Progress bar during scan (`indicatif`)

## Milestone 0.4 — TUI v1

- `ratatui` app with main table, status bar, help overlay
- Faceted view: sort by score / size / age
- Group-by toggle (none → kind → path prefix → none)
- Filter panel: kind checkboxes, age slider, size threshold
- Space to toggle selection, `d` for dry-run preview
- Pagination

## Milestone 0.5 — More Targets

- `npm` target: `node_modules/`, `~/.npm`, `~/.pnpm-store`
- `docker` target: stopped containers, dangling images, unused volumes (via CLI)
- `gradle` target: `~/.gradle/caches`
- `go_cache` target: `~/Library/Caches/go-build`, `~/.cache/go-build`
- `logs` target: `*.log`, rolling log dirs older than N days

## Milestone 0.6 — Profiles & Safety

- Profile `exclude_paths` with glob matching (`glob` crate)
- Active-venv detection: check if any process has the venv's Python open (`lsof`)
- Archive action: tar.gz + optional cloud upload before delete
- Profile export/import

## Milestone 1.0 — Production Ready

- Windows support: paths, `%LOCALAPPDATA%` caches, PowerShell hooks
- Signed binaries + GitHub Releases via `cargo-dist`
- `reclaim doctor` subcommand: summarise what's found without scoring
- Config file at `~/.config/reclaim/config.toml`
- Shell completions (`clap_complete`)
- Man page generation
