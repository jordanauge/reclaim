use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The type of artifact this candidate represents.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetKind {
    Venv,
    BuildDir,
    PipCache,
    BrewCache,
    NpmModules,
    NpmCache,
    DockerImage,
    DockerVolume,
    DockerContainer,
    GradleCache,
    GoCache,
    VsCodeCache,
    VsCodeLogs,
    VsCodeWorkspaceStorage,
    VsCodeCppToolsCache,
    VsCodeExtensionData,
    CiscoLogs,
    PlaywrightCache,
    BrowserCache,
    LargeArchive,
    LargeFile,
    LogFiles,
    XcodeDerivedData,
    XcodeDeviceSupport,
    XcodeSimulators,
    Other(String),
}

impl TargetKind {
    /// Short human-readable label used in the TUI kind column.
    pub fn label(&self) -> &str {
        match self {
            Self::Venv            => "venv",
            Self::BuildDir        => "build",
            Self::PipCache        => "pip-cache",
            Self::BrewCache       => "brew-cache",
            Self::NpmModules      => "node_modules",
            Self::NpmCache        => "npm-cache",
            Self::DockerImage     => "docker-image",
            Self::DockerVolume    => "docker-volume",
            Self::DockerContainer => "docker-container",
            Self::GradleCache     => "gradle-cache",
            Self::GoCache         => "go-cache",
            Self::VsCodeCache     => "vscode-cache",
            Self::VsCodeLogs      => "vscode-logs",
            Self::VsCodeWorkspaceStorage => "vscode-workspace",
            Self::VsCodeCppToolsCache => "vscode-cpptools",
            Self::VsCodeExtensionData => "vscode-extensions",
            Self::CiscoLogs       => "cisco-logs",
            Self::PlaywrightCache => "playwright-cache",
            Self::BrowserCache    => "browser-cache",
            Self::LargeArchive    => "large-archive",
            Self::LargeFile       => "large-file",
            Self::LogFiles        => "logs",
            Self::XcodeDerivedData => "xcode-derived",
            Self::XcodeDeviceSupport => "xcode-devices",
            Self::XcodeSimulators => "xcode-simulators",
            Self::Other(s)        => s.as_str(),
        }
    }
}

/// What reclaim will do with this candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Action {
    /// Do nothing (shown in TUI but not selected).
    #[default]
    Skip,
    /// Remove the path entirely.
    Delete,
    /// Compress and archive before removing.
    Archive,
    /// Delegate cleanup to an external command (e.g. `brew cleanup --prune=all`).
    Exec {
        cmd:         String,
        args:        Vec<String>,
        /// Human-readable command string shown in the TUI/CLI.
        description: String,
    },
}

impl Action {
    /// Short label for table/CSV output.
    pub fn label(&self) -> &str {
        match self {
            Self::Skip    => "skip",
            Self::Delete  => "delete",
            Self::Archive => "archive",
            Self::Exec { .. } => "exec",
        }
    }

    /// Display string: shows the command for Exec actions.
    pub fn display(&self) -> String {
        match self {
            Self::Skip    => "skip".to_string(),
            Self::Delete  => "delete".to_string(),
            Self::Archive => "archive".to_string(),
            Self::Exec { description, .. } => format!("exec: {description}"),
        }
    }

    /// Whether this action is selected (Delete or Exec).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Delete | Self::Exec { .. })
    }
}

/// A single reclaimable artifact discovered on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub path:          PathBuf,
    pub kind:          TargetKind,
    pub size_bytes:    u64,
    pub last_modified: Option<DateTime<Utc>>,
    pub last_accessed: Option<DateTime<Utc>>,

    /// 0.0 (unique / risky) → 1.0 (fully reproducible with one command).
    pub reproducibility: f32,

    /// Combined cleanup score: 0.0 (keep) → 1.0 (safe to delete).
    /// Computed by `strategy::apply`.
    pub score: f32,

    /// Human-readable tags, e.g. `["inactive", "old", "reproducible"]`.
    pub tags: Vec<String>,

    /// Proposed action; may be overridden by the user in the TUI.
    pub action: Action,

    /// Optional group key for faceted grouping (e.g. parent repo name).
    pub group: Option<String>,
}

impl Candidate {
    /// Size formatted for display (e.g. "1.4 GB").
    pub fn size_human(&self) -> String {
        human_bytes(self.size_bytes)
    }

    /// Days since last access or modification (most recent wins).
    /// Returns `None` if neither timestamp is available.
    pub fn age_days(&self) -> Option<i64> {
        let ts = match (self.last_accessed, self.last_modified) {
            (Some(a), Some(m)) => Some(a.max(m)),
            (Some(a), None)    => Some(a),
            (None,    Some(m)) => Some(m),
            (None,    None)    => None,
        }?;
        Some((Utc::now() - ts).num_days())
    }
}

/// Format a byte count as a human-readable string.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit  = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit  += 1;
    }
    format!("{:.1} {}", value, UNITS[unit])
}
