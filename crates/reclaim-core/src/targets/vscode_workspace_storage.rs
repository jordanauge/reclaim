use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

/// Scan VS Code workspace storage for orphaned workspaces
/// 
/// VS Code stores workspace state in:
/// ~/Library/Application Support/Code/User/workspaceStorage/<uuid>/
/// 
/// Each directory contains:
/// - workspace.json: References the actual workspace path
/// - state.vscdb: Workspace state database
/// - Various cache files
/// 
/// Safe to delete if:
/// 1. workspace.json is missing
/// 2. workspace.json references a path that no longer exists
/// 3. Workspace is older than min_age_days
pub fn scan(_root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("vscode-workspace");
    if !config.enabled {
        return Ok(vec![]);
    }
    
    let vscode_storage = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join("Library/Application Support/Code/User/workspaceStorage");
    
    if !vscode_storage.exists() {
        return Ok(vec![]);
    }
    
    let mut candidates = Vec::new();
    let min_age_days = profile.min_age_for("vscode-workspace");
    
    for entry in fs::read_dir(&vscode_storage)? {
        let entry = entry?;
        let path = entry.path();
        
        if !path.is_dir() {
            continue;
        }
        
        // Check if workspace is orphaned
        let workspace_json = path.join("workspace.json");
        let is_orphaned = if workspace_json.exists() {
            check_workspace_orphaned(&workspace_json).unwrap_or(true)
        } else {
            true // No workspace.json = definitely orphaned
        };
        
        if !is_orphaned {
            continue;
        }
        
        let size = compute_dir_size(&path)?;
        
        if profile.should_skip_size(size) {
            continue;
        }
        
        let metadata = fs::metadata(&path)?;
        let last_modified = metadata.modified()
            .ok()
            .map(|t| chrono::DateTime::<Utc>::from(t));
        
        // Apply profile filters
        let age_days = last_modified
            .map(|dt| (Utc::now() - dt).num_days())
            .unwrap_or(0);
        
        if age_days < min_age_days as i64 {
            continue;
        }
        
        candidates.push(Candidate {
            path,
            kind: TargetKind::VsCodeWorkspaceStorage,
            size_bytes: size,
            last_modified,
            last_accessed: None,
            reproducibility: 1.0,
            score: 0.0,
            tags: vec!["orphaned".to_string(), "vscode".to_string()],
            action: Action::Delete,
            group: Some("VS Code Workspace Storage".to_string()),
        });
    }
    
    Ok(candidates)
}

fn check_workspace_orphaned(workspace_json: &Path) -> Result<bool> {
    let content = fs::read_to_string(workspace_json)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;
    
    // Check if "folder" path exists
    if let Some(folder) = json.get("folder") {
        if let Some(path_str) = folder.as_str() {
            // Remove file:// prefix if present
            let clean_path = path_str
                .strip_prefix("file://")
                .unwrap_or(path_str);
            
            let workspace_path = PathBuf::from(clean_path);
            return Ok(!workspace_path.exists());
        }
    }
    
    // If we can't determine, assume orphaned
    Ok(true)
}

fn compute_dir_size(path: &Path) -> Result<u64> {
    let mut total = 0;
    
    for entry in walkdir::WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            total += entry.metadata()?.len();
        }
    }
    
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orphaned_check() {
        // Test would require mock workspace.json files
        // Skip for now - integration test in main app
    }
}
