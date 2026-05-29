# Reclaim Plugins & Extensions Roadmap

## Priority 1: Immediate macOS Cleanup Plugins

### 1. **Xcode Derived Data** (`xcode.rs`)
```rust
// ~/Library/Developer/Xcode/DerivedData/*
// Reproducibility: 1.0 (fully regenerated on build)
// Action: Delete or Exec: `rm -rf ~/Library/Developer/Xcode/DerivedData`
// Typical size: 20-50 GB
```
**Urgency**: HIGH — Very common on dev Macs

### 2. **iOS Device Support** (`ios_device_support.rs`)
```rust
// ~/Library/Developer/Xcode/iOS DeviceSupport/*
// Old iOS version symbols downloaded when debugging devices
// Reproducibility: 0.95 (re-downloaded if device reconnected)
// Min age: 365 days (keep recent iOS versions)
// Typical size: 5-20 GB
```

### 3. **Simulator Devices** (`simulators.rs`)
```rust
// ~/Library/Developer/CoreSimulator/Devices/*
// Old iOS/watchOS/tvOS simulator instances
// Exec: `xcrun simctl delete unavailable`
// Reproducibility: 0.98
// Typical size: 10-30 GB
```

### 4. **Homebrew Downloads** (`brew_downloads.rs`)
```rust
// ~/Library/Caches/Homebrew/downloads/*
// Downloaded .tar.gz archives after install
// Reproducibility: 1.0
// Action: Exec: `brew cleanup -s` (remove downloads)
// Typical size: 5-15 GB
```

### 5. **Docker Images & Volumes** (`docker_full.rs`)
```rust
// Current docker.rs is a stub
// Implement:
// - docker system df (get stats)
// - docker image prune -a (remove unused images)
// - docker volume prune (remove unused volumes)
// - docker builder prune (build cache)
// Typical size: 20-100 GB
```

### 6. **Trash/Bin** (`trash.rs`)
```rust
// ~/.Trash/* (macOS)
// Reproducibility: 0.0 (user data)
// Min age: 30 days (safety)
// Action: Exec: `rm -rf ~/.Trash/*`
// Typical size: Variable
```

### 7. **Application Caches** (`app_caches.rs`)
```rust
// ~/Library/Caches/* (app-specific subdirs)
// Table-driven config for known apps:
// - Slack, Chrome, Firefox, VS Code, etc.
// - Pattern: ~/Library/Caches/<BundleID>/*
// Reproducibility: 0.9
// Typical size: 10-30 GB
```

### 8. **Old Time Machine Local Snapshots** (`timemachine_local.rs`)
```rust
// tmutil listlocalsnapshots /
// tmutil deletelocalsnapshots <date>
// Reproducibility: 0.0 (backup data)
// Min age: 7 days
// Typical size: 20-100 GB
```

### 9. **Mail Downloads & Attachments** (`mail_attachments.rs`)
```rust
// ~/Library/Mail/V*/MailData/Envelope Index
// ~/Library/Mail Downloads/*
// Reproducibility: 0.5 (re-downloadable from server)
// Typical size: 5-20 GB
```

### 10. **Old Kernel Extensions** (`old_kexts.rs`)
```rust
// /Library/Extensions.old/* (created by macOS updates)
// /System/Library/Extensions.backup/*
// Reproducibility: 0.0 (system backup)
// Min age: 90 days (after stable update)
// Typical size: 1-5 GB
```

---

## Priority 2: Extended Analysis Features

### 11. **Duplicate File Finder** (plugin + new module)
```rust
// Use content hashing (SHA-256)
// Group by hash, show duplicate groups
// UI: Tree view with "Keep newest" / "Keep largest" actions
// Cache: Store hashes in ~/.cache/reclaim/file-hashes.db (SQLite)
```

### 12. **Large File Finder** (filter enhancement)
```rust
// Add "Top 100 largest files" quick view
// Ignore system files (/System, /Library)
// Sort by size, show age and last access
```

### 13. **Unused Applications** (macOS-specific)
```rust
// Scan /Applications, ~/Applications
// Check: last opened (via mdls kMDItemLastUsedDate)
// Min age: 365 days
// Action: Move to trash or uninstall (via pkgutil)
// Typical space: 10-50 GB
```

---

## Priority 3: Performance & Caching

### 14. **Persistent Cache with Merkle Tree**
```rust
// Structure:
// - ~/.cache/reclaim/scan-cache.db (SQLite)
// - Tables: paths, hashes, metadata, scan_runs
// - Merkle tree: hash(path + mtime + size) → skip if unchanged
// - Incremental scan: only check changed dirs
// 
// Benefits:
// - 10-100x faster rescans
// - Detect moved/renamed files
// - Track changes over time
```

### 15. **Background Indexing Service** (optional daemon)
```rust
// Launch agent: com.reclaim.indexer.plist
// Watches filesystem (FSEvents on macOS)
// Maintains up-to-date cache
// GUI shows real-time stats
```

---

## Priority 4: Cross-Platform Support

### 16. **Linux Targets** (extend existing modules)
```rust
// apt cache: /var/cache/apt/archives
// dnf cache: /var/cache/dnf
// flatpak cache: ~/.var/app/*/cache
// snap old revisions: snap list --all | awk '/disabled/{print $1, $3}'
```

### 17. **Windows Targets** (new modules)
```rust
// Windows.old: C:\Windows.old
// WinSxS backup: C:\Windows\WinSxS\Backup
// Temp: C:\Windows\Temp, %TEMP%
// Downloads: %USERPROFILE%\Downloads
// Recycle Bin: Shell API
```

---

## Priority 5: Advanced Features

### 18. **Disk Usage Map / Sunburst Chart**
```rust
// Generate hierarchical disk usage data
// UI: Interactive sunburst or treemap
// Click to drill down, right-click to analyze/clean
// Similar to: DaisyDisk, WinDirStat
```

### 19. **Smart Categorization**
```rust
// Classify files into:
// - Apps (executables, bundles)
// - Documents (office, PDF)
// - Media (photos, videos, music)
// - Code (repos, build artifacts)
// - System
// - Other
// 
// Use:
// - File extension
// - Magic numbers (file(1))
// - UTI (macOS)
// - MIME type
```

### 20. **Access Tracking & Archival Suggestions**
```rust
// Track last access time (atime)
// Flag files not accessed in >2 years
// Suggest:
// - Move to external drive
// - Archive to cloud (S3, B2)
// - Compress (.tar.gz, .zip)
```

### 21. **Compression Opportunities**
```rust
// Identify compressible files:
// - Large text files (.log, .json, .xml)
// - Uncompressed media (BMP → PNG, WAV → FLAC)
// - Old repos (git gc --aggressive)
// 
// Estimate space savings before compression
```

---

## Architecture Guidelines

### Plugin System Design

```rust
// traits.rs
pub trait CleanupTarget {
    fn name(&self) -> &str;
    fn scan(&self, config: &Config) -> Result<Vec<Candidate>>;
    fn platforms(&self) -> &[Platform]; // macOS, Linux, Windows
    fn requires_root(&self) -> bool;
}

