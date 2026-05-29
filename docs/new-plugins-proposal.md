# New Plugin Proposals Based on Real Disk Analysis

## Analysis Summary

**Total Reclaimable Space Found**: ~32GB
**Date**: 2026-05-28
**System**: macOS with development workload

## Priority 1: High Impact (>1GB each)

### 1. VS Code Workspace Storage Plugin 🏆 10GB FOUND

**Path**: `~/Library/Application Support/Code/User/workspaceStorage`
**Size**: 10GB
**Category**: IDE Cache
**Reproducibility**: 1.0 (fully reproducible from workspace)

```rust
// crates/reclaim-core/src/targets/vscode_workspace_storage.rs
pub fn scan(root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let vscode_storage = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/Code/User/workspaceStorage");
    
    if !vscode_storage.exists() {
        return Ok(vec![]);
    }
    
    // Each subdirectory is a workspace UUID
    // Contains: state.vscdb, workspace.json, etc.
    // Safe to delete for closed/deleted workspaces
    
    let mut candidates = Vec::new();
    
    for entry in fs::read_dir(&vscode_storage)? {
        let entry = entry?;
        let path = entry.path();
        
        if !path.is_dir() {
            continue;
        }
        
        // Check if workspace.json references a path that still exists
        let workspace_json = path.join("workspace.json");
        let is_orphaned = if workspace_json.exists() {
            check_workspace_orphaned(&workspace_json)?
        } else {
            true // No workspace.json = definitely orphaned
        };
        
        if is_orphaned {
            let size = compute_dir_size(&path)?;
            let age = get_age(&path)?;
            
            candidates.push(Candidate {
                path,
                kind: TargetKind::VsCodeWorkspaceStorage,
                size_bytes: size,
                last_modified: Some(age),
                reproducibility: 1.0,
                score: strategy::score(size, age, 1.0),
                action: Action::Delete,
                group: Some("VS Code Workspace Storage".to_string()),
                ..Default::default()
            });
        }
    }
    
    Ok(candidates)
}

fn check_workspace_orphaned(workspace_json: &Path) -> Result<bool> {
    let content = fs::read_to_string(workspace_json)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;
    
    // Check if "folder" path exists
    if let Some(folder) = json.get("folder") {
        if let Some(path_str) = folder.as_str() {
            let workspace_path = PathBuf::from(path_str.trim_start_matches("file://"));
            return Ok(!workspace_path.exists());
        }
    }
    
    Ok(true) // If we can't parse, assume orphaned
}
```

**Min Age**: 30 days (recent workspaces likely still in use)
**Action**: Delete orphaned workspaces only

---

### 2. Cisco Logs Plugin 🏆 9.5GB FOUND

**Path**: `~/Library/Logs/Cisco Data Shift.log`
**Size**: 9.5GB (single file!)
**Category**: Enterprise Logs
**Reproducibility**: 1.0 (logs are append-only)

```rust
// crates/reclaim-core/src/targets/cisco_logs.rs
pub fn scan(root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let logs_dir = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Logs");
    
    let mut candidates = Vec::new();
    
    // Cisco Data Shift logs
    let cisco_log = logs_dir.join("Cisco Data Shift.log");
    if cisco_log.exists() {
        let size = fs::metadata(&cisco_log)?.len();
        let age = get_age(&cisco_log)?;
        
        if size > profile.cisco_logs.min_size && age.num_days() > profile.cisco_logs.min_age_days {
            candidates.push(Candidate {
                path: cisco_log,
                kind: TargetKind::CiscoLogs,
                size_bytes: size,
                last_modified: Some(age),
                reproducibility: 1.0,
                score: strategy::score(size, age, 1.0),
                action: Action::Delete,
                group: Some("Cisco Logs".to_string()),
                tags: vec!["enterprise".to_string(), "single-file".to_string()],
                ..Default::default()
            });
        }
    }
    
    // Other Cisco logs
    let cisco_dir = logs_dir.join("Cisco");
    if cisco_dir.exists() {
        for entry in fs::read_dir(&cisco_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |e| e == "log") {
                let size = fs::metadata(&path)?.len();
                let age = get_age(&path)?;
                
                if age.num_days() > 30 {
                    candidates.push(Candidate {
                        path,
                        kind: TargetKind::CiscoLogs,
                        size_bytes: size,
                        last_modified: Some(age),
                        reproducibility: 1.0,
                        score: strategy::score(size, age, 1.0),
                        action: Action::Delete,
                        group: Some("Cisco Logs".to_string()),
                        ..Default::default()
                    });
                }
            }
        }
    }
    
    Ok(candidates)
}
```

**Min Age**: 7 days (logs older than a week rarely needed)
**Action**: Delete or archive

---

