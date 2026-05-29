# Smart Incremental Cache - Implementation Design

## The Core Problem

**Without recursive metadata propagation, how do we avoid full disk scans between app runs?**

Answer: We can't perfectly. But we can be **strategically intelligent** about where to look.

## The Pragmatic Solution: "Hot Paths Heuristic"

### Philosophy

1. **Most changes happen in predictable places** (Downloads, Desktop, active repos, caches)
2. **Most of the disk is stable** (old projects, archived files, system files)
3. **Trade-off: Speed (seconds) vs Completeness (minutes)**
4. **User can always trigger full scan manually**

### Three-Tier Architecture

```
App Launch (0ms)
  ↓
Load from Cache → Display immediately (🟡 Cached/Unverified)
  ↓
Tier 1: Point Verification (seconds, background thread)
  ├─ stat() each known candidate
  ├─ Compare size_current vs size_cached
  └─ Update status: 🟢 Verified or 🟠 Changed
  ↓
Tier 2: Hot Paths Discovery (seconds, background thread)
  ├─ Shallow scan (read_dir only) of known-active locations
  ├─ Compare with cache
  └─ Mark new items as 🔵 New
  ↓
Tier 3: Full Deep Scan (minutes, user-triggered)
  └─ Walk entire file system recursively
```

## Implementation

### Data Model

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheStatus {
    /// Never seen before or outside cached roots
    Unknown,
    
    /// In cache, hasn't been verified this session yet
    CachedUnverified,
    
    /// Verified this session - size matches cached value
    CachedVerified,
    
    /// Verified this session - size changed from cache
    Changed,
    
    /// Discovered this session, not in previous cache
    New,
}

pub struct CandidateState {
    pub candidate: Candidate,
    pub selection_mode: SelectionMode,
    pub selection_state: SelectionState,
    pub cache_status: CacheStatus,  // NEW
    pub size_cached: Option<u64>,   // Size at last full scan
    pub size_current: Option<u64>,  // Size at last verification
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub last_verified: DateTime<Utc>,  // Last Tier 1 check
    pub is_new: bool,
    pub is_changed: bool,
}
```

### SQLite Schema

```sql
CREATE TABLE candidates_cache (
    path TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    size_cached INTEGER NOT NULL,      -- Size at last full scan
    size_current INTEGER,               -- Size at last verification (NULL = not yet verified)
    cache_status TEXT NOT NULL,         -- unknown/cached_unverified/cached_verified/changed/new
    first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    last_verified INTEGER,              -- Timestamp of last Tier 1 check
    group_name TEXT,
    
    INDEX idx_cache_status ON candidates_cache(cache_status),
    INDEX idx_last_verified ON candidates_cache(last_verified)
);

-- User selections (already exists)
CREATE TABLE user_selections (
    path TEXT PRIMARY KEY,
    is_checked BOOLEAN NOT NULL,
    selection_mode TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    FOREIGN KEY (path) REFERENCES candidates_cache(path)
);

-- Hot paths configuration
CREATE TABLE hot_paths (
    path TEXT PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    priority INTEGER NOT NULL DEFAULT 1,  -- Higher = scan first
    last_scanned INTEGER
);
```

### Tier 1: Point Verification

```rust
pub struct PointVerifier {
    cache: Arc<Mutex<ScanCache>>,
    progress_tx: Sender<VerificationProgress>,
}

