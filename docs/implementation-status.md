# Progressive Scanning Architecture - Implementation Summary

## ✅ What's Implemented

### 1. **Core Data Model** ([selection.rs](../crates/reclaim-core/src/selection.rs))

```rust
pub enum CacheStatus {
    Unknown,            // ⚪ ? - Never seen or outside roots
    CachedUnverified,   // 🟡 ~ - In cache, not verified this session
    CachedVerified,     // 🟢 ✓ - Verified unchanged
    Changed,            // 🟠 Δ - Size changed from cache
    New,                // 🔵 N - Discovered this session
}

pub struct CandidateState {
    pub candidate: Candidate,
    pub cache_status: CacheStatus,
    pub size_cached: Option<u64>,     // Last full scan
    pub size_current: Option<u64>,    // Last verification (may be estimate)
    pub last_verified: DateTime<Utc>,
    // ... selection tracking ...
}
```

### 2. **Smart Prioritization** ([priority.rs](../crates/reclaim-core/src/priority.rs))

#### Two Heuristics:

**First Scan** - Maximize large file discovery:
- node_modules, target, .venv, build: **+50 pts**
- repos/, projects/, workspace/: **+30 pts**
- cache/, deriveddata/: **+40 pts**
- Penalty for media, system folders

**Rescan** - Maximize change detection:
- target/debug, __pycache__: **+60 pts**
- node_modules, .venv: **+50 pts**
- tmp/, cache/: **+55 pts**
- Time factor: older scans get higher priority (×2 max)

#### Progressive Phases:

```rust
pub enum ScanPhase {
    Quick,      // Report after 50 items or 10 GB
    Medium,     // Report after 200 items or 50 GB
    Thorough,   // Report after 1000 items or 200 GB
    Complete,
}
```

### 3. **System Integration** ([plugins.rs](../crates/reclaim-core/src/plugins.rs))

Piggyback on OS indexers for fast change detection:

| Platform | Plugin | Command | Speed |
|----------|--------|---------|-------|
| macOS | Spotlight | `mdfind` | Near-instant |
| Linux | locate/updatedb | `locate` | Very fast |
| Linux | Baloo (KDE) | `baloosearch` | Fast |

```rust
pub fn detect_changed_dirs(root: &Path, since: DateTime<Utc>) -> Option<Vec<PathBuf>> {
    // Uses system indexer to find files modified since timestamp
    // Rolls up to parent directories
    // Fallback to None if unavailable
}
```

### 4. **Cache Implementation** ([cache.rs](../crates/reclaim-core/src/cache.rs))

```sql
CREATE TABLE candidates_cache (
    path TEXT PRIMARY KEY,
    size_cached INTEGER,      -- Size at last full scan
    size_current INTEGER,     -- Size at last verification
    cache_status TEXT,        -- Status enum
    last_verified INTEGER,    -- Last Tier 1 check timestamp
    ...
);
```

Methods:
- `merge_scan_results()` - Integrates new scan with cache
- `save_user_selections()` - Persists manual selections
- `purge_old_entries()` - Cleanup old cache

### 5. **GUI Integration** ([reclaim-gui/src/main.rs](../crates/reclaim-gui/src/main.rs))

#### Update Banner (not modal):
```
╔════════════════════════════════════════════════════════╗
║ 📊 Scan Update Available                              ║
║ 5 new items, 2 changed items                          ║
║                         [✖ Dismiss] [🔄 Update View]  ║
╚════════════════════════════════════════════════════════╝
```

#### Tab Bar for Views:
```
[📊 Table] [🎴 Cards] [📋 Compact] [🌳 Tree]
```

## 🎯 User Experience Flow

### On App Launch:
```
1. Load cache → Display immediately (🟡 unverified)
   ├─ User sees data in <100ms
   └─ Banner: "Showing cached data, verifying..."

2. Background: Tier 1 Verification (seconds)
   ├─ stat() each known candidate
   ├─ Compare sizes
   └─ Update status: 🟡 → 🟢 or 🟠

3. Background: Hot Paths Discovery (seconds)
   ├─ Check ~/Downloads, ~/Desktop, ~/repos, etc.
   ├─ Optional: Use Spotlight/updatedb for speed
   └─ Mark new items as 🔵

4. Banner appears if changes detected:
   "5 new items, 2 changed. [Review] [Update]"

5. User can trigger full Deep Scan manually
```

### Status Indicators in UI:

| Badge | Meaning | Size Display |
|-------|---------|--------------|
| ⚪ ? | Unknown | Unknown size |
| 🟡 ~ | Cached, unverified | ~12 MB (estimate) |
| 🟢 ✓ | Verified unchanged | 12 MB |
| 🟠 Δ | Changed | 15 MB (was 12 MB) |
| 🔵 N | New this scan | 8 MB |

## 🔧 User Controls

### In UI:
- **Pause button** - Stop current scan phase
- **Skip to next phase** - Jump from Quick → Medium → Thorough
- **Force full scan** - Override heuristics, scan everything
- **Configure hot paths** - Add/remove priority directories

