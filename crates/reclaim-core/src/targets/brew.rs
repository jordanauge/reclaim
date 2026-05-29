/// Homebrew cache — macOS only.
use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use crate::targets::dir_size;
use anyhow::Result;
use std::path::Path;

pub fn scan(_root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("brew_cache");
    if !config.enabled {
        return Ok(vec![]);
    }

    let cache_dir = dirs::home_dir()
        .map(|h| h.join("Library/Caches/Homebrew"))
        .ok_or_else(|| anyhow::anyhow!("cannot locate home dir"))?;

    if !cache_dir.exists() {
        return Ok(vec![]);
    }

    let size_bytes = dir_size(&cache_dir);
    if profile.should_skip_size(size_bytes) {
        return Ok(vec![]);
    }

    Ok(vec![Candidate {
        path: cache_dir,
        kind: TargetKind::BrewCache,
        size_bytes,
        last_modified: None,
        last_accessed: None,
        reproducibility: 1.0,
        score: 0.0,
        tags: vec!["cache".to_string(), "reproducible".to_string()],
        action: Action::Exec {
            cmd:         "brew".to_string(),
            args:        vec!["cleanup".to_string(), "--prune=all".to_string()],
            description: "brew cleanup --prune=all".to_string(),
        },
        group: Some("brew".to_string()),
    }])
}
