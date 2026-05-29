# Immediate Improvements & Quick Wins

## UI/UX Enhancements (1-2 hours each)

### 1. **Keyboard Shortcuts**

```rust
// Add to GUI update() method:
if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
    // Toggle select on focused item
}
if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::A)) {
    // Select all
}
if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
    // Open cleanup confirmation
}
```

### 2. **Bulk Selection Actions**

- Add "Select All" / "Deselect All" buttons
- "Select by Score > 0.7" quick filter
- "Select all in this group" (in tree view)

### 3. **Status Bar with Live Stats**

```rust
// Bottom panel showing:
// Total: 1.2 TB | Scanned: 450 GB | Selected: 23 GB (45 items) | Scannable: 800 GB
```

### 4. **Export Functionality**

```rust
// Add buttons:
// - "Export to CSV" → candidates list with all fields
// - "Export Report" → HTML summary with charts
// - "Save Session" → JSON for later review
```

### 5. **Search/Filter Bar**

```rust
// Add text search at top:
// - Filter by path contains "..."
// - Filter by kind contains "..."
// Real-time as you type
```

### 6. **Undo/History**

```rust
// Track last N cleanup operations
// "Undo last cleanup" → restore from trash (if available)
// Show history log
```

---

## Performance Improvements (2-3 hours)

### 7. **Lazy Loading for Large Scans**

```rust
// Don't render all 10,000 items at once
// Use egui::ScrollArea with virtual scrolling
// Only render visible items + buffer
```

### 8. **Async Cleanup**

```rust
// Current apply_cleanup() blocks UI
// Use thread + channel like scan
// Show progress: "Removing 45/200 items..."
```

### 9. **Incremental Filtering**

```rust
// Cache filtered results
// Only recompute when filters change
// Use dirty flag pattern
```

---

## Data Quality & Safety (1-2 hours each)

### 10. **Dry-Run Preview Enhancement**

```rust
// Show:
// - Exact commands that will run (for Exec actions)
// - Estimated time (based on file count)
// - Risk assessment (red/yellow/green)
```

### 11. **Exclude Patterns in Profile**

```toml
[global]
exclude_patterns = [
    "*/important/*",
    "**/backup/**",
    "*/production/*"
]
```

### 12. **Size Verification**

```rust
// Before delete: re-check size hasn't changed
// Warn if file grew (active log file)
// Skip if locked/in-use
```

### 13. **Backup Before Delete** (optional)

```rust
// Move to ~/.reclaim-trash/ instead of rm -rf
// Keep for 7 days
// "Empty Reclaim Trash" button
```

---

## Code Architecture Improvements (4-6 hours)

### 14. **Extract GUI into Modules**

```
crates/reclaim-gui/src/
  main.rs         # App setup
  app.rs          # ReclaimApp struct
  views/
    table.rs      # render_table_view
    cards.rs      # render_cards_view
    compact.rs    # render_compact_view
    tree.rs       # render_tree_view
  panels/
    controls.rs   # Left sidebar
    top_bar.rs    # Profile selector + scan
    bottom_bar.rs # Status bar
  modals/
    confirm.rs    # Apply confirmation
    progress.rs   # Cleanup progress
```

### 15. **Shared State Management**

```rust
// Current: ReclaimApp has 20+ fields
// Better: Group related state
struct ScanState { ... }
struct FilterState { ... }
struct UIState { ... }

struct ReclaimApp {
    scan: ScanState,
    filters: FilterState,
    ui: UIState,
}
```

### 16. **Action Framework**

```rust
// Current: Action enum mixed with execution
// Better: Separate concerns

trait ActionExecutor {
    fn dry_run(&self, candidate: &Candidate) -> ActionPlan;
    fn execute(&self, candidate: &Candidate) -> Result<ExecutionResult>;
    fn can_undo(&self) -> bool;
    fn undo(&self, result: &ExecutionResult) -> Result<()>;
}

struct DeleteExecutor;
struct ExecExecutor { cmd: String, args: Vec<String> }
struct ArchiveExecutor { dest: PathBuf }
```

---

## Testing & Validation (3-4 hours)

### 17. **Integration Test Suite**

```rust
#[test]
fn test_full_scan_pipeline() {
    let tmp = setup_test_filesystem();
    let profile = Profile::load("conservative").unwrap();
    
    let candidates = scanner::scan(&[tmp.path()], &profile)?;
    assert!(candidates.len() > 0);
    
    // Verify filtering works
    // Verify scoring is correct
    // Verify no false positives
}
```

### 18. **Benchmark Suite**

```rust
// Use criterion.rs
// Benchmark:
// - Scan speed (GB/s)
// - Filter performance (items/ms)
// - UI render time (FPS)
```

### 19. **Fuzzing**

```rust
// cargo-fuzz
// Generate random filesystem structures
// Ensure no panics, no data loss
```

---

## Documentation (2-3 hours)

### 20. **User Guide**

```markdown
# docs/user-guide.md
- Installation
- Quick start
- Understanding scores
- Profile selection guide
- Safety tips
- FAQ
```

### 21. **Video Tutorial**

```
Record 5-minute screencast:
1. Launch app
2. Select profile
3. Scan
4. Review candidates
5. Apply cleanup
6. Verify results
```

### 22. **Inline Help**

```rust
// Add "?" icons with tooltips
// Explain each filter
// Explain score calculation
// Link to full docs
```

---

## Reliability & Error Handling (2-3 hours)

### 23. **Graceful Degradation**

```rust
// If scan fails on one root → continue others
// If one target fails → log and continue
// Never crash, always show partial results
```

### 24. **Permission Handling**

```rust
// Detect when we need sudo
// Show clear message: "Some paths require admin access"
// Offer to re-run with elevated privileges
```

### 25. **Progress Interruption**

```rust
// Allow canceling long scans
// Save partial results
// Resume from checkpoint
```

---

## Monitoring & Analytics (optional, 1-2 hours)

### 26. **Usage Statistics** (local only, privacy-first)

```rust
// Track in ~/.reclaim/stats.json:
// - Total space freed (lifetime)
// - Most common target types
// - Average scan time
// - Show in "About" dialog
```

### 27. **Scan History**

```rust
// Keep last 10 scans
// Show trend graph: space usage over time
// "Compare with last scan" feature
```

---

## Immediate Action Plan (for this week)

### Day 1: macOS Plugins (HIGH PRIORITY)

- [ ] Implement Xcode cleanup
- [ ] Implement Docker full support
- [ ] Implement Homebrew downloads
- [ ] Test on your Mac → free up 20-30 GB

### Day 2: UX Polish

- [ ] Add keyboard shortcuts
- [ ] Add bulk selection
- [ ] Add export to CSV
- [ ] Add search/filter bar

### Day 3: Safety & Performance

- [ ] Async cleanup with progress
- [ ] Backup before delete
- [ ] Size verification
- [ ] Lazy loading for large scans

### Day 4: Code Quality

- [ ] Extract GUI into modules
- [ ] Add integration tests
- [ ] Document all public APIs
- [ ] Fix all clippy warnings

### Day 5: Real-World Testing

- [ ] Full scan of your Mac
- [ ] Identify any false positives
- [ ] Measure performance (speed, memory)
- [ ] Collect feedback

---

## Success Metrics

- ✅ **Space freed**: 50+ GB on your Mac
- ✅ **Speed**: Full scan in <2 minutes
- ✅ **Safety**: Zero data loss incidents
- ✅ **UX**: Intuitive enough for non-technical users
- ✅ **Code quality**: All tests passing, 0 warnings
- ✅ **Documentation**: Complete user guide + API docs

Let's ship this! 🚀
