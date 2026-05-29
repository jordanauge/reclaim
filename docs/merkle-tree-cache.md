# Merkle Tree Cache Design for Reclaim

## Problem Statement

File system `mtime` on directories is unreliable for detecting deep changes:
- ✅ Changes when direct children are added/removed
- ❌ Doesn't change when files in subdirectories are modified
- Result: Can't trust directory mtime for incremental scanning

## Solution: Merkle Tree with Content Hashing

### Architecture

```
Root: node_modules/
├─ Hash: abc123 (computed from children)
├─ Children:
   ├─ express/
   │  ├─ Hash: def456
   │  ├─ Size: 12MB
   │  ├─ Children: [lib/, package.json, ...]
   │  └─ Last verified: 2026-05-27 10:30
   ├─ react/
   │  ├─ Hash: ghi789
   │  ├─ Size: 8MB
   │  └─ ...
```

### Hash Computation

```rust
fn compute_merkle_hash(path: &Path) -> Result<String> {
    let mut hasher = seahash::SeaHasher::new();
    
    // For files: hash content (or mtime+size for speed)
    if path.is_file() {
        let meta = fs::metadata(path)?;
        hasher.write_u64(meta.len());
        hasher.write_i64(meta.modified()?.timestamp());
        return Ok(format!("{:016x}", hasher.finish()));
    }
    
    // For directories: hash all children recursively
    let mut entries: Vec<_> = fs::read_dir(path)?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|e| e.path());
    
    for entry in entries {
        let name = entry.file_name();
        hasher.write(name.as_bytes());
        
        let child_hash = compute_merkle_hash(&entry.path())?;
        hasher.write(child_hash.as_bytes());
    }
    
    Ok(format!("{:016x}", hasher.finish()))
}
```

### Cache States

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CacheStatus {
    /// First time seeing this item
    Unknown,
    
    /// In cache, hash not yet re-verified (from previous scan)
    CachedUnverified,
    
    /// In cache, hash verified = content unchanged
    CachedVerified,
    
    /// In cache, but hash changed = content modified
    Changed,
    
    /// Not in previous cache = newly discovered
    New,
}
```

### SQLite Schema Update

```sql
CREATE TABLE merkle_cache (
    path TEXT PRIMARY KEY,
    merkle_hash TEXT NOT NULL,         -- Hash of content
    parent_path TEXT,                  -- For tree structure
    size_bytes INTEGER NOT NULL,
    file_count INTEGER,                -- For directories
    last_verified INTEGER NOT NULL,    -- Timestamp
    cache_status TEXT NOT NULL,        -- unknown/cached_unverified/cached_verified/changed/new
    
    FOREIGN KEY (parent_path) REFERENCES merkle_cache(path)
);

CREATE INDEX idx_merkle_hash ON merkle_cache(merkle_hash);
CREATE INDEX idx_last_verified ON merkle_cache(last_verified);
CREATE INDEX idx_parent_path ON merkle_cache(parent_path);
```

### Incremental Scan Algorithm

```rust
pub fn incremental_scan(
    cache: &mut MerkleCache,
    roots: &[PathBuf],
    profile: &Profile,
) -> Result<ScanResult> {
    let mut new_items = Vec::new();
    let mut changed_items = Vec::new();
    let mut verified_items = Vec::new();
    
    for root in roots {
        // Step 1: Compute current hash
        let current_hash = compute_merkle_hash(root)?;
        
        // Step 2: Check cache
        let cached = cache.get_entry(root)?;
        
        match cached {
            None => {
                // Never seen before -> full scan
                let candidates = full_scan(root, profile)?;
                new_items.extend(candidates);
            }
            Some(cached_entry) if cached_entry.merkle_hash == current_hash => {
                // Hash matches -> no changes, mark as verified
                cache.mark_verified(root)?;
                let candidates = cache.get_cached_candidates(root)?;
                verified_items.extend(candidates);
            }
            Some(_cached_entry) => {
                // Hash differs -> something changed, rescan
                let candidates = full_scan(root, profile)?;
                changed_items.extend(candidates);
            }
        }
    }
    
    Ok(ScanResult {
        new: new_items,
        changed: changed_items,
        verified: verified_items,
    })
}
```

## UI Display with Cache Status

### Badges in Table View

```
┌─────────────────────────────────────────────────────────┐
│ Status │ Badge │ Size   │ Path                          │
├────────┼───────┼────────┼───────────────────────────────┤
│ [✓]    │ 🟢 V  │ 12 MB  │ .venv                        │  ← Cached + Verified
│ [✓]    │ 🟡 C  │ 8 MB   │ node_modules/express         │  ← Cached + Unverified
│ [ ]    │ 🔵 N  │ 5 MB   │ build/                       │  ← New
│ [✓]    │ 🟠 M  │ 10 MB  │ target/                      │  ← Changed (Modified)
│ [ ]    │ ⚪ ?  │ 2 MB   │ .cache/                      │  ← Unknown
└─────────────────────────────────────────────────────────┘