### Command-line (future):
```bash
reclaim scan --quick              # Stop after Quick phase
reclaim scan --priority ./mydir   # Scan this dir first
reclaim scan --full               # Ignore cache, scan everything
```

## 📊 Performance Estimates

### Test System: 500 GB, 10,000 known candidates

| Operation | Time | Coverage |
|-----------|------|----------|
| Load cache | 100ms | 100% (cached) |
| Tier 1 verify | 2-5s | 100% (known) |
| Hot paths | 3-7s | ~80-90% (likely changes) |
| Full scan | 5-10min | 100% (everything) |

### Change Detection with Spotlight (macOS):
```
Without plugin: Scan entire ~/repos (50 GB) = 30 seconds
With Spotlight: Query changes since last scan = 200ms
Speedup: 150x
```

## 🔮 Next Steps

### Phase 1: Badges & Visual Indicators (1-2h)
- [ ] Add cache status badges to table view
- [ ] Add size estimation indicators
- [ ] Color-code changed items (orange)
- [ ] Color-code new items (blue)

### Phase 2: Verification Thread (2-3h)
- [ ] Spawn background thread on app startup
- [ ] Implement Tier 1 point verification
- [ ] Update UI progressively as items verify
- [ ] Show progress in status bar

### Phase 3: Hot Paths Discovery (2-3h)
- [ ] Implement hot paths scanner
- [ ] Try system plugins (Spotlight/etc)
- [ ] Merge discovered items with cache
- [ ] Trigger banner when changes found

### Phase 4: User Controls (1-2h)
- [ ] Add pause/resume button
- [ ] Add "Force Full Scan" button
- [ ] Add hot paths configuration
- [ ] Save/load hot paths from config

### Phase 5: Testing & Optimization (2-3h)
- [ ] Test on real disk with 100GB+ data
- [ ] Benchmark verification speed
- [ ] Tune phase thresholds
- [ ] Polish UI feedback

## 💡 Key Design Decisions

1. **Trade-off Accepted**: Not 100% complete detection (95% is enough)
2. **User-Centric**: Show data immediately, improve over time
3. **Transparent**: Always indicate verification status
4. **Non-blocking**: Never block UI thread
5. **Configurable**: Users can override everything

## 🧪 Example Scenarios

### Scenario 1: Daily Developer
```
Day 1 (first run):
  - Full scan: 5 minutes
  - Found: 150 candidates, 45 GB

Day 2 (reopening):
  - Load cache: instant
  - Verify (background): 3 seconds
  - Hot paths: found 2 new node_modules (new projects)
  - Banner: "2 new items detected"
  - Total time to actionable data: <5 seconds
```

### Scenario 2: Weekly Cleanup
```
Week 1: Scanned, cleaned 20 GB
Week 2: 
  - Spotlight reports: 5 dirs changed in ~/repos
  - Prioritize those 5 dirs → scan 2 seconds
  - Rest: verify cache → 3 seconds
  - Banner: "3 changed items, 1 new"
```

### Scenario 3: After Long Break
```
Opened after 2 months:
  - Load cache: instant (but old)
  - Verification finds many changes
  - Banner: "50 changed items, 10 new"
  - Suggests: "Run full scan to update all"
```

## 📐 Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    Reclaim GUI                          │
│  ┌──────────────────────────────────────────────────┐  │
│  │ Tab Bar: [Table] [Cards] [Compact] [Tree]       │  │
│  ├──────────────────────────────────────────────────┤  │
│  │ Banner: 📊 5 new, 2 changed [Dismiss] [Update]  │  │
│  ├──────────────────────────────────────────────────┤  │
│  │ Candidates List (with badges)                    │  │
│  │   🟢 .venv/ 12 MB (verified)                     │  │
│  │   🟠 target/ 15 MB (was 12 MB)                   │  │
│  │   🔵 build/ 8 MB (new)                           │  │
│  │   🟡 node_modules/ ~50 MB (unverified)           │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│              Background Threads                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Tier 1       │  │ Hot Paths    │  │ System       │  │
│  │ Verifier     │  │ Discovery    │  │ Plugins      │  │
│  │              │  │              │  │              │  │
│  │ stat() each  │  │ Scan ~/Down- │  │ Spotlight/   │  │
│  │ known path   │  │ loads, etc.  │  │ updatedb     │  │
│  │              │  │              │  │              │  │
│  │ → 🟡→🟢/🟠   │  │ → 🔵 new     │  │ → changed    │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│              SQLite Cache                                │
│  ┌────────────────────────────────────────────────────┐ │
│  │ candidates_cache (path, size_cached,               │ │
│  │                  size_current, cache_status)       │ │
│  │ user_selections (manual checkboxes preserved)     │ │
│  │ hot_paths (learned priority locations)            │ │
│  └────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

---

**Status**: ✅ Core implementation complete, compiles successfully  
**Next**: Add UI badges and start verification thread
