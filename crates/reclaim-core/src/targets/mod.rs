pub mod brew;
pub mod build;
pub mod docker;
pub mod logs;
pub mod npm;
pub mod pip;
pub mod venv;
pub mod vscode_chats;
pub mod vscode_workspace_storage;
pub mod cisco_logs;
pub mod browser_caches;
pub mod large_archives;
pub mod vscode_extensions;
pub mod large_files;
#[cfg(target_os = "macos")]
pub mod system_caches;
#[cfg(target_os = "macos")]
pub mod macos;

use std::path::Path;
use walkdir::WalkDir;

/// Sum the total byte size of all files under a directory tree.
/// Used by all target modules.
pub fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}
