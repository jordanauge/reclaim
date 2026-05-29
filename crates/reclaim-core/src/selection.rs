use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::candidate::Candidate;

/// Cache verification status for progressive scanning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheStatus {
    /// Never seen before or outside cached roots
    Unknown,
    /// In cache, hasn't been verified this session yet
    CachedUnverified,
    /// Verified this session - size matches cached value
    CachedVerified,
    /// Verified this session - size changed from cache
    Changed,
    /// Discovered this session, not in previous cache
    New,
}

impl Default for CacheStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

impl CacheStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            Self::Unknown => "⚪ ?",
            Self::CachedUnverified => "🟡 ~",
            Self::CachedVerified => "🟢 ✓",
            Self::Changed => "🟠 Δ",
            Self::New => "🔵 N",
        }
    }
    
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::CachedVerified | Self::Changed | Self::New)
    }
    
    pub fn is_estimation(&self) -> bool {
        matches!(self, Self::CachedUnverified | Self::Unknown)
    }
}

/// How a selection state was determined
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionMode {
    /// System-controlled based on score (default behavior)
    Auto,
    /// User explicitly set, persists across rescans
    Manual,
    /// Mixed state (used only for groups with inconsistent children)
    Mixed,
}

impl Default for SelectionMode {
    fn default() -> Self {
        Self::Auto
    }
}

impl SelectionMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "A",
            Self::Manual => "M",
            Self::Mixed => "-",
        }
    }
}

/// Checkbox state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionState {
    Unchecked,
    Checked,
    /// Indeterminate (used for groups with mixed children)
    Indeterminate,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::Unchecked
    }
}

impl SelectionState {
    pub fn is_checked(&self) -> bool {
        matches!(self, Self::Checked)
    }

    pub fn is_unchecked(&self) -> bool {
        matches!(self, Self::Unchecked)
    }

    pub fn is_indeterminate(&self) -> bool {
        matches!(self, Self::Indeterminate)
    }
}

/// Enhanced candidate with selection tracking
#[derive(Debug, Clone)]
pub struct CandidateState {
    pub candidate: Candidate,
    pub selection_mode: SelectionMode,
    pub selection_state: SelectionState,
    
    // Cache & verification status
    pub cache_status: CacheStatus,
    pub size_cached: Option<u64>,      // Size at last full scan
    pub size_current: Option<u64>,     // Size at last verification (may be estimate)
    pub last_verified: DateTime<Utc>,  // Last Tier 1 check
    
    // Discovery tracking
    pub is_new: bool,
    pub is_changed: bool,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

impl CandidateState {
    /// Create a new CandidateState from a Candidate
    pub fn new(candidate: Candidate) -> Self {
        let now = Utc::now();
        let size = candidate.size_bytes;
        
        // Auto-select based on score
        let selection_state = if candidate.score >= 0.7 {
            SelectionState::Checked
        } else {
            SelectionState::Unchecked
        };

        Self {
            candidate,
            selection_mode: SelectionMode::Auto,
            selection_state,
            cache_status: CacheStatus::New,
            size_cached: Some(size),
            size_current: Some(size),
            last_verified: now,
            is_new: true,
            is_changed: false,
            first_seen: now,
            last_seen: now,
        }
    }

    /// Create from existing cache entry
    pub fn from_cached(
        candidate: Candidate,
        mode: SelectionMode,
        state: SelectionState,
        cache_status: CacheStatus,
        size_cached: Option<u64>,
        is_new: bool,
        is_changed: bool,
        first_seen: DateTime<Utc>,
        last_verified: DateTime<Utc>,
    ) -> Self {
        let now = Utc::now();
        let size_current = candidate.size_bytes;
        Self {
            candidate,
            selection_mode: mode,
            selection_state: state,
            cache_status,
            size_cached,
            size_current: Some(size_current),
            last_verified,
            is_new,
            is_changed,
            first_seen,
            last_seen: now,
        }
    }

    /// Toggle selection (user action -> Manual mode)
    pub fn toggle(&mut self) {
        self.selection_state = match self.selection_state {
            SelectionState::Checked => SelectionState::Unchecked,
            SelectionState::Unchecked | SelectionState::Indeterminate => SelectionState::Checked,
        };
        self.selection_mode = SelectionMode::Manual;
    }

