# Contributing to Reclaim

Thank you for your interest in contributing to Reclaim! This guide will help you get started.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)

## Code of Conduct

Be respectful, inclusive, and professional. We're all here to build something useful.

## Getting Started

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Git
- Platform-specific dependencies (see [DEVELOPMENT.md](DEVELOPMENT.md))

### Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork:
   ```bash
   git clone git@github.com:YOUR_USERNAME/reclaim.git
   cd reclaim
   ```
3. Add upstream remote:
   ```bash
   git remote add upstream git@github.com:jordanauge/reclaim.git
   ```

## Development Setup

See [DEVELOPMENT.md](DEVELOPMENT.md) for detailed setup instructions.

Quick start:
```bash
# Build
cargo build

# Run GUI
cargo run --bin reclaim-gui

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy
```

## How to Contribute

### Reporting Bugs

**Before filing a bug:**
- Search existing issues
- Check if it's fixed in latest main
- Try to reproduce with minimal steps

**Bug report should include:**
- Reclaim version
- Operating system and version
- Steps to reproduce
- Expected vs actual behavior
- Screenshots if applicable
- Relevant logs from `~/.cache/reclaim/`

### Suggesting Features

**Feature requests should include:**
- Clear use case
- Why existing features don't cover it
- Proposed solution (optional)
- Mockups or examples (optional)

### Contributing Code

**Good first issues:**
- Look for `good-first-issue` label
- Documentation improvements
- Test coverage additions
- Minor bug fixes

**Areas we need help:**
- New target plugins (detect more artifact types)
- Platform-specific features
- Performance optimizations
- UI/UX improvements
- Test coverage
- Documentation

## Pull Request Process

### 1. Create a Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/bug-description
```

**Branch naming:**
- `feature/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation
- `refactor/` - Code refactoring
- `test/` - Test additions

### 2. Make Changes

- Write clean, documented code
- Follow coding standards (see below)
- Add tests for new functionality
- Update documentation

### 3. Commit Changes

**Commit message format:**
```
<type>: <short description>

<optional body>

<optional footer>
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting
- `refactor`: Code restructuring
- `test`: Test additions
- `chore`: Build/tooling

**Examples:**
```
feat: Add support for npm global caches

Detects ~/.npm/_cacache and similar locations.
Adds scoring based on cache age.

Closes #123
```

```
fix: Handle permission errors gracefully

Previously crashed when scanning restricted folders.
Now logs warning and continues.
```

### 4. Test Your Changes

```bash
# Run all tests
cargo test

# Run clippy
cargo clippy -- -D warnings

# Check formatting
cargo fmt --check

# Build release
cargo build --release

# Test on your platform
./target/release/reclaim-gui
```

### 5. Push and Create PR

```bash
git push origin your-branch-name
```

Then open a Pull Request on GitHub.

**PR description should include:**
- What changes were made
- Why (reference issue if applicable)
- How to test
- Screenshots for UI changes
- Breaking changes (if any)

### 6. Code Review

Maintainers will review your PR:
- Address feedback promptly
- Push new commits to update PR
- Discussion is welcome

### 7. Merge

Once approved:
- Maintainer will merge (usually squash merge)
- Your contribution will be in the next release!

## Coding Standards

### Rust Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` (rustfmt) for formatting
- Use `cargo clippy` for linting
- No warnings in CI

### Code Organization

**Crates:**
- `reclaim-core`: Library (scanner, cache, grouping)
- `reclaim-gui`: Native GUI application
- `reclaim-cli`: CLI (currently inactive)

**Core principles:**
- Separation of concerns
- Platform-agnostic core
- Platform-specific in bins

### Naming Conventions

- `snake_case` for functions, variables, modules
- `PascalCase` for types, traits, enums
- `SCREAMING_SNAKE_CASE` for constants
- Descriptive names (no single letters except loop counters)

### Documentation

```rust
/// Brief description (one line)
///
/// Longer explanation if needed. Use Markdown.
///
/// # Examples
///
/// ```
/// let result = function(arg);
/// ```
///
/// # Errors
///
/// Returns `Err` if...
///
/// # Panics
///
/// Panics if...
pub fn function(arg: Type) -> Result<ReturnType> {
    // Implementation
}
```

### Error Handling

- Use `Result<T, E>` for recoverable errors
- Use `anyhow::Result` for application errors
- Use `thiserror` for library errors
- Don't `unwrap()` or `expect()` in production code
- Propagate errors with `?`

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        let result = function(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_error_case() {
        let result = function(bad_input);
        assert!(result.is_err());
    }
}
```

## Testing

### Running Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_name

# With output
cargo test -- --nocapture

# Integration tests only
cargo test --test '*'

# Doc tests
cargo test --doc
```

### Test Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage

# Open report
open coverage/index.html
```

### Writing Tests

- **Unit tests**: In same file as code (`#[cfg(test)] mod tests`)
- **Integration tests**: In `tests/` directory
- **Doc tests**: In doc comments

**Test checklist:**
- ✅ Happy path
- ✅ Edge cases
- ✅ Error conditions
- ✅ Boundary values
- ✅ Platform-specific behavior

## Documentation

### Code Documentation

- Public APIs must be documented
- Use doc comments (`///`)
- Include examples
- Document errors and panics

### User Documentation

- Update README.md for user-facing changes
- Add to QUICKSTART.md if relevant
- Update CHANGELOG.md

### Technical Documentation

- Add design docs to `docs/` for major features
- Explain architecture decisions
- Include diagrams if helpful

## Specific Contribution Areas

### Adding a New Target Plugin

1. Create file in `crates/reclaim-core/src/targets/`
2. Implement detection logic
3. Define scoring criteria
4. Add tests
5. Register in `scanner.rs`
6. Update documentation

**Example:**
```rust
// crates/reclaim-core/src/targets/your_plugin.rs

use crate::candidate::{Action, Candidate, TargetKind};
use std::path::Path;

pub fn scan(root: &Path) -> Vec<Candidate> {
    // Detection logic
    vec![]
}
```

### Improving UI

1. Make changes in `crates/reclaim-gui/src/main.rs`
2. Test on multiple resolutions
3. Consider accessibility
4. Add screenshots to PR

### Platform Support

We welcome platform-specific contributions:
- macOS: Improve permissions handling
- Linux: Package formats (Flatpak, Snap, etc.)
- Windows: Installer improvements

## Release Process

Maintainers handle releases:

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Create git tag: `git tag v0.x.0`
4. Push tag: `git push origin v0.x.0`
5. GitHub Actions builds all platforms
6. Create GitHub Release
7. Announce

## Questions?

- **General**: Open a Discussion on GitHub
- **Bugs**: File an Issue
- **Security**: Email maintainer privately

## License

By contributing, you agree your code will be dual-licensed under MIT and Apache-2.0.

---

Thank you for contributing to Reclaim! 🚀
