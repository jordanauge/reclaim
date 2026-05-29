# Cache & Auto-Update Architecture

## Overview

Background scanning with persistent cache that maintains user selections across rescans.

## Data Model

### SelectionMode

```rust
pub enum SelectionMode {
    Auto,    // System-controlled based on score
    Manual,  // User explicitly set, persists across rescans
}
```

### SelectionState  

```rust
pub enum SelectionState {
    Unchecked,
    Checked,
    Indeterminate,  // For groups with mixed children
}
```

### CandidateState

```rust
pub struct CandidateState {
    pub candidate: Candidate,
    pub selection_mode: SelectionMode,
    pub selection_state: SelectionState,
    pub is_new: bool,  // True if discovered in latest scan
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}
```

## Cache Structure (SQLite)

### Tables

#### `cached_entries`

```sql
CREATE TABLE cached_entries (
    path TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,  -- hash(mtime + size)
    size_bytes INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL
);
CREATE INDEX idx_last_seen ON cached_entries(last_seen);
```

#### `user_selections`

```sql
CREATE TABLE user_selections (
    path TEXT PRIMARY KEY,
    is_checked BOOLEAN NOT NULL,
    selection_mode TEXT NOT NULL,  -- 'auto' or 'manual'
    timestamp INTEGER NOT NULL,
    FOREIGN KEY (path) REFERENCES cached_entries(path)
);
```

#### `scan_runs`

```sql
CREATE TABLE scan_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    roots TEXT NOT NULL,  -- JSON array
    profile_name TEXT NOT NULL,
    items_found INTEGER,
    new_items INTEGER,
    changed_items INTEGER
);
```

## Background Scanning Flow

### 1. Initial Scan

```
User clicks Scan
  → Full scan + build cache
  → All items marked as Auto mode
  → Items with score ≥ 0.7 → Checked
  → Store in cache
```

### 2. User Interactions

```
User checks/unchecks item
  → Mark as Manual mode
  → Store in user_selections table
  → Persist across rescans
```

### 3. Background Rescan

```
Timer triggers (every 5 minutes? configurable)
  → Incremental scan (only changed paths)
  → Compare with cache:
    - New items → is_new = true, Auto mode
    - Changed items → update metadata, preserve Manual selections
    - Removed items → remove from cache
  → If changes detected:
    → Send notification to GUI
    → Show popup: "5 new items, 2 changed. Update view?"
```

## UI Components

### Enhanced Checkbox Widget

```
┌─────────────────────┐
│ [☑] M  100 MB  venv │  ← Manual, Checked
│ [☐] A   50 MB  npm  │  ← Auto, Unchecked (score < 0.7)
│ [☒] -  150 MB  Group│  ← Indeterminate (mixed children)
└─────────────────────┘
```

### Update Notification Modal

```
┌──────────────────────────────────────┐
│  📊 Scan Update Available            │
│                                      │
│  New items:      5 (12 GB)          │
│  Changed items:  2                   │
│  Removed items:  1                   │
│                                      │
│  [Review Changes] [Update Now] [Skip]│
└──────────────────────────────────────┘
```

### Highlight Scheme

- **New items**: Yellow background
- **Changed items**: Blue background  
- **Manual selections**: Bold text or icon badge
- **Auto selections**: Normal text

## Group State Aggregation

### Tri-state Logic

```rust
fn compute_group_state(children: &[CandidateState]) -> (SelectionState, SelectionMode) {
    let checked_count = children.iter().filter(|c| c.selection_state == Checked).count();
    let manual_count = children.iter().filter(|c| c.selection_mode == Manual).count();
    
    let state = if checked_count == 0 {
        SelectionState::Unchecked
    } else if checked_count == children.len() {
        SelectionState::Checked
    } else {
        SelectionState::Indeterminate
    };
    
    let mode = if manual_count == 0 {
        SelectionMode::Auto
    } else if manual_count == children.len() {
        SelectionMode::Manual
    } else {
        SelectionMode::Mixed  // New variant needed
    };
    
    (state, mode)
}
```

## Implementation Phases

### Phase 1: Data Model (1-2 hours)

- [x] Add SelectionMode/SelectionState enums
- [ ] Wrap Candidate in CandidateState
- [ ] Update all UI code to use CandidateState

### Phase 2: SQLite Cache (2-3 hours)

- [ ] Create cache module with rusqlite
- [ ] Implement cache read/write
- [ ] Implement incremental scan logic
- [ ] Change detection (hash comparison)

### Phase 3: Background Scanning (2-3 hours)

- [ ] Background thread with timer
- [ ] Send updates via channel
- [ ] Update notification system

### Phase 4: UI Updates (3-4 hours)

- [ ] Enhanced checkbox widget
- [ ] Mode indicator badge (A/M/-)
- [ ] Highlight new/changed items
- [ ] Update notification modal
- [ ] Group tri-state display

### Phase 5: Testing & Polish (2-3 hours)

- [ ] Test manual selection persistence
- [ ] Test background scan
- [ ] Test change detection
- [ ] Performance validation

## Configuration

```toml
[cache]
enabled = true
path = "~/.cache/reclaim/scan-cache.db"
rescan_interval_minutes = 5
max_age_days = 30  # Purge old cache entries

[ui]
highlight_new_items = true
highlight_changed_items = true
show_update_notifications = true
auto_update = false  # If true, update view automatically without popup
```

## Benefits

1. **Always up-to-date**: Background scanning keeps data fresh
2. **No re-decisions**: Manual selections persist
3. **Clear visibility**: Visual indicators for new/changed items
4. **Smart grouping**: Tri-state shows group consistency
5. **Fast rescans**: Incremental scanning via cache
6. **Offline capable**: Cache allows viewing without rescan

## Technical Considerations

### Change Detection Algorithm

```rust
fn content_hash(path: &Path) -> Result<String> {
    let meta = fs::metadata(path)?;
    let mtime = meta.modified()?.duration_since(UNIX_EPOCH)?.as_secs();
    let size = meta.len();
    
    // Quick hash without reading file content
    Ok(format!("{:x}", seahash::hash(&format!("{}-{}", mtime, size))))
}
```

### Memory Management

- Only load visible items in UI (lazy loading)
- Cache stores metadata, not full Candidate objects
- Background thread has low priority

### Race Conditions

- Use Arc<Mutex<>> for shared state
- SQLite with WAL mode for concurrent access
- Atomic operations for critical sections

### Error Handling

- Background scan failures don't crash UI
- Cache corruption → rebuild from scratch
- Network drive timeouts → skip that root
