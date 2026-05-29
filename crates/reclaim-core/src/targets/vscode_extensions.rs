use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Scan VS Code extension data and caches
/// 
/// VS Code stores extension data, file history, and cached extension packages
/// that can grow significantly over time:
/// - globalStorage: Extension persistent data (800MB+)
/// - History: File edit history (678MB+)
/// - CachedExtensionVSIXs: Downloaded extension packages (565MB+)
/// 
/// Safe to delete:
/// - Old history (extensions will work without it)
/// - Cached VSIX files (re-downloaded on next update)
/// - Unused extension storage (if extensions are uninstalled)
pub fn scan(_root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("vscode-extensions");
    if !config.enabled {
        return Ok(vec![]);
    }
    
    let min_size_bytes = profile.min_size_bytes.unwrap_or(50 * 1024 * 1024); // Default 50MB
    
    let vscode_user = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join("Library/Application Support/Code/User");
    
    if !vscode_user.exists() {
        return Ok(vec![]);
    }
    
    let mut candidates = Vec::new();
    
    // 1. Scan globalStorage (extension persistent data)
    let global_storage = vscode_user.join("globalStorage");
    if global_storage.exists() {
        for entry in WalkDir::new(&global_storage)
            .max_depth(2) // Only scan top-level extension dirs
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            
            if !path.is_dir() || path == global_storage {
                continue;
            }
            
            let metadata = match fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            
            // Calculate directory size
            let size = calculate_dir_size(path);
            
            if size < min_size_bytes {
                continue;
            }
            
            let modified = metadata.modified().ok()
                .map(chrono::DateTime::<chrono::Utc>::from);
            
            let extension_id = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            
            candidates.push(Candidate {
                path: path.to_path_buf(),
                kind: TargetKind::VsCodeExtensionData,
                action: Action::Skip,
                size_bytes: size,
                last_modified: modified,
                last_accessed: None,
                reproducibility: 0.5, // May contain extension settings
                score: 0.5, // Medium score - requires review
                tags: vec!["extension-data".to_string(), extension_id.to_string(), "review".to_string()],
                group: Some("VS Code Extension Storage".to_string()),
            });
        }
    }
    
    // 2. Scan History (file edit history)
    let history = vscode_user.join("History");
    if history.exists() {
        let size = calculate_dir_size(&history);
        
        if size >= min_size_bytes {
            let metadata = fs::metadata(&history).ok();
            let modified = metadata.as_ref()
                .and_then(|m| m.modified().ok())
                .map(chrono::DateTime::<chrono::Utc>::from);
            
            candidates.push(Candidate {
                path: history,
                kind: TargetKind::VsCodeExtensionData,
                action: Action::Delete,
                size_bytes: size,
                last_modified: modified,
                last_accessed: None,
                reproducibility: 1.0, // Fully reproducible (VS Code regenerates)
                score: 0.7, // Higher score - safe to delete
                tags: vec!["history".to_string(), "cache".to_string()],
                group: Some("VS Code File History".to_string()),
            });
        }
    }
    
    // 3. Scan CachedExtensionVSIXs (downloaded extension packages)
    let cached_vsix = vscode_user.parent()
        .map(|p| p.join("CachedExtensionVSIXs"));
    
    if let Some(cached_vsix) = cached_vsix {
        if cached_vsix.exists() {
            for entry in WalkDir::new(&cached_vsix)
                .max_depth(1)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                
                if !path.is_file() || path == cached_vsix {
                    continue;
                }
                
                let metadata = match fs::metadata(path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                
                if metadata.len() < min_size_bytes {
                    continue;
                }
                
                let modified = metadata.modified().ok()
                    .map(chrono::DateTime::<chrono::Utc>::from);
                
                candidates.push(Candidate {
                    path: path.to_path_buf(),
                    kind: TargetKind::VsCodeExtensionData,
                    action: Action::Delete,
                    size_bytes: metadata.len(),
                    last_modified: modified,
                    last_accessed: None,
                    reproducibility: 1.0, // Will re-download on next update
                    score: 0.8, // High score - safe to delete, will re-download
                    tags: vec!["vsix".to_string(), "cache".to_string()],
                    group: Some("VS Code Cached Extensions".to_string()),
                });
            }
        }
    }
    
    // 4. Scan logs directory
    let logs = vscode_user.parent()
        .map(|p| p.join("logs"));
    
    if let Some(logs) = logs {
        if logs.exists() {
            let size = calculate_dir_size(&logs);
            
            if size >= min_size_bytes {
                let metadata = fs::metadata(&logs).ok();
                let modified = metadata.as_ref()
                    .and_then(|m| m.modified().ok())
                    .map(chrono::DateTime::<chrono::Utc>::from);
                
                candidates.push(Candidate {
                    path: logs,
                    kind: TargetKind::VsCodeExtensionData,
                    action: Action::Delete,
                    size_bytes: size,
                    last_modified: modified,
                    last_accessed: None,
                    reproducibility: 1.0, // Logs are safe to delete
                    score: 0.8, // High score - logs are safe to delete
                    tags: vec!["logs".to_string()],
                    group: Some("VS Code Logs".to_string()),
                });
            }
        }
    }
    
    Ok(candidates)
}

/// Calculate total size of a directory recursively
fn calculate_dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| fs::metadata(e.path()).ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
