use anyhow::Result;
use chrono::Utc;
use crossbeam_channel::Sender;
use std::path::Path;
use std::fs;

use crate::selection::{CacheStatus, CandidateState};

/// Progress message from verification thread
#[derive(Debug, Clone)]
pub enum VerificationMessage {
    Progress {
        current: usize,
        total: usize,
        path: String,
    },
    Complete {
        verified: Vec<CandidateState>,
        stats: VerificationStats,
    },
    Error(String),
}

#[derive(Debug, Clone, Default)]
pub struct VerificationStats {
    pub total: usize,
    pub verified_unchanged: usize,
    pub changed: usize,
    pub unavailable: usize,
}

/// Tier 1 verifier - quickly checks known candidates using stat()
pub struct Tier1Verifier {
    progress_tx: Sender<VerificationMessage>,
}

impl Tier1Verifier {
    pub fn new(progress_tx: Sender<VerificationMessage>) -> Self {
        Self { progress_tx }
    }

    /// Verify a list of cached candidates
    pub fn verify_all(&self, mut candidates: Vec<CandidateState>) -> Result<Vec<CandidateState>> {
        let total = candidates.len();
        let mut stats = VerificationStats {
            total,
            ..Default::default()
        };

        for (idx, state) in candidates.iter_mut().enumerate() {
            // Send progress update every 10 items
            if idx % 10 == 0 {
                let _ = self.progress_tx.send(VerificationMessage::Progress {
                    current: idx + 1,
                    total,
                    path: state.candidate.path.display().to_string(),
                });
            }

            // Quick verification via stat
            match verify_candidate(state) {
                Ok(true) => {
                    // Unchanged
                    state.cache_status = CacheStatus::CachedVerified;
                    state.last_verified = Utc::now();
                    stats.verified_unchanged += 1;
                }
                Ok(false) => {
                    // Changed
                    state.cache_status = CacheStatus::Changed;
                    state.last_verified = Utc::now();
                    state.is_changed = true;
                    stats.changed += 1;
                }
                Err(_) => {
                    // Path no longer exists or inaccessible
                    state.cache_status = CacheStatus::Unknown;
                    stats.unavailable += 1;
                }
            }
        }

        let _ = self.progress_tx.send(VerificationMessage::Complete {
            verified: candidates.clone(),
            stats,
        });

        Ok(candidates)
    }
}

/// Quick verification of a single candidate via stat()
/// Returns Ok(true) if unchanged, Ok(false) if changed, Err if unavailable
fn verify_candidate(state: &mut CandidateState) -> Result<bool> {
    let path = &state.candidate.path;

    // Get current size via quick stat
    let current_size = get_quick_size(path)?;

    // Update current size
    state.size_current = Some(current_size);

    // Compare with cached size
    if let Some(cached_size) = state.size_cached {
        Ok(current_size == cached_size)
    } else {
        // No cached size, assume changed
        Ok(false)
    }
}

/// Get size quickly without full recursive walk
fn get_quick_size(path: &Path) -> Result<u64> {
    if path.is_file() {
        return Ok(fs::metadata(path)?.len());
    }

    // For directories: just get shallow size
    // This is fast but may not detect deep changes
    // Trade-off: speed vs accuracy
    let mut total = 0;

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;

        if meta.is_file() {
            total += meta.len();
        } else if meta.is_dir() {
            // Count dir as fixed block size (not recursive)
            total += 4096;
        }
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::{Action, Candidate, TargetKind};
    use std::path::PathBuf;

    fn make_candidate_state(path: &str, cached_size: u64) -> CandidateState {
        let candidate = Candidate {
            path: PathBuf::from(path),
            kind: TargetKind::Venv,
            size_bytes: cached_size,
            last_modified: None,
            last_accessed: None,
            reproducibility: 1.0,
            score: 0.5,
            tags: vec![],
            action: Action::Skip,
            group: None,
        };

        let mut state = CandidateState::new(candidate);
        state.size_cached = Some(cached_size);
        state.cache_status = CacheStatus::CachedUnverified;
        state
    }

    #[test]
    fn test_get_quick_size_file() {
        // Test on Cargo.toml which should exist in manifest dir
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let cargo_toml = manifest_dir.join("Cargo.toml");
        let size = get_quick_size(&cargo_toml).unwrap();
        assert!(size > 0);
    }

    #[test]
    fn test_get_quick_size_dir() {
        let path = std::path::Path::new(".");
        let size = get_quick_size(path).unwrap();
        assert!(size > 0);
    }
}