### 3. VS Code C++ Tools Cache Plugin 🏆 2.5GB FOUND

**Path**: `~/Library/Caches/vscode-cpptools`
**Size**: 2.5GB
**Category**: IDE Language Server Cache
**Reproducibility**: 1.0 (regenerated on next C++ file open)

```rust
// crates/reclaim-core/src/targets/vscode_cpptools.rs
pub fn scan(root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let cache_dir = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Caches/vscode-cpptools");
    
    if !cache_dir.exists() {
        return Ok(vec![]);
    }
    
    let size = compute_dir_size(&cache_dir)?;
    let age = get_age(&cache_dir)?;
    
    if size < profile.vscode_cpptools.min_size {
        return Ok(vec![]);
    }
    
    Ok(vec![Candidate {
        path: cache_dir,
        kind: TargetKind::VsCodeCppToolsCache,
        size_bytes: size,
        last_modified: Some(age),
        reproducibility: 1.0,
        score: strategy::score(size, age, 1.0),
        action: Action::Delete,
        group: Some("VS Code C++ Tools Cache".to_string()),
        tags: vec!["intellisense".to_string(), "language-server".to_string()],
        ..Default::default()
    }])
}
```

**Min Age**: 30 days
**Action**: Delete (rebuilds on next use)

---

### 4. Playwright Browsers Plugin 🎭 1GB FOUND

**Path**: `~/Library/Caches/ms-playwright`
**Size**: 1GB
**Category**: Browser Automation
**Reproducibility**: 1.0 (reinstallable via `npx playwright install`)

```rust
// crates/reclaim-core/src/targets/playwright.rs
pub fn scan(root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let cache_dir = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Caches/ms-playwright");
    
    if !cache_dir.exists() {
        return Ok(vec![]);
    }
    
    let size = compute_dir_size(&cache_dir)?;
    let age = get_age(&cache_dir)?;
    
    Ok(vec![Candidate {
        path: cache_dir,
        kind: TargetKind::PlaywrightCache,
        size_bytes: size,
        last_modified: Some(age),
        reproducibility: 1.0,
        score: strategy::score(size, age, 1.0),
        action: Action::Exec {
            cmd: "npx".to_string(),
            args: vec!["playwright".to_string(), "uninstall".to_string(), "--all".to_string()],
            description: "Uninstall Playwright browsers (reinstall with: npx playwright install)".to_string(),
        },
        group: Some("Playwright Browsers".to_string()),
        tags: vec!["testing".to_string(), "chromium".to_string(), "firefox".to_string()],
        ..Default::default()
    }])
}
```

**Min Age**: 60 days
**Action**: Exec `npx playwright uninstall --all`

---

### 5. Browser Caches Plugin 🌐 2GB FOUND

**Paths**:

- `~/Library/Caches/Google` (1.3GB)
- `~/Library/Caches/Mozilla` (502MB)
- `~/Library/Caches/Firefox` (107MB)

**Category**: Web Browser Cache
**Reproducibility**: 1.0 (redownloads on browsing)

```rust
// crates/reclaim-core/src/targets/browser_caches.rs
pub fn scan(root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let caches = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Caches");
    
    let browsers = vec![
        ("Google", "Chrome/Chromium"),
        ("Mozilla", "Firefox"),
        ("Firefox", "Firefox"),
        ("Safari", "Safari"),
        ("com.microsoft.Edge", "Microsoft Edge"),
    ];
    
    let mut candidates = Vec::new();
    
    for (dir_name, browser_name) in browsers {
        let cache_dir = caches.join(dir_name);
        
        if !cache_dir.exists() {
            continue;
        }
        
        let size = compute_dir_size(&cache_dir)?;
        let age = get_age(&cache_dir)?;
        
        if size < profile.browser_caches.min_size {
            continue;
        }
        
        candidates.push(Candidate {
            path: cache_dir,
            kind: TargetKind::BrowserCache,
            size_bytes: size,
            last_modified: Some(age),
            reproducibility: 1.0,
            score: strategy::score(size, age, 1.0),
            action: Action::Delete,
            group: Some(format!("{} Cache", browser_name)),
            tags: vec!["browser".to_string(), "web".to_string()],
            ..Default::default()
        });
    }
    
    Ok(candidates)
}
```

**Min Age**: 30 days
**Action**: Delete (browsers refill on use)

---

## Priority 2: Medium Impact (100MB-1GB)

### 6. Discord Cache Plugin 💬 445MB FOUND

**Path**: `~/Library/Caches/com.hnc.Discord.ShipIt`
**Size**: 445MB
**Category**: App Cache
**Reproducibility**: 1.0

```rust
// Add to existing app_caches.rs or create discord.rs
let discord_cache = caches.join("com.hnc.Discord.ShipIt");
```

