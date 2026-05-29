use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use crate::targets::dir_size;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Quick scan using system indexers (Spotlight on macOS, locate on Linux)
/// Returns results immediately for known reclaimable patterns.
/// This is much faster than walking the filesystem but may miss some items.
pub fn quick_scan(roots: &[PathBuf], profile: &Profile) -> Result<Vec<Candidate>> {
    #[cfg(target_os = "macos")]
    {
        quick_scan_macos(roots, profile)
    }
    
    #[cfg(target_os = "linux")]
    {
        quick_scan_linux(roots, profile)
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        // Fallback: no quick scan available
        Ok(vec![])
    }
}

#[cfg(target_os = "macos")]
fn quick_scan_macos(roots: &[PathBuf], profile: &Profile) -> Result<Vec<Candidate>> {
    let mut all_candidates = Vec::new();
    
    // Common patterns to search for using Spotlight
    let patterns = vec![
        ("node_modules", TargetKind::NpmModules),
        (".venv", TargetKind::Venv),
        ("venv", TargetKind::Venv),
        ("target", TargetKind::BuildDir), // Rust
        ("build", TargetKind::BuildDir),
        ("dist", TargetKind::BuildDir),
        (".next", TargetKind::BuildDir), // Next.js
        (".nuxt", TargetKind::BuildDir), // Nuxt
        ("__pycache__", TargetKind::BuildDir), // Python
    ];
    
    for root in roots {
        for (pattern, kind) in &patterns {
            // Use mdfind (Spotlight) to find directories quickly
            let output = Command::new("mdfind")
                .arg("-onlyin")
                .arg(root)
                .arg(format!("kMDItemFSName == '{}'", pattern))
                .output();
            
            if let Ok(output) = output {
                if output.status.success() {
                    let paths = String::from_utf8_lossy(&output.stdout);
                    for line in paths.lines() {
                        let path = PathBuf::from(line.trim());
                        if !path.exists() {
                            continue;
                        }
                        
                        // Verify it's actually a directory
                        if !path.is_dir() {
                            continue;
                        }
                        
                        // Get metadata
                        let meta = std::fs::metadata(&path).ok();
                        let last_modified = meta.as_ref()
                            .and_then(|m| m.modified().ok())
                            .map(chrono::DateTime::<chrono::Utc>::from);
                        let last_accessed = meta.as_ref()
                            .and_then(|m| m.accessed().ok())
                            .map(chrono::DateTime::<chrono::Utc>::from);
                        
                        let size_bytes = dir_size(&path);
                        if profile.should_skip_size(size_bytes) {
                            continue;
                        }
                        
                        // Group by parent directory name
                        let group = path
                            .parent()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string());
                        
                        all_candidates.push(Candidate {
                            path: path.clone(),
                            kind: kind.clone(),
                            size_bytes,
                            last_modified,
                            last_accessed,
                            reproducibility: 0.9,
                            score: 0.0,
                            tags: vec!["quick-scan".to_string(), "reproducible".to_string()],
                            action: Action::Skip,
                            group,
                        });
                    }
                }
            }
        }
    }
    
    // Deduplicate by path
    all_candidates.sort_by(|a, b| a.path.cmp(&b.path));
    all_candidates.dedup_by(|a, b| a.path == b.path);
    
    Ok(all_candidates)
}

#[cfg(target_os = "linux")]
fn quick_scan_linux(roots: &[PathBuf], profile: &Profile) -> Result<Vec<Candidate>> {
    let mut all_candidates = Vec::new();
    
    // Try to use locate first (faster), fall back to find
    let use_locate = Command::new("which")
        .arg("locate")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    
    let patterns = vec![
        ("node_modules", TargetKind::NpmModules),
        (".venv", TargetKind::Venv),
        ("venv", TargetKind::Venv),
        ("target", TargetKind::BuildDir),
        ("build", TargetKind::BuildDir),
        ("dist", TargetKind::BuildDir),
        ("__pycache__", TargetKind::BuildDir),
    ];
    
    for root in roots {
        for (pattern, kind) in &patterns {
            let output = if use_locate {
                // Use locate for speed
                Command::new("locate")
                    .arg("--regex")
                    .arg(format!("{}/.*/{}", root.display(), pattern))
                    .output()
            } else {
                // Fallback to find (slower but more reliable)
                Command::new("find")
                    .arg(root)
                    .arg("-type")
                    .arg("d")
                    .arg("-name")
                    .arg(pattern)
                    .arg("-maxdepth")
                    .arg("5")
                    .output()
            };
            
            if let Ok(output) = output {
                if output.status.success() {
                    let paths = String::from_utf8_lossy(&output.stdout);
                    for line in paths.lines() {
                        let path = PathBuf::from(line.trim());
                        if !path.exists() || !path.is_dir() {
                            continue;
                        }
                        
                        let meta = std::fs::metadata(&path).ok();
                        let last_modified = meta.as_ref()
                            .and_then(|m| m.modified().ok())
                            .map(chrono::DateTime::<chrono::Utc>::from);
                        let last_accessed = meta.as_ref()
                            .and_then(|m| m.accessed().ok())
                            .map(chrono::DateTime::<chrono::Utc>::from);
                        
                        let size_bytes = dir_size(&path);
                        if profile.should_skip_size(size_bytes) {
                            continue;
                        }
                        
                        let group = path
                            .parent()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string());
                        
                        all_candidates.push(Candidate {
                            path: path.clone(),
                            kind: kind.clone(),
                            size_bytes,
                            last_modified,
                            last_accessed,
                            reproducibility: 0.9,
                            score: 0.0,
                            tags: vec!["quick-scan".to_string(), "reproducible".to_string()],
                            action: Action::Skip,
                            group,
                        });
                    }
                }
            }
        }
    }
    
    all_candidates.sort_by(|a, b| a.path.cmp(&b.path));
    all_candidates.dedup_by(|a, b| a.path == b.path);
    
    Ok(all_candidates)
}