Legend:
🟢 V = Verified (hash confirmed unchanged)
🟡 C = Cached (not yet re-verified this session)
🔵 N = New (first time detected)
🟠 M = Modified (hash changed)
⚪ ? = Unknown (no cache yet)
```

### Enhanced CandidateState

```rust
pub struct CandidateState {
    pub candidate: Candidate,
    pub selection_mode: SelectionMode,
    pub selection_state: SelectionState,
    pub cache_status: CacheStatus,      // NEW
    pub merkle_hash: Option<String>,    // NEW
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub last_verified: DateTime<Utc>,   // NEW
}
```

### Color Coding in UI

```rust
fn cache_status_color(status: CacheStatus) -> egui::Color32 {
    match status {
        CacheStatus::CachedVerified => egui::Color32::from_rgb(100, 200, 100), // Green
        CacheStatus::CachedUnverified => egui::Color32::from_rgb(200, 200, 100), // Yellow
        CacheStatus::New => egui::Color32::from_rgb(100, 150, 255), // Blue
        CacheStatus::Changed => egui::Color32::from_rgb(255, 150, 50), // Orange
        CacheStatus::Unknown => egui::Color32::from_rgb(150, 150, 150), // Gray
    }
}

fn cache_status_badge(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::CachedVerified => "🟢 V",
        CacheStatus::CachedUnverified => "🟡 C",
        CacheStatus::New => "🔵 N",
        CacheStatus::Changed => "🟠 M",
        CacheStatus::Unknown => "⚪ ?",
    }
}
```

## Performance Optimization

### Fast Path: Hash-Only Check

```rust
// Phase 1: Quick hash verification (no file reads)
pub fn quick_verify(cache: &MerkleCache, path: &Path) -> Result<bool> {
    // Only check top-level hash
    let current = compute_merkle_hash(path)?;
    let cached = cache.get_hash(path)?;
    Ok(cached.map(|h| h == current).unwrap_or(false))
}
```

### Lazy Verification

```rust
// Only verify on-demand when viewing or before actions
impl ReclaimApp {
    fn lazy_verify_on_view(&mut self, indices: &[usize]) {
        for &idx in indices {
            if let Some(state) = self.candidates.get_mut(idx) {
                if state.cache_status == CacheStatus::CachedUnverified {
                    // Trigger background verification
                    self.verify_queue.push(state.candidate.path.clone());
                }
            }
        }
    }
}
```

### Background Verification Worker

```rust
// Spawn worker thread to verify cache in background
std::thread::spawn(move || {
    while let Ok(path) = verify_rx.recv() {
        match compute_merkle_hash(&path) {
            Ok(current_hash) => {
                let cached_hash = cache.get_hash(&path).ok().flatten();
                let status = match cached_hash {
                    Some(h) if h == current_hash => CacheStatus::CachedVerified,
                    Some(_) => CacheStatus::Changed,
                    None => CacheStatus::Unknown,
                };
                let _ = result_tx.send((path, status, current_hash));
            }
            Err(e) => eprintln!("Verification failed for {:?}: {}", path, e),
        }
    }
});
```

## Comparison with rsync

| Feature | rsync | Reclaim Merkle Cache |
|---------|-------|---------------------|
| Quick check | mtime + size | Merkle hash (recursive) |
| Deep scan | Checksums (optional) | Full hash tree |
| Incremental | File-by-file | Directory-by-directory |
| False positives | Possible (mtime changes) | No (hash guaranteed) |
| Speed | Very fast (stat only) | Fast (cached hashes) |
| Accuracy | Good (with --checksum) | Perfect (with hash) |

## Implementation Phases

### Phase 1: Add CacheStatus enum (30 min)
- [ ] Add CacheStatus to CandidateState
- [ ] Update selection.rs
- [ ] Add badge rendering in GUI

### Phase 2: Merkle hash computation (1 hour)
- [ ] Implement compute_merkle_hash()
- [ ] Add merkle_cache table to SQLite
- [ ] Store hashes on scan

### Phase 3: Incremental scan (1-2 hours)
- [ ] Hash comparison logic
- [ ] Detect new/changed/verified
- [ ] Update cache on changes

### Phase 4: Background verification (1 hour)
- [ ] Worker thread for verification
- [ ] Lazy verification on view
- [ ] Progress indicators

### Phase 5: UI integration (1 hour)
- [ ] Status badges in all views
- [ ] Color coding
- [ ] Filter by cache status
- [ ] Legend panel

## Benefits

1. **⚡ Fast rescans**: Skip unchanged directories entirely
2. **🎯 Accurate**: Hash detects all changes, no false negatives
3. **📊 Transparent**: UI shows exactly what was verified vs cached
4. **🔄 Incremental**: Only process what changed
5. **💾 Persistent**: Cache survives app restarts
6. **🌳 Hierarchical**: Parent hash changes when any child changes

## Example Scenario

```
Initial scan:
- Scan ~/repos → compute merkle hash → store in cache
- Result: 500 candidates, all marked "New"

Hour later (rescan):
- ~/repos/.venv: hash unchanged → CachedVerified (skip scan)
- ~/repos/node_modules: hash changed → Changed (rescan)
- ~/repos/build: new directory → New (scan)
- ~/repos/target: hash unchanged → CachedVerified (skip)

Result: Only scanned 2/4 directories (50% savings)
```

## Memory Usage

```
Average directory with 1000 files:
- Merkle hash: 16 bytes × 1000 = 16 KB
- Metadata per entry: ~100 bytes × 1000 = 100 KB
- Total per 1000 files: ~120 KB

For 100K files: ~12 MB cache
```

Very reasonable for modern systems!