// registry.rs
pub struct TargetRegistry {
    targets: Vec<Box<dyn CleanupTarget>>,
}

impl TargetRegistry {
    pub fn register(&mut self, target: Box<dyn CleanupTarget>) { ... }
    pub fn scan_all(&self, config: &Config) -> Vec<Candidate> { ... }
}
```

### Module Organization

```
crates/reclaim-core/src/
  targets/
    mod.rs           # Registry and trait
    macos/           # macOS-specific
      xcode.rs
      simulators.rs
      timemachine.rs
    linux/           # Linux-specific
      apt.rs
      flatpak.rs
    windows/         # Windows-specific
      winsxs.rs
    cross_platform/  # Works everywhere
      node_modules.rs
      build_dirs.rs
```

### Configuration Schema

```toml
# ~/.config/reclaim/config.toml

[cache]
enabled = true
path = "~/.cache/reclaim"
max_age_days = 7  # Rebuild cache weekly

[plugins]
enabled = ["xcode", "brew", "docker", "simulators"]
disabled = ["trash"]  # Too risky

[targets.xcode]
derived_data_age = 30  # days
ios_support_age = 365

[targets.docker]
prune_images = true
prune_volumes = false  # User decides
```

### Testing Strategy

```rust
#[cfg(test)]
mod tests {
    // Unit tests: Each target in isolation
    // Integration tests: Full scan pipeline
    // Mock filesystem for reproducible tests
    
    #[test]
    fn test_xcode_detection() {
        let tmp = TempDir::new();
        tmp.create("Library/Developer/Xcode/DerivedData/MyApp-xyz/");
        
        let candidates = xcode::scan(&tmp.path(), &profile);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].kind, TargetKind::XcodeDerivedData);
    }
}
```

---

## Implementation Priority

### Phase 1: Urgent macOS Cleanup (1-2 days)
- [ ] Xcode derived data
- [ ] Simulators
- [ ] Homebrew downloads
- [ ] Docker full implementation
- [ ] Application caches

### Phase 2: Analysis & UX (2-3 days)
- [ ] Duplicate finder (basic, no Merkle yet)
- [ ] Disk usage sunburst chart
- [ ] Smart categorization
- [ ] Export scan results (JSON/CSV)

### Phase 3: Performance (1-2 days)
- [ ] SQLite cache
- [ ] Merkle tree for change detection
- [ ] Incremental scan

### Phase 4: Cross-Platform (3-5 days)
- [ ] Linux targets
- [ ] Windows targets
- [ ] CI/CD for all platforms

---

## Code Quality Checklist

- ✅ **Modular**: Each target is a separate module
- ✅ **Composable**: Targets implement common trait
- ✅ **Testable**: Unit tests for each target
- ✅ **Safe**: No destructive operations without confirmation
- ✅ **Documented**: Inline docs + user guide
- ✅ **Error handling**: Result<T> everywhere, clear error messages
- ✅ **Performance**: Parallel scan with rayon
- ✅ **UI/UX**: Multiple views, real-time filtering, progress bars

---

## Next Actions

1. **Create macOS-specific target modules** (`crates/reclaim-core/src/targets/macos/`)
2. **Implement Xcode cleanup** (highest impact for devs)
3. **Add disk usage visualization** (sunburst chart with egui_plot or plotters)
4. **Build cache infrastructure** (SQLite + Merkle tree)
5. **Test on real Mac** (your machine with actual data)

**Goal**: Free up 50-100 GB on your Mac in the next week! 🚀
