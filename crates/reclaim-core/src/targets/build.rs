use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use crate::targets::dir_size;
use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

/// (directory name, TargetKind, reproducibility score)
const BUILD_PATTERNS: &[(&str, f32)] = &[
    ("target",       0.98), // Rust
    ("build",        0.97), // CMake, Gradle, generic
    ("dist",         0.95), // Python sdist/wheel, JS bundler output
    ("__pycache__",  1.00), // Python bytecode
    (".mypy_cache",  1.00), // mypy type-check cache
    (".pytest_cache",1.00), // pytest cache
    (".tox",         0.98), // tox test environments
    (".ruff_cache",  1.00), // ruff linter cache
];

pub fn scan(root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("build");
    if !config.enabled {
        return Ok(vec![]);
    }

    let mut candidates = Vec::new();

    for entry in WalkDir::new(root).max_depth(6).follow_links(false) {
        let entry = match entry {
            Ok(e)  => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        let Some(&(_dir_name, repro)) = BUILD_PATTERNS.iter().find(|(n, _)| *n == name.as_ref())
        else {
            continue;
        };

        let path = entry.into_path();

        // Skip if this looks like it lives inside another build dir (avoid double-counting).
        if path.ancestors().skip(1).any(|p| {
            p.file_name()
                .map(|n| BUILD_PATTERNS.iter().any(|(b, _)| *b == n.to_string_lossy().as_ref()))
                .unwrap_or(false)
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
            kind: TargetKind::BuildDir,
            size_bytes,
            last_modified,
            last_accessed,
            reproducibility: repro,
            score: 0.0,
            tags: vec!["reproducible".to_string()],
            action: Action::Skip,
            group,
        });
    }

    Ok(candidates)
}
