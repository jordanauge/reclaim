use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Controls how a single target type behaves within a profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TargetConfig {
    /// Whether this target type is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Override the global `min_age_days` for this target.
    pub min_age_days: Option<u32>,

    /// Default action when score exceeds threshold: "delete" | "archive" | "skip".
    pub default_action: Option<String>,
}

fn default_true() -> bool { true }

/// A cleanup profile loaded from a TOML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name:        String,
    pub description: String,

    /// Minimum artifact age (in days) used for score normalization.
    #[serde(default = "default_min_age")]
    pub min_age_days: u32,

    /// Optional minimum size filter applied during scan for performance.
    /// If None, scan everything (filter in UI later).
    /// If Some(size), skip smaller artifacts during scan to save I/O.
    pub min_size_bytes: Option<u64>,

    /// Per-target configuration keyed by `TargetKind::label()`.
    #[serde(default)]
    pub targets: HashMap<String, TargetConfig>,

    /// Glob patterns for paths that must never be touched.
    #[serde(default)]
    pub exclude_paths: Vec<String>,
}

fn default_min_age()  -> u32 { 30 }

impl Profile {
    /// Load a profile from a TOML file.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let profile: Profile = toml::from_str(&content)?;
        Ok(profile)
    }

    /// Create a default conservative profile (used as fallback when no file exists)
    pub fn default_conservative() -> Self {
        let mut targets = HashMap::new();
        
        // Enable all common targets with conservative defaults
        for target in ["venv", "build", "npm", "logs", "docker", "large-archives", 
                       "large-files", "vscode-workspace", "vscode-extensions", 
                       "cisco-logs", "browser-caches", "pip-cache", "brew-cache"] {
            targets.insert(target.to_string(), TargetConfig {
                enabled: true,
                min_age_days: Some(30),
                default_action: Some("delete".to_string()),
            });
        }
        
        Self {
            name: "conservative".to_string(),
            description: "Safe cleanup of reproducible artifacts (built-in default)".to_string(),
            min_age_days: 30,
            min_size_bytes: Some(10 * 1024 * 1024), // 10 MB minimum
            targets,
            exclude_paths: vec![],
        }
    }

    /// Return the `TargetConfig` for a given kind label, with fallback defaults.
    pub fn target_config(&self, kind_label: &str) -> TargetConfig {
        self.targets.get(kind_label).cloned().unwrap_or_default()
    }

    /// Effective minimum age for a target kind (target override or global).
    pub fn min_age_for(&self, kind_label: &str) -> u32 {
        self.targets
            .get(kind_label)
            .and_then(|c| c.min_age_days)
            .unwrap_or(self.min_age_days)
    }

    /// Check if a size should be filtered during scan (for performance).
    /// Returns true if we should skip this artifact during scan.
    pub fn should_skip_size(&self, size_bytes: u64) -> bool {
        match self.min_size_bytes {
            Some(min) => size_bytes < min,
            None => false, // No size filter = scan everything
        }
    }
}