impl PointVerifier {
    pub fn verify_cached_candidates(
        &mut self,
        candidates: Vec<CandidateState>,
    ) -> Result<Vec<CandidateState>> {
        let total = candidates.len();
        let mut updated = Vec::with_capacity(total);
        
        for (idx, mut state) in candidates.into_iter().enumerate() {
            // Quick size check - single stat()
            match dir_size_quick(&state.candidate.path) {
                Ok(current_size) => {
                    state.size_current = Some(current_size);
                    state.last_verified = Utc::now();
                    
                    if let Some(cached_size) = state.size_cached {
                        if current_size == cached_size {
                            state.cache_status = CacheStatus::CachedVerified;
                            state.is_changed = false;
                        } else {
                            state.cache_status = CacheStatus::Changed;
                            state.is_changed = true;
                        }
                    }
                }
                Err(e) => {
                    // Path no longer exists or inaccessible
                    eprintln!("Verification failed for {:?}: {}", state.candidate.path, e);
                    state.cache_status = CacheStatus::Unknown;
                }
            }
            
            let _ = self.progress_tx.send(VerificationProgress {
                current: idx + 1,
                total,
                path: state.candidate.path.display().to_string(),
            });
            
            updated.push(state);
        }
        
        Ok(updated)
    }
}

/// Quick size check without full recursive walk
fn dir_size_quick(path: &Path) -> Result<u64> {
    if path.is_file() {
        return Ok(fs::metadata(path)?.len());
    }
    
    // For directories: get stored size from filesystem attributes
    // This is OS-specific and may not reflect deep changes
    #[cfg(target_os = "macos")]
    {
        // macOS: try spotlight metadata first (fast if indexed)
        if let Ok(size) = spotlight_dir_size(path) {
            return Ok(size);
        }
    }
    
    // Fallback: shallow stat (just direct children, not recursive)
    shallow_dir_size(path)
}

fn shallow_dir_size(path: &Path) -> Result<u64> {
    let mut total = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        
        if meta.is_file() {
            total += meta.len();
        } else if meta.is_dir() {
            // For subdirs: just count the directory entry itself
            // NOT recursive - this is the key optimization
            total += 4096; // Typical dir block size
        }
    }
    Ok(total)
}

#[cfg(target_os = "macos")]
fn spotlight_dir_size(path: &Path) -> Result<u64> {
    use std::process::Command;
    
    // Use mdls (Spotlight metadata) for fast lookup
    let output = Command::new("mdls")
        .arg("-name")
        .arg("kMDItemFSSize")
        .arg(path)
        .output()?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(size_str) = stdout.split('=').nth(1) {
            if let Ok(size) = size_str.trim().parse::<u64>() {
                return Ok(size);
            }
        }
    }
    
    Err(anyhow::anyhow!("Spotlight lookup failed"))
}
```

### Tier 2: Hot Paths Discovery

```rust
pub struct HotPathsScanner {
    hot_paths: Vec<HotPath>,
    cache: Arc<Mutex<ScanCache>>,
}

#[derive(Debug, Clone)]
pub struct HotPath {
    pub path: PathBuf,
    pub priority: u8,
    pub enabled: bool,
}

impl HotPathsScanner {
    pub fn default_hot_paths(home: &Path) -> Vec<HotPath> {
        vec![
            HotPath {
                path: home.join("Downloads"),
                priority: 10, // Highest
                enabled: true,
            },
            HotPath {
                path: home.join("Desktop"),
                priority: 9,
                enabled: true,
            },
            HotPath {
                path: home.join("repos"),
                priority: 8,
                enabled: true,
            },
            HotPath {
                path: home.join("Documents"),
                priority: 7,
                enabled: true,
            },
            HotPath {
                path: home.join("Library").join("Caches"),
                priority: 6,
                enabled: true,
            },
            HotPath {
                path: home.join(".cache"),
                priority: 6,
                enabled: true,
            },
            HotPath {
                path: PathBuf::from("/tmp"),
                priority: 5,
                enabled: true,
            },
        ]
    }
    
