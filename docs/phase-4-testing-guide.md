# Phase 4: Testing Guide

## Build and Launch

```bash
cd ~/repos/perso/reclaim
cargo build --release
./target/release/reclaim-gui
```

## Testing Checklist

### 1. Auto-Scan on Launch ⭐ NEW
- [ ] App launches successfully
- [ ] Scan starts automatically within 1 second
- [ ] Status indicator shows "🟢 Scanning..."
- [ ] No manual "Scan" button needed
- [ ] Progress visible in status bar

### 2. Default Roots
- [ ] Scan uses default roots: ~/, ~/repos, ~/Projects, ~/Library (macOS)
- [ ] Check console output or status to verify which roots are being scanned
- [ ] All default roots that exist are included

### 3. Scan Status & Controls ⭐ NEW
- [ ] **Scanning state**: "🟢 Scanning..." with [⏸ Pause] button
- [ ] **Paused state**: "🟡 Paused" with [▶ Resume] button
- [ ] **Idle state**: "🔵 Idle" with [🔄] refresh button
- [ ] **Error state**: "❌ Error: ..." with [🔄] retry button
- [ ] Pause button works - scan stops gracefully
- [ ] Resume button works - scan restarts
- [ ] Refresh button triggers new scan from idle

### 4. Cache Status Badges (Phase 1)
- [ ] **Cards View**: Badge visible next to checkbox
- [ ] **Compact View**: Badge visible, size shows ~ for estimates
- [ ] **Table View**: Status column with badges
- [ ] **Tree View**: Badges in grouped items
- [ ] Badge colors: 🟢✓ verified, 🟡~ unverified, 🔵N new, 🟠Δ changed

### 5. Verification Thread (Phase 2)
- [ ] After scan, verification starts automatically
- [ ] Status bar shows "Verifying cache... X/Y"
- [ ] Badges change from 🟡~ (unverified) to 🟢✓ (verified)
- [ ] Verification completes in 2-5 seconds for typical dataset
- [ ] Final status: "✓ Verified: X unchanged, Y changed, Z unavailable"

### 5. Hot Paths Discovery (Phase 3)
- [ ] After verification, discovery starts automatically
- [ ] Status bar shows "Discovering... X paths scanned"
- [ ] On macOS: Check if Spotlight is used (console output: "Using Spotlight for change detection")
- [ ] New items marked with 🔵N badge
- [ ] Discovery complete message shows new items count

### 6. Cache Persistence
- [ ] Close app
- [ ] Relaunch app
- [ ] Previously discovered items have 🟡~ badge (cached unverified)
- [ ] Verification thread re-verifies them
- [ ] User selections preserved across restarts

### 7. Disk Space Analysis
- [ ] Note total disk usage (top of window)
- [ ] Sort by size (largest first)
- [ ] Identify top space consumers:
  - Xcode DerivedData?
  - node_modules?
  - .venv directories?
  - Docker caches?
  - Homebrew caches?
  - VS Code chats?
- [ ] Document findings in session memory

### 8. Performance Benchmarks
- [ ] Measure scan time for full ~/repos
- [ ] Measure verification time (target: <5s for 10K items)
- [ ] Measure discovery time
- [ ] Check memory usage (Activity Monitor)

### 9. Cleanup Operations (Dry Run First!)
- [ ] Select a few test items
- [ ] Click "Dry Run" button
- [ ] Review planned actions
- [ ] Verify no important files targeted
- [ ] If safe, proceed with "Apply Actions"
- [ ] Confirm disk space reclaimed

## Known Limitations

1. **Hot paths discovery**: May not detect all new items if Spotlight indexing is disabled
2. **Size estimation**: Shallow directory scanning trades accuracy for speed
3. **Verification speed**: Large directories (>100K files) may take longer
4. **Cache invalidation**: Manual cache clear needed if disk structure changes dramatically

## Debugging Tips

- Console output shows plugin availability: `mdfind` on macOS
- Cache location: `~/.cache/reclaim/scan-cache.db`
- SQLite inspection: `sqlite3 ~/.cache/reclaim/scan-cache.db .schema`
- Enable debug logging: `RUST_LOG=debug ./target/release/reclaim-gui`

## Success Criteria

- [ ] All 4 phases complete without crashes
- [ ] Cache persists across app restarts
- [ ] User can identify top space consumers
- [ ] Verification completes in <10 seconds
- [ ] Cleanup operations work as expected
- [ ] User understands "why after only a few months it is full"

## Next Steps After Testing

Based on test results, consider:
- Add missing plugin types (Docker, iOS, Simulators, Trash)
- Improve verification speed for very large datasets
- Add progress bars for long operations
- Implement category editing (future: semantic taxonomy)
- Add duplicate group detection
