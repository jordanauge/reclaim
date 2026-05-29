/// VS Code Copilot chat sessions — can grow very large (50K+ lines per session).
use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use crate::targets::dir_size;
use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

pub fn scan(_root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("vscode_chats");
    if !config.enabled {
        return Ok(vec![]);
    }

    let code_user = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot find home dir"))?
        .join("Library/Application Support/Code/User");

    if !code_user.exists() {
        return Ok(vec![]);
    }

    let mut candidates = Vec::new();

    // Scan workspaceStorage/*/chatSessions directories
    let workspace_storage = code_user.join("workspaceStorage");
    if workspace_storage.exists() {
        for entry in WalkDir::new(&workspace_storage)
            .max_depth(2)
            .follow_links(false)
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if !entry.file_type().is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy();
            if name != "chatSessions" {
                continue;
            }

            let path = entry.into_path();
            let size_bytes = dir_size(&path);

            if profile.should_skip_size(size_bytes) {
                continue;
            }

            // Count .jsonl files
            let file_count = std::fs::read_dir(&path)
                .ok()
                .map(|entries| entries.filter_map(|e| e.ok()).count())
                .unwrap_or(0);

            let meta = std::fs::metadata(&path).ok();
            let last_modified = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .map(chrono::DateTime::<chrono::Utc>::from);

            candidates.push(Candidate {
                path,
                kind: TargetKind::VsCodeCache,
                size_bytes,
                last_modified,
                last_accessed: None,
                reproducibility: 0.0, // Chat sessions are NOT reproducible
                score: 0.0,
                tags: vec![
                    "vscode".to_string(),
                    "copilot".to_string(),
                    format!("{} sessions", file_count),
                ],
                action: Action::Skip,
                group: Some("vscode-chats".to_string()),
            });
        }
    }

    Ok(candidates)
}
