use crate::candidate::Candidate;
use crate::profile::Profile;
use crate::targets;
use anyhow::Result;
use rayon::prelude::*;
use std::path::PathBuf;

/// Scan `roots` in parallel and collect all cleanup candidates matching `profile`.
pub fn scan(roots: &[PathBuf], profile: &Profile) -> Result<Vec<Candidate>> {
    let results: Vec<Result<Vec<Candidate>>> = roots
        .par_iter()
        .map(|root| scan_root(root, profile))
        .collect();

    let mut all = Vec::new();
    for result in results {
        match result {
            Ok(candidates) => all.extend(candidates),
            Err(e) => {
                // Log permission errors but continue scanning
                eprintln!("Warning: Scan error for root (continuing): {}", e);
            }
        }
    }
    
    // Deduplicate by path
    all.sort_by(|a, b| a.path.cmp(&b.path));
    all.dedup_by(|a, b| a.path == b.path);
    
    Ok(all)
}

fn scan_root(root: &PathBuf, profile: &Profile) -> Result<Vec<Candidate>> {
    let mut all = Vec::new();

    // Collect results from all targets, logging errors but continuing
    macro_rules! scan_target {
        ($target:expr) => {
            match $target {
                Ok(candidates) => all.extend(candidates),
                Err(e) => eprintln!("Warning: {} (continuing)", e),
            }
        };
    }

    scan_target!(targets::venv::scan(root, profile));
    scan_target!(targets::build::scan(root, profile));
    scan_target!(targets::npm::scan(root, profile));
    scan_target!(targets::logs::scan(root, profile));
    scan_target!(targets::docker::scan(root, profile));
    scan_target!(targets::large_archives::scan(root, profile));
    scan_target!(targets::large_files::scan(root, profile));

    // pip and brew use fixed cache dirs, not arbitrary roots —
    // only scan them once (when root is the home dir).
    if root == &dirs::home_dir().unwrap_or_default() {
        scan_target!(targets::pip::scan(root, profile));
        scan_target!(targets::vscode_chats::scan(root, profile));
        scan_target!(targets::vscode_workspace_storage::scan(root, profile));
        scan_target!(targets::vscode_extensions::scan(root, profile));
        scan_target!(targets::cisco_logs::scan(root, profile));
        scan_target!(targets::browser_caches::scan(root, profile));
        #[cfg(target_os = "macos")]
        {
            scan_target!(targets::brew::scan(root, profile));
            scan_target!(targets::system_caches::scan(root, profile));
            scan_target!(targets::macos::xcode::scan(root, profile));
        }
    }

    Ok(all)
}