    /// Set checked state (user action -> Manual mode)
    pub fn set_checked(&mut self, checked: bool) {
        self.selection_state = if checked {
            SelectionState::Checked
        } else {
            SelectionState::Unchecked
        };
        self.selection_mode = SelectionMode::Manual;
    }

    /// Reset to auto mode (recalculate based on score)
    pub fn reset_to_auto(&mut self) {
        self.selection_mode = SelectionMode::Auto;
        self.selection_state = if self.candidate.score >= 0.7 {
            SelectionState::Checked
        } else {
            SelectionState::Unchecked
        };
    }

    /// Check if this should be included in bulk operations
    pub fn is_selected(&self) -> bool {
        self.selection_state.is_checked()
    }
}

/// Compute aggregate state for a group of candidates
pub fn compute_group_state(children: &[CandidateState]) -> (SelectionState, SelectionMode) {
    if children.is_empty() {
        return (SelectionState::Unchecked, SelectionMode::Auto);
    }

    let checked_count = children
        .iter()
        .filter(|c| c.selection_state.is_checked())
        .count();
    let manual_count = children
        .iter()
        .filter(|c| c.selection_mode == SelectionMode::Manual)
        .count();

    let state = if checked_count == 0 {
        SelectionState::Unchecked
    } else if checked_count == children.len() {
        SelectionState::Checked
    } else {
        SelectionState::Indeterminate
    };

    let mode = if manual_count == 0 {
        SelectionMode::Auto
    } else if manual_count == children.len() {
        SelectionMode::Manual
    } else {
        SelectionMode::Mixed
    };

    (state, mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::{Action, TargetKind};
    use std::path::PathBuf;

    fn make_candidate(score: f64) -> Candidate {
        Candidate {
            path: PathBuf::from("/test"),
            kind: TargetKind::Venv,
            size_bytes: 1000,
            last_modified: None,
            last_accessed: None,
            reproducibility: 1.0,
            score,
            tags: vec![],
            action: Action::Skip,
            group: None,
        }
    }

    #[test]
    fn test_auto_selection_by_score() {
        let low_score = CandidateState::new(make_candidate(0.3));
        assert!(!low_score.is_selected());
        assert_eq!(low_score.selection_mode, SelectionMode::Auto);

        let high_score = CandidateState::new(make_candidate(0.9));
        assert!(high_score.is_selected());
        assert_eq!(high_score.selection_mode, SelectionMode::Auto);
    }

    #[test]
    fn test_toggle_becomes_manual() {
        let mut state = CandidateState::new(make_candidate(0.3));
        assert!(!state.is_selected());

        state.toggle();
        assert!(state.is_selected());
        assert_eq!(state.selection_mode, SelectionMode::Manual);
    }

    #[test]
    fn test_reset_to_auto() {
        let mut state = CandidateState::new(make_candidate(0.3));
        state.toggle(); // Now Manual + Checked

        state.reset_to_auto();
        assert!(!state.is_selected()); // Score 0.3 -> Unchecked
        assert_eq!(state.selection_mode, SelectionMode::Auto);
    }

    #[test]
    fn test_group_state_all_checked() {
        let children: Vec<CandidateState> = vec![
            CandidateState::new(make_candidate(0.9)),
            CandidateState::new(make_candidate(0.8)),
        ];
        let (state, mode) = compute_group_state(&children);
        assert_eq!(state, SelectionState::Checked);
        assert_eq!(mode, SelectionMode::Auto);
    }

    #[test]
    fn test_group_state_mixed() {
        let mut child1 = CandidateState::new(make_candidate(0.9)); // Auto + Checked
        let mut child2 = CandidateState::new(make_candidate(0.3)); // Auto + Unchecked
        child2.toggle(); // Manual + Checked

        let children = vec![child1, child2];
        let (state, mode) = compute_group_state(&children);
        assert_eq!(state, SelectionState::Checked); // Both checked
        assert_eq!(mode, SelectionMode::Mixed); // One auto, one manual
    }

    #[test]
    fn test_group_state_indeterminate() {
        let child1 = CandidateState::new(make_candidate(0.9)); // Checked
        let child2 = CandidateState::new(make_candidate(0.3)); // Unchecked

        let children = vec![child1, child2];
        let (state, mode) = compute_group_state(&children);
        assert_eq!(state, SelectionState::Indeterminate);
        assert_eq!(mode, SelectionMode::Auto);
    }
}
