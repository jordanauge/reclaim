use crate::candidate::{human_bytes, Candidate, TargetKind};
use std::collections::HashMap;

/// Per-kind aggregated statistics.
#[derive(Debug, Default)]
pub struct KindSummary {
    pub count:      usize,
    pub size_bytes: u64,
}

/// A group of related candidates (faceted view, e.g. "all venvs in ~/repos/claris").
#[derive(Debug)]
pub struct Group {
    /// Display key (kind label, path prefix, etc.)
    pub key:        String,
    pub kind:       Option<TargetKind>,
    /// Indices into the flat `candidates` slice.
    pub indices:    Vec<usize>,
    pub size_bytes: u64,
}

/// Full scan report produced from a flat list of candidates.
#[derive(Debug, Default)]
pub struct Report {
    pub total_candidates:    usize,
    pub total_size_bytes:    u64,
    pub selected_size_bytes: u64,
    pub by_kind:             HashMap<String, KindSummary>,
    pub groups:              Vec<Group>,
}

impl Report {
    /// Build a report from scored candidates.
    pub fn build(candidates: &[Candidate]) -> Self {
        let mut report = Report {
            total_candidates: candidates.len(),
            ..Default::default()
        };

        for (i, c) in candidates.iter().enumerate() {
            report.total_size_bytes += c.size_bytes;
            if c.action.is_active() {
                report.selected_size_bytes += c.size_bytes;
            }

            let entry = report
                .by_kind
                .entry(c.kind.label().to_string())
                .or_default();
            entry.count      += 1;
            entry.size_bytes += c.size_bytes;

            // Group by `candidate.group` field when present.
            if let Some(key) = &c.group {
                if let Some(g) = report.groups.iter_mut().find(|g| &g.key == key) {
                    g.indices.push(i);
                    g.size_bytes += c.size_bytes;
                } else {
                    report.groups.push(Group {
                        key:        key.clone(),
                        kind:       Some(c.kind.clone()),
                        indices:    vec![i],
                        size_bytes: c.size_bytes,
                    });
                }
            }
        }

        report.groups.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        report
    }

    /// Pretty-print a one-line summary.
    pub fn summary_line(&self) -> String {
        format!(
            "{} candidates  |  {} total  |  {} selected",
            self.total_candidates,
            human_bytes(self.total_size_bytes),
            human_bytes(self.selected_size_bytes),
        )
    }
}
