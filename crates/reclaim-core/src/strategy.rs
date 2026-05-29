use crate::candidate::{Action, Candidate};
use crate::profile::Profile;

/// Score all candidates and assign default actions according to `profile`.
pub fn apply(candidates: &mut Vec<Candidate>, profile: &Profile) {
    for c in candidates.iter_mut() {
        c.score  = compute_score(c, profile);
        c.action = default_action(c);
        populate_tags(c, profile);
    }
}

/// Combined score: age × 0.4 + size × 0.3 + reproducibility × 0.3.
///
/// All components are normalised to [0.0, 1.0].
fn compute_score(c: &Candidate, profile: &Profile) -> f32 {
    let min_age = profile.min_age_for(c.kind.label()) as i64;

    let age_score: f32 = match c.age_days() {
        None       => 0.5,
        Some(days) => {
            if days < min_age {
                0.0
            } else {
                ((days - min_age) as f64 / 365.0).min(1.0) as f32
            }
        }
    };

    // Log-scale size score: ~100 MB → 0.5, ~10 GB → 1.0
    let size_score: f32 = if c.size_bytes == 0 {
        0.0
    } else {
        let mb = c.size_bytes as f64 / (1024.0 * 1024.0);
        (mb.ln() / 10_000_f64.ln()).clamp(0.0, 1.0) as f32
    };

    age_score * 0.4 + size_score * 0.3 + c.reproducibility * 0.3
}

/// Candidates with score ≥ 0.7 are pre-selected for deletion.
/// Exec candidates preserve whatever action the target module set.
fn default_action(c: &Candidate) -> Action {
    // Preserve command-based actions set by target modules.
    if matches!(c.action, Action::Exec { .. }) {
        return c.action.clone();
    }
    if c.score >= 0.7 {
        Action::Delete
    } else {
        Action::Skip
    }
}

fn populate_tags(c: &mut Candidate, profile: &Profile) {
    let min_age = profile.min_age_for(c.kind.label()) as i64;
    if let Some(days) = c.age_days() {
        if days > min_age * 2 {
            c.tags.push("old".to_string());
        }
    }
    if c.reproducibility >= 0.9 {
        c.tags.push("reproducible".to_string());
    }
    if c.size_bytes > 1_000_000_000 {
        c.tags.push("large".to_string());
    }
}
