# Background Scanning Architecture

## Philosophy: Piggyback, Don't Reinvent

**Core Principle**: Leverage existing system services instead of building external daemons. Reclaim should be a single-process application that intelligently uses OS-level indexing and notification services.

## Current Implementation (v0.1)

### System Indexer Integration ✅

We already piggyback on these services:

1. **macOS Spotlight** (via `mdfind`)
   - 150x speedup for change detection
   - Query: `mdfind -onlyin ~/repos kMDItemFSContentChangeDate > $since`
   - Returns changed files since last scan
   - Roll up to directories for scanning

2. **Linux locate** (via `locate`)
   - Uses updatedb database
   - Query: Recent files in specified roots
   - Fallback if updatedb not current

3. **KDE Baloo** (via `baloosearch`)
   - KDE's file indexing service
   - Semantic search capabilities
   - Query: Modified files in roots

### Auto-Scan on Launch ✅

```rust
// On first update() call
if self.scan_status == ScanStatus::NotStarted {
    self.start_auto_scan();
}
```

**Default roots**:
- `~/` (home directory)
- `~/repos` (if exists)
- `~/Projects` (if exists)
- `~/Library` (macOS only, if exists)

### Continuous Background States

```
[Profile: ▼] [🟢 Scanning...] [⏸ Pause]  →  Scan in progress
[Profile: ▼] [🟡 Paused]      [▶ Resume] →  User paused
[Profile: ▼] [🔵 Idle]        [🔄]       →  Ready, waiting
[Profile: ▼] [❌ Error: ...]  [🔄]       →  Error, can retry
```

## Future Enhancements

### Phase 1: Real-time File System Monitoring 🚧

#### macOS: FSEvents

```rust
use fseventstream::*;

pub struct FSEventsWatcher {
    stream: FSEventStream,
    callback: Box<dyn Fn(Vec<PathBuf>)>,
}

impl FSEventsWatcher {
    pub fn watch(roots: Vec<PathBuf>, callback: impl Fn(Vec<PathBuf>) + 'static) {
        let stream = FSEventStream::new(
            &roots,
            Duration::from_secs(5), // Latency
            FSEventStreamCreateFlags::FILE_EVENTS,
        );
        
        stream.start(move |events| {
            let changed_paths: Vec<_> = events
                .iter()
                .filter(|e| e.flags.contains(FSEventStreamEventFlags::ITEM_MODIFIED))
                .map(|e| e.path.clone())
                .collect();
            
            if !changed_paths.is_empty() {
                callback(changed_paths);
            }
        });
    }
}
```

**Benefits**:
- Real-time change detection (no polling)
- Kernel-level monitoring (no syscall overhead)
- Coalescing of rapid changes (built-in debouncing)

**Trade-offs**:
- macOS only
- Requires `fseventstream` crate
- Callback must be Send + Sync

#### Linux: inotify

```rust
use inotify::{Inotify, WatchMask};

pub struct InotifyWatcher {
    inotify: Inotify,
    watches: HashMap<PathBuf, WatchDescriptor>,
}

impl InotifyWatcher {
    pub fn watch(roots: Vec<PathBuf>, callback: impl Fn(PathBuf) + 'static) {
        let mut inotify = Inotify::init().unwrap();
        
        for root in roots {
            inotify.add_watch(
                &root,
                WatchMask::CREATE | WatchMask::MODIFY | WatchMask::DELETE,
            ).unwrap();
        }
        
        // Event loop in background thread
        std::thread::spawn(move || {
            let mut buffer = [0; 1024];
            loop {
                let events = inotify.read_events_blocking(&mut buffer).unwrap();
                for event in events {
                    if let Some(name) = event.name {
                        let path = PathBuf::from(name.to_str().unwrap());
                        callback(path);
                    }
                }
            }
        });
    }
}
```

**Benefits**:
- Real-time change detection
- Very low overhead (kernel-level)
- Standard Linux API

**Trade-offs**:
- Requires recursive watches for directories
- Watch limit per process (usually 8192)
- Doesn't track moves across filesystems

#### Windows: ReadDirectoryChangesW

```rust
use winapi::um::winbase::ReadDirectoryChangesW;

pub struct WindowsWatcher {
    handles: Vec<HANDLE>,
}

impl WindowsWatcher {
    pub fn watch(roots: Vec<PathBuf>, callback: impl Fn(PathBuf) + 'static) {
        // Similar pattern to inotify
        // Uses ReadDirectoryChangesW API
    }
}
```

### Phase 2: Baloo Extensions (KDE) 🔮

**Opportunity**: Baloo is extensible via plugins. We could contribute a reclaim-specific indexer.

#### Potential Baloo Plugin

