use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use crate::targets::dir_size;
use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

/// Folder names that identify a Python virtual environment root.
const VENV_NAMES: &[&str] = &[".venv", "venv", ".env"];

/// Presence of any of these files confirms the directory is actually a venv.
const VENV_MARKERS: &[&str] = &["pyvenv.cfg", "bin/python", "bin/python3", "Scripts/python.exe"];

/// Scan `root` recursively (up to depth 5) for Python virtual environments.
pub fn scan(root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("venv");
    if !config.enabled {
        return Ok(vec![]);
    }

    let mut candidates = Vec::new();

    for entry in WalkDir::new(root).max_depth(5).follow_links(false) {
        let entry = match entry {
            Ok(e)  => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        if !VENV_NAMES.contains(&name.as_ref()) {
            continue;
        }

        let path = entry.into_path();

        // Confirm it is actually a venv.
        if !VENV_MARKERS.iter().any(|m| path.join(m).exists()) {
            continue;
        }

        let meta          = std::fs::metadata(&path).ok();
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

        // Group key: the immediate parent directory name (= repo name).
        let group = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string());

        candidates.push(Candidate {
            path,
            kind: TargetKind::Venv,
            size_bytes,
            last_modified,
            last_accessed,
            reproducibility: 0.95,
            score: 0.0, // set later by strategy::apply
            tags: vec!["reproducible".to_string()],
            action: Action::Skip,
            group,
        });
    }

    Ok(candidates)
}
