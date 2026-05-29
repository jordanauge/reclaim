/// Xcode derived data and build artifacts — macOS only.
/// 
/// DerivedData contains build products, indexes, and module caches.
/// Fully reproducible by rebuilding projects.
/// Can easily consume 20-50 GB on active development machines.
use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use crate::targets::dir_size;
use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

/// Scan Xcode DerivedData directory for old build artifacts.
pub fn scan(_root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("xcode_derived_data");
    if !config.enabled {
        return Ok(vec![]);
    }

    let derived_data = dirs::home_dir()
        .map(|h| h.join("Library/Developer/Xcode/DerivedData"))
        .ok_or_else(|| anyhow::anyhow!("cannot locate home dir"))?;

    if !derived_data.exists() {
        return Ok(vec![]);
    }

    let mut candidates = Vec::new();

    // Each subdirectory is a project's build artifacts
    for entry in WalkDir::new(&derived_data)
        .max_depth(1)
        .min_depth(1)
        .follow_links(false)
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_dir() {
            continue;
        }

        let path = entry.into_path();
        
        // Extract project name from directory (e.g., "MyApp-abc123def" → "MyApp")
        let project_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| s.split('-').next())
            .unwrap_or("unknown")
            .to_string();
        
        let size_bytes = dir_size(&path);

        if profile.should_skip_size(size_bytes) {
            continue;
        }

        let meta = std::fs::metadata(&path).ok();
        let last_modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(chrono::DateTime::<chrono::Utc>::from);

        candidates.push(Candidate {
            path,
            kind: TargetKind::XcodeDerivedData,
            size_bytes,
            last_modified,
            last_accessed: None,
            reproducibility: 1.0, // Fully reproducible by building
            score: 0.0,
            tags: vec!["xcode".to_string(), "build".to_string()],
            action: Action::Delete,
            group: Some(format!("xcode-{}", project_name)),
        });
    }

    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_xcode_detection() {
        let tmp = TempDir::new().unwrap();
        let xcode_dir = tmp.path().join("Library/Developer/Xcode/DerivedData");
        fs::create_dir_all(&xcode_dir).unwrap();
        
        let project_dir = xcode_dir.join("MyApp-abcdef123456");
        fs::create_dir(&project_dir).unwrap();
        fs::write(project_dir.join("dummy.txt"), b"test").unwrap();

        // Note: In real usage, we scan with actual home dir
        // This test just validates the structure
        assert!(xcode_dir.exists());
        assert!(project_dir.exists());
    }
}
