# Auto-Scan Architecture - Implementation Summary

## What Changed

Transformed from **manual scan workflow** → **continuous background scanning**

### Before (Manual)

```
[Profile: ▼] [Scan Button]
```

- User must click "Scan" to start
- Roots must be configured manually
- App sits idle until user action

### After (Continuous)

```
[Profile: ▼] [🟢 Scanning...] [⏸ Pause]
```

- Auto-starts on launch
- Uses intelligent default roots
- Always working in background
- User can pause/resume/refresh

## UI Changes

### Removed

- ❌ Manual "Scan" button
- ❌ Root configuration fields (moved to future Settings)

### Added

- ✅ **Status Indicator**:
  - 🟢 Scanning... (active)
  - 🟡 Paused (user paused)
  - 🔵 Idle (ready)
  - ❌ Error: ... (failed)

- ✅ **Control Buttons**:
  - ⏸ Pause (when scanning)
  - ▶ Resume (when paused)
  - 🔄 Refresh (when idle/error)

### Default Scan Roots

Automatically scans these on launch (if they exist):

- `~/` (home directory)
- `~/repos` (development projects)
- `~/Projects` (alternative dev location)
- `~/Library` (macOS caches and app data)

## Architecture Benefits

### 1. Piggyback on System Services ✅

Already implemented:

- **macOS Spotlight** (150x speedup)
- **Linux locate/updatedb**
- **KDE Baloo**

No external daemon needed - just use what's already there.

### 2. Single Process Model ✅

- No system service installation
- No sudo required
- No hidden background processes
- Clear lifecycle (quit = stop)
- Easy debugging

### 3. Transparent Operation ✅

User always sees what's happening:

- Status indicator in top bar
- Progress updates during scan
- Pause when needed (CPU concerns)
- Manual refresh available

## Implementation Details

### Auto-Start Logic

```rust
impl eframe::App for ReclaimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto-start on first update
        if self.scan_status == ScanStatus::NotStarted {
            self.start_auto_scan();
        }
        // ... rest of update loop
    }
}
```

### Status State Machine

```
NotStarted ──(auto)──> Scanning ──(user)──> Paused
                           │                     │
                           │                     └──(resume)──┐
                           │                                  │
                           └──(complete)──> Idle <────────────┘
                                            │
                                            └──(refresh)──> Scanning
                                            
                        (any) ──(error)──> Error
                                            │
                                            └──(retry)──> Scanning
```

### Future: Real-Time Monitoring

See [background-scanning.md](./background-scanning.md) for detailed plans:

- **macOS FSEvents**: Real-time file changes (kernel-level)
- **Linux inotify**: Low-overhead change detection
- **Windows ReadDirectoryChangesW**: Native file monitoring
- **KDE Baloo Extensions**: Custom indexer plugins
- **CPU Throttling**: Adaptive sleep when high usage

## Testing

Launch and verify:

```bash
./target/release/reclaim-gui
```

Expected behavior:

1. App launches
2. Within 1 second: Status shows "🟢 Scanning..."
3. Progress visible as it scans default roots
4. Transitions to "🔵 Idle" when complete
5. Verification and discovery threads auto-start
6. User can pause/resume at any time

See [phase-4-testing-guide.md](./phase-4-testing-guide.md) for full checklist.

## Philosophy

> "Don't build what already exists. Piggyback on system services, stay single-process, be transparent."

This architecture respects:

- **User trust**: No hidden daemons
- **System resources**: Use existing indexers
- **User control**: Always visible, pausable
- **Simplicity**: One binary, no installation

## Next Steps

1. **Test on real Mac** - verify auto-scan works with real disk
2. **FSEvents integration** - add real-time monitoring (Phase 1)
3. **CPU throttling** - adaptive sleep when high usage
4. **Settings UI** - configure roots, refresh interval, CPU limits
5. **Baloo extensions** - contribute upstream plugins (KDE)

---

**Status**: ✅ Implemented and ready for testing
**Build**: Release build succeeds (2.85s)
**Warnings**: 5 (unused helper methods, safe to ignore)