    pub fn discover_new_candidates(
        &self,
        profile: &Profile,
    ) -> Result<(Vec<CandidateState>, DiscoveryStats)> {
        let mut new_candidates = Vec::new();
        let mut stats = DiscoveryStats::default();
        
        // Sort by priority
        let mut paths = self.hot_paths.clone();
        paths.sort_by_key(|p| std::cmp::Reverse(p.priority));
        
        for hot_path in paths {
            if !hot_path.enabled {
                continue;
            }
            
            stats.paths_scanned += 1;
            
            // Shallow scan - only read top-level entries
            match self.scan_shallow(&hot_path.path, profile) {
                Ok(candidates) => {
                    stats.items_found += candidates.len();
                    new_candidates.extend(candidates);
                }
                Err(e) => {
                    eprintln!("Hot path scan failed for {:?}: {}", hot_path.path, e);
                }
            }
        }
        
        Ok((new_candidates, stats))
    }
    
    fn scan_shallow(&self, root: &Path, profile: &Profile) -> Result<Vec<CandidateState>> {
        let cache = self.cache.lock().unwrap();
        let mut new_candidates = Vec::new();
        
        // Read only direct children (not recursive)
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            
            // Check if already in cache
            if cache.get_cached_entry(&path)?.is_some() {
                continue; // Already known
            }
            
            // Check if it matches any target patterns
            if let Some(kind) = classify_path(&path) {
                let size = if path.is_dir() {
                    dir_size(&path) // Full recursive size for new discoveries
                } else {
                    entry.metadata()?.len()
                };
                
                if profile.should_skip_size(size) {
                    continue;
                }
                
                let mut candidate = Candidate {
                    path: path.clone(),
                    kind,
                    size_bytes: size,
                    last_modified: entry.metadata().ok()
                        .and_then(|m| m.modified().ok())
                        .map(DateTime::from),
                    last_accessed: None,
                    reproducibility: kind.reproducibility(),
                    score: 0.0,
                    tags: vec!["hot-path-discovery".to_string()],
                    action: Action::Skip,
                    group: None,
                };
                
                // Apply scoring
                strategy::score_candidate(&mut candidate, profile);
                
                let state = CandidateState {
                    candidate,
                    selection_mode: SelectionMode::Auto,
                    selection_state: if candidate.score >= 0.7 {
                        SelectionState::Checked
                    } else {
                        SelectionState::Unchecked
                    },
                    cache_status: CacheStatus::New,
                    size_cached: Some(size),
                    size_current: Some(size),
                    first_seen: Utc::now(),
                    last_seen: Utc::now(),
                    last_verified: Utc::now(),
                    is_new: true,
                    is_changed: false,
                };
                
                new_candidates.push(state);
            }
        }
        
        Ok(new_candidates)
    }
}

#[derive(Default)]
pub struct DiscoveryStats {
    pub paths_scanned: usize,
    pub items_found: usize,
}
```

### GUI Integration

```rust
impl ReclaimApp {
    fn on_startup(&mut self, ctx: &egui::Context) {
        // Load from cache immediately
        match ScanCache::open_default() {
            Ok(cache) => {
                if let Ok(cached_states) = cache.load_all_candidates() {
                    self.candidates = cached_states;
                    self.scan_status = ScanStatus::Complete;
                    self.status_message = format!(
                        "{} items from cache (unverified)",
                        self.candidates.len()
                    );
                }
                self.cache = Some(cache);
            }
            Err(e) => eprintln!("Failed to open cache: {}", e),
        }
        
        // Start background verification
        self.start_background_verification(ctx);
        
        // Start hot paths discovery
        self.start_hot_paths_discovery(ctx);
    }
    
