use anyhow::Result;
use chrono::{DateTime, Utc};
use crossbeam_channel::Sender;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::candidate::Candidate;
use crate::plugins;
use crate::profile::Profile;
use crate::scanner;
use crate::selection::{CacheStatus, CandidateState};

/// Progress message from hot paths discovery
#[derive(Debug, Clone)]
pub enum DiscoveryMessage {
    Progress {
        current: String,
        paths_scanned: usize,
    },
    Complete {
        new_candidates: Vec<CandidateState>,
        stats: DiscoveryStats,
    },
    Error(String),
}

#[derive(Debug, Clone, Default)]
pub struct DiscoveryStats {
    pub paths_scanned: usize,
    pub new_items: usize,
    pub changed_items: usize,
}

/// Hot paths discoverer - finds new candidates without full rescan
pub struct HotPathsDiscoverer {
    progress_tx: Sender<DiscoveryMessage>,
    profile: Profile,
    known_paths: HashSet<PathBuf>,
}

impl HotPathsDiscoverer {
    pub fn new(progress_tx: Sender<DiscoveryMessage>, profile: Profile, known_paths: HashSet<PathBuf>) -> Self {
        Self {
            progress_tx,
            profile,
            known_paths,
        }
    }

    /// Discover new/changed candidates using hot paths heuristics
    pub fn discover(&self, roots: Vec<PathBuf>, last_scan: Option<DateTime<Utc>>) -> Result<Vec<CandidateState>> {
        let mut new_candidates = Vec::new();
        let mut stats = DiscoveryStats::default();

        // Strategy 1: Try system indexer (Spotlight/updatedb/Baloo)
        if let Some(since) = last_scan {
            if let Some(changed_dirs) = self.try_indexer_discovery(roots.clone(), since) {
                let _ = self.progress_tx.send(DiscoveryMessage::Progress {
                    current: format!("Found {} changed directories via system indexer", changed_dirs.len()),
                    paths_scanned: changed_dirs.len(),
                });

                // Scan changed directories
                for dir in changed_dirs {
                    stats.paths_scanned += 1;
                    if let Ok(candidates) = self.scan_path(&dir) {
                        for cand in candidates {
                            if !self.known_paths.contains(&cand.path) {
                                let mut state = CandidateState::new(cand);
                                state.cache_status = CacheStatus::New;
                                state.is_changed = true;
                                new_candidates.push(state);
                                stats.new_items += 1;
                            }
                        }
                    }
                }

                let _ = self.progress_tx.send(DiscoveryMessage::Complete {
                    new_candidates: new_candidates.clone(),
                    stats: stats.clone(),
                });

                return Ok(new_candidates);
            }
        }

        // Strategy 2: Fallback to hot paths heuristics
        let hot_paths = self.get_hot_paths(&roots);
        
        let _ = self.progress_tx.send(DiscoveryMessage::Progress {
            current: format!("Scanning {} hot paths", hot_paths.len()),
            paths_scanned: 0,
        });

        for hot_path in hot_paths {
            stats.paths_scanned += 1;
            
            let _ = self.progress_tx.send(DiscoveryMessage::Progress {
                current: hot_path.display().to_string(),
                paths_scanned: stats.paths_scanned,
            });

            if let Ok(candidates) = self.scan_path(&hot_path) {
                for cand in candidates {
                    if !self.known_paths.contains(&cand.path) {
                        let mut state = CandidateState::new(cand);
                        state.cache_status = CacheStatus::New;
                        state.is_changed = true;
                        new_candidates.push(state);
                        stats.new_items += 1;
                    }
                }
            }
        }

        let _ = self.progress_tx.send(DiscoveryMessage::Complete {
            new_candidates: new_candidates.clone(),
            stats: stats.clone(),
        });

        Ok(new_candidates)
    }

    /// Try system indexer for change detection
    fn try_indexer_discovery(&self, roots: Vec<PathBuf>, since: DateTime<Utc>) -> Option<Vec<PathBuf>> {
        for root in roots {
            if let Some(dirs) = plugins::detect_changed_dirs(&root, since) {
                return Some(dirs);
            }
        }
        
        None
    }

    /// Get list of hot paths likely to have new artifacts
    fn get_hot_paths(&self, roots: &[PathBuf]) -> Vec<PathBuf> {
        let mut hot_paths = Vec::new();

        // Common hot paths
        let common = vec![
            "Downloads",
            "Desktop",
            "Documents",
        ];

        // Development hot paths
        let dev = vec![
            "repos",
            "Projects",
            "Code",
            "Development",
            "git",
        ];

        // Cache hot paths
        let caches = vec![
            ".cache",
            "Library/Caches",
            "Library/Developer",
        ];

        let home = dirs::home_dir().unwrap_or_default();

        // Add common paths
        for name in common.iter().chain(dev.iter()).chain(caches.iter()) {
            let path = home.join(name);
            if path.exists() {
                hot_paths.push(path);
            }
        }

        // Add roots themselves (shallow scan)
        for root in roots {
            hot_paths.push(root.clone());
        }

        hot_paths
    }

    /// Scan a single path for candidates
    fn scan_path(&self, path: &Path) -> Result<Vec<Candidate>> {
        let roots = vec![path.to_path_buf()];
        let candidates = scanner::scan(&roots, &self.profile)?;
        Ok(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_hot_paths() {
        let profile = Profile::default();
        let (tx, _rx) = crossbeam_channel::unbounded();
        let known = HashSet::new();
        let discoverer = HotPathsDiscoverer::new(tx, profile, known);
        
        let roots = vec![PathBuf::from("/tmp")];
        let hot_paths = discoverer.get_hot_paths(&roots);
        
        assert!(!hot_paths.is_empty());
    }
}