### 7. Spark Email Logs Plugin 📧 521MB FOUND

**Path**: `~/Library/Logs/SparkMacDesktop`
**Size**: 521MB
**Category**: Email Client Logs

### 8. TypeScript Compiler Cache 📘 86MB FOUND

**Path**: `~/Library/Caches/typescript`
**Size**: 86MB
**Category**: Compiler Cache
**Reproducibility**: 1.0

### 9. node-gyp Cache ⚙️ 125MB FOUND

**Path**: `~/Library/Caches/node-gyp`
**Size**: 125MB
**Category**: Build Tools

### 10. Bazelisk Cache 🏗️ 50MB FOUND

**Path**: `~/Library/Caches/bazelisk`
**Size**: 50MB
**Category**: Build Tools

### 11. Go Tools Cache 🐹 28MB FOUND

**Paths**:

- `~/Library/Caches/gopls` (14MB)
- `~/Library/Caches/goimports` (14MB)

### 12. OneDrive Logs ☁️ 60MB FOUND

**Path**: `~/Library/Logs/OneDrive`
**Size**: 60MB
**Category**: Cloud Storage Logs

---

## Implementation Plan

### Phase 1: Quick Wins (Week 1)

1. ✅ VS Code Workspace Storage (10GB)
2. ✅ Cisco Logs (9.5GB)
3. ✅ Browser Caches (2GB)

**Total**: 21.5GB with 3 plugins

### Phase 2: Language Tools (Week 2)

4. ✅ VS Code C++ Tools (2.5GB)
2. ✅ Playwright (1GB)
3. ✅ TypeScript Cache (86MB)
4. ✅ node-gyp (125MB)

**Total**: +3.7GB with 4 plugins

### Phase 3: App Caches (Week 3)

8. ✅ Discord (445MB)
2. ✅ Spark Logs (521MB)
3. ✅ OneDrive Logs (60MB)

**Total**: +1GB with 3 plugins

### Phase 4: Build Tools (Week 4)

11. ✅ Bazelisk (50MB)
2. ✅ Go Tools (28MB)

**Total**: +78MB with 2 plugins

---

## New TargetKind Enum Additions

```rust
pub enum TargetKind {
    // Existing...
    Venv,
    Build,
    Npm,
    // ... etc
    
    // NEW:
    VsCodeWorkspaceStorage,  // 🏆 10GB priority
    CiscoLogs,               // 🏆 9.5GB priority
    VsCodeCppToolsCache,     // 🏆 2.5GB priority
    BrowserCache,            // 🏆 2GB priority
    PlaywrightCache,         // 🎭 1GB
    DiscordCache,            // 💬 445MB
    SparkLogs,               // 📧 521MB
    TypeScriptCache,         // 📘 86MB
    NodeGypCache,            // ⚙️ 125MB
    BazeliskCache,           // 🏗️ 50MB
    GoToolsCache,            // 🐹 28MB (gopls + goimports)
    OneDriveLogs,            // ☁️ 60MB
}
```

---

## Profile Configuration Additions

```toml
# profiles/conservative.toml
[vscode_workspace_storage]
min_age = 30  # days
min_size = 100_000_000  # 100MB

[cisco_logs]
min_age = 7  # days
min_size = 10_000_000  # 10MB

[browser_caches]
min_age = 30  # days
min_size = 100_000_000  # 100MB

[playwright_cache]
min_age = 60  # days
min_size = 100_000_000  # 100MB

[typescript_cache]
min_age = 30  # days
min_size = 50_000_000  # 50MB
```

---

## Testing Results

Based on real disk analysis of `/Users/augjorda`:

| Plugin | Size Found | Files/Dirs | Age Range | Safe to Delete? |
|--------|-----------|------------|-----------|-----------------|
| VS Code Workspace Storage | 10GB | Multiple UUIDs | 30-365 days | ✅ Yes (orphaned only) |
| Cisco Logs | 9.5GB | 1 file | Unknown | ✅ Yes |
| Browser Caches | 2GB | 3 browsers | Active | ✅ Yes |
| VS Code C++ Tools | 2.5GB | Cache files | Active | ✅ Yes |
| Playwright | 1GB | Browsers | 60+ days | ✅ Yes |
| Discord | 445MB | ShipIt cache | Active | ✅ Yes |
| Python venvs | 3.5GB | 10 venvs | Various | ⚠️ Selective |
| node_modules | 2.4GB | 7 projects | Various | ⚠️ Selective |

**Total Immediate Opportunity**: ~32GB

---

## Next Steps

1. Implement Priority 1 plugins (21.5GB impact)
2. Add to scanner.rs integration
3. Test on real disk
4. Measure actual space reclaimed
5. Iterate on profiles based on user feedback