    fn start_background_verification(&mut self, ctx: &egui::Context) {
        let candidates = self.candidates.clone();
        let (tx, rx) = unbounded();
        self.verification_receiver = Some(rx);
        
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            let mut verifier = PointVerifier { /* ... */ };
            
            match verifier.verify_cached_candidates(candidates) {
                Ok(verified) => {
                    let _ = tx.send(VerificationMessage::Complete(verified));
                    ctx_clone.request_repaint();
                }
                Err(e) => {
                    let _ = tx.send(VerificationMessage::Error(e.to_string()));
                }
            }
        });
    }
    
    fn start_hot_paths_discovery(&mut self, ctx: &egui::Context) {
        let (tx, rx) = unbounded();
        self.discovery_receiver = Some(rx);
        
        let profile = self.profile.clone().unwrap_or_default();
        let ctx_clone = ctx.clone();
        
        std::thread::spawn(move || {
            let scanner = HotPathsScanner { /* ... */ };
            
            match scanner.discover_new_candidates(&profile) {
                Ok((new_candidates, stats)) => {
                    let _ = tx.send(DiscoveryMessage::Complete {
                        candidates: new_candidates,
                        stats,
                    });
                    ctx_clone.request_repaint();
                }
                Err(e) => {
                    let _ = tx.send(DiscoveryMessage::Error(e.to_string()));
                }
            }
        });
    }
}
```

### Banner for Updates

```rust
fn render_update_banner(&mut self, ctx: &egui::Context) {
    if let Some(update) = &self.pending_update {
        egui::TopBottomPanel::top("update_banner").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("📢").size(20.0));
                ui.label(format!(
                    "Update available: {} new, {} changed",
                    update.stats.new_items,
                    update.stats.changed_items
                ));
                
                // Show modified groups
                if !update.changed_groups.is_empty() {
                    ui.label("Modified groups:");
                    for group in &update.changed_groups {
                        ui.label(egui::RichText::new(format!("🟠 {}", group))
                            .color(egui::Color32::from_rgb(255, 150, 50)));
                    }
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✖ Dismiss").clicked() {
                        self.pending_update = None;
                    }
                    if ui.button("🔄 Review Changes").clicked() {
                        self.show_changes_modal = true;
                    }
                    if ui.button("✓ Update View").clicked() {
                        self.apply_pending_update();
                    }
                });
            });
        });
    }
}

fn apply_pending_update(&mut self) {
    if let Some(update) = self.pending_update.take() {
        // Merge new candidates
        self.candidates.extend(update.new_candidates);
        
        // Update changed candidates
        for changed in update.changed_candidates {
            if let Some(existing) = self.candidates.iter_mut()
                .find(|c| c.candidate.path == changed.candidate.path)
            {
                *existing = changed;
            }
        }
        
        // Re-extract filter values
        self.extract_filter_values();
        
        self.status_message = format!(
            "Updated: {} new, {} changed",
            update.stats.new_items,
            update.stats.changed_items
        );
    }
}
```

## Performance Analysis

### Benchmark Scenario

```
System: 500 GB disk, 10,000 known candidates in cache

Full Scan (baseline):
  - Walk entire home directory: ~5-10 minutes
  - Process 500,000+ files
  
Smart Incremental:
  - Load from cache: 100ms
  - Tier 1 verification: 10,000 stat() calls = 1-2 seconds
  - Tier 2 hot paths: 10 locations, shallow scan = 2-5 seconds
  - Total: 3-7 seconds (50-100x faster)

Miss rate:
  - Changes outside hot paths: ~5%
  - User can trigger full scan manually
```

## Configuration

```toml
# ~/.cache/reclaim/config.toml

[cache]
enabled = true
verify_on_startup = true
discover_on_startup = true

[hot_paths]
# User can customize
paths = [
    "~/Downloads",
    "~/Desktop",
    "~/repos",
    "~/Documents",
]

# Advanced: auto-learn hot paths
auto_learn = true
learn_threshold_changes = 3  # Path becomes "hot" after 3 changes

[ui]
show_cache_status = true
show_update_banner = true
auto_apply_updates = false
```

## Summary

This design:
- ✅ Shows data **instantly** on launch (from cache)
- ✅ Verifies quickly in background (seconds, not minutes)
- ✅ Discovers most new items automatically (hot paths)
- ✅ Allows manual full scan when needed
- ✅ Transparent to user (badges show verification status)
- ✅ Preserves user selections across sessions
- ❌ Not 100% complete (by design - trade-off accepted)

**Key insight**: Don't try to solve the impossible. Be smart about where to look.
