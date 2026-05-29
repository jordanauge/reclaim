# Testing Guide

Comprehensive testing guide for Reclaim, including unit tests, integration tests, and coverage reporting.

## Table of Contents

- [Overview](#overview)
- [Running Tests](#running-tests)
- [Writing Tests](#writing-tests)
- [Test Coverage](#test-coverage)
- [CI/CD Testing](#cicd-testing)
- [Testing Best Practices](#testing-best-practices)

## Overview

Reclaim uses Rust's built-in testing framework with additional tools for coverage and benchmarking.

### Test Types

1. **Unit Tests**: Test individual functions/modules
2. **Integration Tests**: Test crate APIs
3. **Doc Tests**: Examples in documentation
4. **Benchmarks**: Performance testing

### Current Coverage

Target coverage goals:
- **Core library** (`reclaim-core`): >80%
- **Critical paths** (scanner, cache, grouping): >90%
- **GUI** (`reclaim-gui`): >50%
- **Overall**: >70%

## Running Tests

### All Tests

```bash
# Run all tests
cargo test

# With output (don't capture stdout)
cargo test -- --nocapture

# Show ignored tests
cargo test -- --ignored

# Run single-threaded (for debugging)
cargo test -- --test-threads=1
```

### Specific Tests

```bash
# By name
cargo test test_cache_load

# By pattern
cargo test cache::

# In specific crate
cargo test -p reclaim-core

# In specific module
cargo test targets::venv
```

### Test Categories

```bash
# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test '*'

# Doc tests only
cargo test --doc

# Benchmarks (don't run by default)
cargo test --benches
```

### Watch Mode

Auto-run tests on file changes:

```bash
# Install cargo-watch
cargo install cargo-watch

# Watch and test
cargo watch -x test

# Watch specific package
cargo watch -p reclaim-core -x test
```

## Writing Tests

### Unit Tests

Located in same file as code:

```rust
// In crates/reclaim-core/src/cache.rs

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_cache_creation() {
        let temp = TempDir::new().unwrap();
        let cache_path = temp.path().join("test.db");
        
        let cache = ScanCache::new(&cache_path).unwrap();
        assert!(cache_path.exists());
    }
    
    #[test]
    fn test_load_empty_cache() {
        let temp = TempDir::new().unwrap();
        let cache_path = temp.path().join("test.db");
        let cache = ScanCache::new(&cache_path).unwrap();
        
        let entries = cache.load_all_cached().unwrap();
        assert_eq!(entries.len(), 0);
    }
    
    #[test]
    #[should_panic(expected = "invalid path")]
    fn test_invalid_cache_path() {
        ScanCache::new("/invalid/path/cache.db").unwrap();
    }
}
```

### Integration Tests

Located in `tests/` directory:

```rust
// tests/scanner_integration.rs

use reclaim_core::{scanner, profile::Profile};
use std::path::PathBuf;

#[test]
fn test_scan_with_profile() {
    let roots = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))];
    let profile = Profile::default_conservative();
    
    let candidates = scanner::scan(&roots, &profile);
    
    // Should find at least target/ directory
    assert!(!candidates.is_empty());
    
    // Should have proper scoring
    for candidate in &candidates {
        assert!(candidate.score >= 0.0 && candidate.score <= 100.0);
    }
}
```

### Doc Tests

In documentation comments:

```rust
/// Loads cached entries from database.
///
/// # Examples
///
/// ```
/// use reclaim_core::cache::ScanCache;
/// use tempfile::TempDir;
///
/// let temp = TempDir::new().unwrap();
/// let cache_path = temp.path().join("cache.db");
/// let cache = ScanCache::new(&cache_path).unwrap();
/// let entries = cache.load_all_cached().unwrap();
/// assert!(entries.is_empty());
/// ```
pub fn load_all_cached(&self) -> Result<Vec<CachedEntry>> {
    // Implementation
}
```

### Test Utilities

Create helpers in `tests/common/mod.rs`:

```rust
// tests/common/mod.rs
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub fn create_test_structure() -> TempDir {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    
    // Create test directory structure
    std::fs::create_dir(root.join("venv")).unwrap();
    std::fs::write(root.join("venv/pyvenv.cfg"), "").unwrap();
    
    std::fs::create_dir(root.join("node_modules")).unwrap();
    std::fs::write(root.join("package.json"), "{}").unwrap();
    
    temp
}

// Usage in tests:
// tests/my_test.rs
mod common;

#[test]
fn test_with_structure() {
    let temp = common::create_test_structure();
    // Test code
}
```

### Property-Based Testing

Use `proptest` for property-based testing:

```toml
[dev-dependencies]
proptest = "1.0"
```

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_score_always_valid(size in 0u64..1_000_000_000, age in 0u64..1000) {
        let score = calculate_score(size, age);
        prop_assert!(score >= 0.0 && score <= 100.0);
    }
}
```

## Test Coverage

### Using Tarpaulin (Recommended)

```bash
# Install
cargo install cargo-tarpaulin

# Generate HTML report
cargo tarpaulin --out Html --output-dir coverage

# Open report
open coverage/index.html

# Generate multiple formats
cargo tarpaulin --out Html --out Lcov --output-dir coverage

# Ignore test code
cargo tarpaulin --ignore-tests

# Exclude specific files
cargo tarpaulin --exclude-files 'crates/reclaim-gui/*'
```

### Using llvm-cov

```bash
# Install
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov

# Generate report
cargo llvm-cov --html

# Open report
open target/llvm-cov/html/index.html

# Generate text summary
cargo llvm-cov --text

# Generate for specific package
cargo llvm-cov -p reclaim-core --html
```

### Coverage Configuration

Create `.tarpaulin.toml`:

```toml
[coverage]
# Ignore test code
ignore-tests = true

# Exclude files
exclude-files = [
    "crates/reclaim-gui/src/main.rs",
    "tests/*",
]

# Target minimum coverage
target-coverage = 70

# Fail if below target
fail-under = 70

# Output formats
output = ["Html", "Lcov"]

# Timeout per test
timeout = "5m"
```

### CI Coverage

Add to `.github/workflows/test.yml`:

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Install dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libgtk-3-dev
      
      - name: Run tests
        run: cargo test --all-features
      
      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin
      
      - name: Generate coverage
        run: cargo tarpaulin --out Xml
      
      - name: Upload to Codecov
        uses: codecov/codecov-action@v3
        with:
          files: ./cobertura.xml
```

### Coverage Badges

Add to README.md:

```markdown
[![Coverage](https://codecov.io/gh/jordanauge/reclaim/branch/main/graph/badge.svg)](https://codecov.io/gh/jordanauge/reclaim)
```

## CI/CD Testing

### GitHub Actions

Create `.github/workflows/test.yml`:

```yaml
name: Tests

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test Suite
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable, beta]
    runs-on: ${{ matrix.os }}
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
      
      - name: Cache cargo
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Install dependencies (Ubuntu)
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libgtk-3-dev
      
      - name: Run tests
        run: cargo test --all-features --workspace
      
      - name: Run doc tests
        run: cargo test --doc --workspace

  fmt:
    name: Formatting
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - name: Install dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libgtk-3-dev
      - run: cargo clippy --all-targets --all-features -- -D warnings
```

## Testing Best Practices

### General Principles

1. **Test behavior, not implementation**
   ```rust
   // Good
   #[test]
   fn test_cache_persists_data() {
       let cache = create_cache();
       cache.save_entry(&entry);
       
       let loaded = cache.load_entry(&entry.path).unwrap();
       assert_eq!(loaded.size, entry.size);
   }
   
   // Bad (tests implementation detail)
   #[test]
   fn test_cache_uses_sqlite() {
       assert!(cache.db_path.ends_with(".db"));
   }
   ```

2. **Use descriptive names**
   ```rust
   // Good
   #[test]
   fn test_scanner_skips_permission_denied_directories()
   
   // Bad
   #[test]
   fn test_scan()
   ```

3. **One assertion per test (when possible)**
   ```rust
   #[test]
   fn test_candidate_has_valid_size() {
       let candidate = create_test_candidate();
       assert!(candidate.size_bytes > 0);
   }
   
   #[test]
   fn test_candidate_has_valid_score() {
       let candidate = create_test_candidate();
       assert!(candidate.score >= 0.0 && candidate.score <= 100.0);
   }
   ```

4. **Clean up resources**
   ```rust
   #[test]
   fn test_with_temp_dir() {
       let temp = TempDir::new().unwrap();
       // Test code
       // TempDir automatically cleaned up on drop
   }
   ```

### Mocking

For external dependencies, use traits:

```rust
// Define trait
trait FileSystem {
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>>;
}

// Real implementation
struct RealFS;
impl FileSystem for RealFS {
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        // Real implementation
    }
}

// Mock for testing
#[cfg(test)]
struct MockFS {
    files: Vec<PathBuf>,
}
#[cfg(test)]
impl FileSystem for MockFS {
    fn read_dir(&self, _path: &Path) -> Result<Vec<PathBuf>> {
        Ok(self.files.clone())
    }
}

// Use in tests
#[test]
fn test_with_mock_fs() {
    let mock = MockFS {
        files: vec![PathBuf::from("/test/file")],
    };
    let result = process_files(&mock);
    assert!(!result.is_empty());
}
```

### Platform-Specific Tests

```rust
#[test]
#[cfg(target_os = "macos")]
fn test_macos_full_disk_access() {
    // macOS-specific test
}

#[test]
#[cfg(target_os = "linux")]
fn test_linux_appimage_detection() {
    // Linux-specific test
}

#[test]
#[cfg(not(windows))]
fn test_unix_permissions() {
    // All except Windows
}
```

### Flaky Tests

Avoid flaky tests:

```rust
// Bad - depends on timing
#[test]
fn test_async_result() {
    let result = async_operation();
    std::thread::sleep(Duration::from_millis(100));
    assert!(result.is_ready());  // Might fail!
}

// Good - wait properly
#[test]
fn test_async_result() {
    let result = async_operation();
    result.wait_until_ready();
    assert!(result.is_ready());
}
```

### Test Data

Keep test data small and focused:

```rust
// Good
#[test]
fn test_parse_config() {
    let config = r#"
        {
            "min_size": 1000,
            "max_age": 30
        }
    "#;
    let parsed = parse(config).unwrap();
    assert_eq!(parsed.min_size, 1000);
}

// Bad - too much data
#[test]
fn test_parse_config() {
    let config = include_str!("huge_config.json");
    // ...
}
```

## Benchmarking

See [DEVELOPMENT.md](DEVELOPMENT.md#profiling) for details.

Quick reference:

```bash
# Run benchmarks
cargo bench

# Specific benchmark
cargo bench scan_performance

# Compare with baseline
cargo bench -- --save-baseline main
# Make changes
cargo bench -- --baseline main
```

## Troubleshooting

### Tests Fail in CI but Pass Locally

- Check platform differences (paths, line endings)
- Verify dependencies are installed in CI
- Check for timing issues (use longer timeouts in CI)

### Coverage Reports Incomplete

- Ensure all code paths are tested
- Check for `#[cfg(test)]` guards excluding code
- Use `--all-features` flag

### Slow Tests

```bash
# Profile tests
cargo test -- --nocapture --test-threads=1 | ts

# Identify slow tests
cargo test -- --nocapture | grep "test result"
```

## Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)
- [proptest](https://github.com/proptest-rs/proptest)

---

**Coverage is important, but don't let it stop you from shipping!** 🚀
