use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::Path;

/// Scan web browser caches
/// 
/// Browsers cache downloaded resources (images, scripts, fonts, etc.)
/// These can grow to multiple GB over time
/// Safe to delete - browsers will redownload on next use
/// 
/// Supported browsers:
/// - Google Chrome/Chromium
/// - Mozilla Firefox
/// - Safari
/// - Microsoft Edge
pub fn scan(_root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("browser-cache");
    if !config.enabled {
        return Ok(vec![]);
    }
    
    let min_age_days = profile.min_age_for("browser-cache");
    
    let caches = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join("Library/Caches");
    
    if !caches.exists() {
        return Ok(vec![]);
    }
    
    let browsers = vec![
        ("Google", "Chrome/Chromium"),
        ("Mozilla", "Mozilla"),
        ("Firefox", "Firefox"),
        ("com.apple.Safari", "Safari"),
        ("com.microsoft.Edge", "Microsoft Edge"),
        ("BraveSoftware", "Brave"),
    ];
    
    let mut candidates = Vec::new();
    
    for (dir_name, browser_name) in browsers {
        let cache_dir = caches.join(dir_name);
        
        if !cache_dir.exists() {
            continue;
        }
        
        let size = compute_dir_size(&cache_dir)?;
        
        if profile.should_skip_size(size) {
            continue;
        }
        
        let metadata = fs::metadata(&cache_dir)?;
        let last_modified = metadata.modified()
            .ok()
            .map(|t| chrono::DateTime::<Utc>::from(t));
        
        let age_days = last_modified
            .map(|dt| (Utc::now() - dt).num_days())
            .unwrap_or(0);
        
        if age_days < min_age_days as i64 {
            continue;
        }
        
        candidates.push(Candidate {
            path: cache_dir,
            kind: TargetKind::BrowserCache,
            size_bytes: size,
            last_modified,
            last_accessed: None,
            reproducibility: 1.0,
            score: 0.0,
            tags: vec!["browser".to_string(), "web".to_string(), browser_name.to_lowercase().replace(" ", "-")],
            action: Action::Delete,
            group: Some(format!("{} Cache", browser_name)),
        });
    }
    
    Ok(candidates)
}

fn compute_dir_size(path: &Path) -> Result<u64> {
    let mut total = 0;
    
    for entry in walkdir::WalkDir::new(path)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok()) // Skip permission errors silently
    {
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                total += metadata.len();
            }
        }
    }
    
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_detection() {
        // Would require mock cache directories
        // Integration test in main app
    }
}
