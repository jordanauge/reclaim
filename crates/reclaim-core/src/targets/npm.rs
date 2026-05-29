use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use crate::targets::dir_size;
use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

pub fn scan(root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("npm");
    if !config.enabled {
        return Ok(vec![]);
    }

    let mut candidates = Vec::new();

    // Scan for node_modules/ directories inside repos.
    for entry in WalkDir::new(root).max_depth(5).follow_links(false) {
        let entry = match entry {
            Ok(e)  => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_dir() {
            continue;
        }
        if entry.file_name() != "node_modules" {
            continue;
        }

        let path = entry.into_path();

        // Skip nested node_modules (inside another node_modules).
        if path.ancestors().skip(1).any(|p| {
            p.file_name().map(|n| n == "node_modules").unwrap_or(false)
        }) {
            continue;
        }

        let size_bytes = dir_size(&path);
        if profile.should_skip_size(size_bytes) {
            continue;
        }

        let meta          = std::fs::metadata(&path).ok();
        let last_modified = meta.as_ref()
            .and_then(|m| m.modified().ok())
            .map(chrono::DateTime::<chrono::Utc>::from);
        let last_accessed = meta.as_ref()
            .and_then(|m| m.accessed().ok())
            .map(chrono::DateTime::<chrono::Utc>::from);

        let group = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string());

        candidates.push(Candidate {
            path,
            kind: TargetKind::NpmModules,
            size_bytes,
            last_modified,
            last_accessed,
            reproducibility: 0.98,
            score: 0.0,
            tags: vec!["reproducible".to_string()],
            action: Action::Skip,
            group,
        });
    }

    // Also surface the global npm cache (fixed location).
    if let Some(npm_cache) = npm_global_cache() {
        if npm_cache.exists() {
            let size_bytes = dir_size(&npm_cache);
            if !profile.should_skip_size(size_bytes) {
                candidates.push(Candidate {
                    path: npm_cache,
                    kind: TargetKind::NpmCache,
                    size_bytes,
                    last_modified: None,
                    last_accessed: None,
                    reproducibility: 1.0,
                    score: 0.0,
                    tags: vec!["cache".to_string(), "reproducible".to_string()],
                    action: Action::Skip,
                    group: Some("npm-global".to_string()),
                });
            }
        }
    }

    Ok(candidates)
}

fn npm_global_cache() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    return Some(home.join(".npm"));
    #[cfg(target_os = "windows")]
    return Some(home.join("AppData/Roaming/npm-cache"));
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return Some(home.join(".npm"));
}
