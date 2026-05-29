use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use crate::targets::dir_size;
use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

const LOG_DIR_NAMES: &[&str] = &["logs", "log", ".logs"];
const LOG_EXTENSIONS: &[&str] = &["log", "log.gz", "log.1", "log.2"];

pub fn scan(root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("logs");
    if !config.enabled {
        return Ok(vec![]);
    }

    let min_age = profile.min_age_for("logs") as i64;
    let mut candidates = Vec::new();

    for entry in WalkDir::new(root).max_depth(5).follow_links(false) {
        let entry = match entry {
            Ok(e)  => e,
            Err(_) => continue,
        };

        let path = entry.path().to_path_buf();
        let name = entry.file_name().to_string_lossy();

        let (size_bytes, kind) = if entry.file_type().is_dir()
            && LOG_DIR_NAMES.contains(&name.as_ref())
        {
            (dir_size(&path), TargetKind::LogFiles)
        } else if entry.file_type().is_file()
            && LOG_EXTENSIONS.iter().any(|ext| name.ends_with(ext))
        {
            let sz = entry.metadata().map(|m| m.len()).unwrap_or(0);
            (sz, TargetKind::LogFiles)
        } else {
            continue;
        };

        if profile.should_skip_size(size_bytes) {
            continue;
        }

        let meta          = std::fs::metadata(&path).ok();
        let last_modified = meta.as_ref()
            .and_then(|m| m.modified().ok())
            .map(chrono::DateTime::<chrono::Utc>::from);

        // Skip if younger than min_age.
        let age_ok = last_modified.map(|ts| {
            (chrono::Utc::now() - ts).num_days() >= min_age
        }).unwrap_or(true);
        if !age_ok {
            continue;
        }

        candidates.push(Candidate {
            path,
            kind,
            size_bytes,
            last_modified,
            last_accessed: None,
            reproducibility: 0.0, // logs are NOT reproducible
            score: 0.0,
            tags: vec!["logs".to_string()],
            action: Action::Skip,
            group: Some("logs".to_string()),
        });
    }

    Ok(candidates)
}
