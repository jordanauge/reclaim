use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use crate::targets::dir_size;
use anyhow::Result;
use std::path::Path;

/// Pip cache lives in a fixed system location, not under a scanned root.
pub fn scan(_root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("pip_cache");
    if !config.enabled {
        return Ok(vec![]);
    }

    let cache_dir = pip_cache_dir().ok_or_else(|| anyhow::anyhow!("cannot locate pip cache"))?;
    if !cache_dir.exists() {
        return Ok(vec![]);
    }

    let size_bytes = dir_size(&cache_dir);
    if profile.should_skip_size(size_bytes) {
        return Ok(vec![]);
    }

    let meta          = std::fs::metadata(&cache_dir).ok();
    let last_modified = meta.as_ref()
        .and_then(|m| m.modified().ok())
        .map(chrono::DateTime::<chrono::Utc>::from);

    Ok(vec![Candidate {
        path: cache_dir,
        kind: TargetKind::PipCache,
        size_bytes,
        last_modified,
        last_accessed: None,
        reproducibility: 1.0,
        score: 0.0,
        tags: vec!["cache".to_string(), "reproducible".to_string()],
        action: Action::Exec {
            cmd:         "pip".to_string(),
            args:        vec!["cache".to_string(), "purge".to_string()],
            description: "pip cache purge".to_string(),
        },
        group: Some("pip".to_string()),
    }])
}

fn pip_cache_dir() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    #[cfg(target_os = "macos")]
    return Some(home.join("Library/Caches/pip"));
    #[cfg(target_os = "linux")]
    return Some(home.join(".cache/pip"));
    #[cfg(target_os = "windows")]
    return Some(home.join("AppData/Local/pip/Cache"));
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return Some(home.join(".cache/pip"));
}