```cpp
// baloo-reclaim-extractor/main.cpp
#include <KFileMetaData/Extractor>

class ReclaimExtractor : public KFileMetaData::Extractor {
public:
    void extract(ExtractionResult* result) override {
        // Mark directories as "reclaimable"
        if (isNodeModules(result->inputUrl())) {
            result->add(Property::Subject, "reclaim-candidate");
            result->add(Property::Type, "npm-modules");
        }
        
        if (isVenv(result->inputUrl())) {
            result->add(Property::Subject, "reclaim-candidate");
            result->add(Property::Type, "python-venv");
        }
        
        // ... more detection logic
    }
    
    QStringList mimetypes() const override {
        return {"inode/directory"};
    }
};
```

**Benefits**:
- Baloo indexes automatically
- Fast queries: `baloosearch "reclaim-candidate AND type:npm-modules"`
- No duplicate work (Baloo already walking filesystem)
- Metadata persisted in Baloo database

**Trade-offs**:
- KDE only
- Requires C++ plugin development
- Distribution complexity (separate package)
- Baloo must be enabled by user

### Phase 3: Periodic Refresh Mode ⏰

```rust
pub struct RefreshScheduler {
    interval: Duration,
    last_scan: DateTime<Utc>,
}

impl RefreshScheduler {
    pub fn should_scan(&self) -> bool {
        Utc::now() - self.last_scan > self.interval
    }
    
    pub fn configure(mode: RefreshMode) {
        match mode {
            RefreshMode::Manual => None,
            RefreshMode::OnLaunch => Some(Duration::ZERO),
            RefreshMode::Hourly => Some(Duration::from_secs(3600)),
            RefreshMode::Daily => Some(Duration::from_secs(86400)),
            RefreshMode::Realtime => {
                // Use FSEvents/inotify
                Self::start_fs_watcher()
            }
        }
    }
}
```

### Phase 4: CPU Throttling 🎚️

```rust
pub struct ScanThrottler {
    max_cpu_percent: f32,
    check_interval: Duration,
}

impl ScanThrottler {
    pub fn should_pause(&self) -> bool {
        let usage = get_process_cpu_usage();
        usage > self.max_cpu_percent
    }
    
    pub fn adaptive_sleep(&self) {
        // Exponential backoff if CPU high
        if self.should_pause() {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

// In scanner thread:
for entry in walker {
    throttler.adaptive_sleep();
    // ... process entry
}
```

**UI Control**:
```
[⚙️ Configure]
  CPU Limit: [50%] ────────────────○──── [100%]
  Refresh:   ( ) Manual  (●) On Launch  ( ) Hourly  ( ) Realtime
```

## Implementation Priority

### High Priority (Next Release)
1. ✅ Auto-scan on launch with default roots
2. ✅ Pause/Resume controls
3. ✅ Status indicator in top bar
4. 🚧 FSEvents/inotify integration (macOS first)
5. 🚧 CPU throttling controls

### Medium Priority
6. 🔮 Periodic refresh scheduler
7. 🔮 Configurable roots UI (Settings modal)
8. 🔮 Smart sleep when idle (battery optimization)

### Low Priority (Research)
9. 🔮 Baloo plugin development
10. 🔮 Spotlight extension (if Apple allows)
11. 🔮 Windows Search integration

## Design Principles

1. **Single Process**: No separate daemon, no system service installation
2. **Piggyback First**: Use OS indexers before custom scanning
3. **Graceful Degradation**: Fall back to manual scan if OS services unavailable
4. **Non-Intrusive**: Respect CPU limits, pause when user is active
5. **Transparent**: Always show what's happening (status indicator)

## Why No External Service?

❌ **Bad**: Separate daemon process
- Requires system service installation (sudo)
- Launch agent/systemd unit complexity
- IPC between daemon and GUI
- Permission issues
- Hard to debug

✅ **Good**: Single-process with threads
- No installation complexity
- Direct state management
- Easy debugging
- Clear lifecycle (quit app = stop scanning)
- Better user trust (no hidden processes)

## Status Quo Comparison

### Traditional Approach
```
[Install Service] → [Configure launchd/systemd] → [Start Daemon]
     ↓                        ↓                         ↓
  sudo required         root permissions           hidden process
```

### Reclaim Approach
```
[Launch App] → [Auto-scan starts] → [Use OS indexers]
     ↓                  ↓                    ↓
  user level      transparent         piggyback system
```

## References

- FSEvents: https://developer.apple.com/documentation/coreservices/file_system_events
- inotify: https://man7.org/linux/man-pages/man7/inotify.7.html
- Baloo: https://community.kde.org/Baloo
- ReadDirectoryChangesW: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw
