/// macOS system cache directories — each surfaced as a Candidate with either
/// Action::Delete (plain directory) or Action::Exec (dedicated cleanup command).
use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use crate::targets::dir_size;
use anyhow::Result;
use std::path::Path;

struct CacheEntry {
    /// Path relative to home directory.
    rel_path:    &'static str,
    kind:        TargetKindTag,
    profile_key: &'static str,
    /// Optional command-based cleanup.  None = plain FS delete.
    exec:        Option<ExecSpec>,
    /// How reproducible / safe this cache is to remove.
    repro:       f32,
}

#[derive(Clone, Copy)]
enum TargetKindTag {
    GoCache,
    VsCodeCache,
    VsCodeLogs,
    PlaywrightCache,
    BrowserCache,
}

impl TargetKindTag {
    fn into_kind(self) -> TargetKind {
        match self {
            Self::GoCache         => TargetKind::GoCache,
            Self::VsCodeCache     => TargetKind::VsCodeCache,
            Self::VsCodeLogs      => TargetKind::VsCodeLogs,
            Self::PlaywrightCache => TargetKind::PlaywrightCache,
            Self::BrowserCache    => TargetKind::BrowserCache,
        }
    }
}

struct ExecSpec {
    cmd:         &'static str,
    args:        &'static [&'static str],
    description: &'static str,
}

const CACHES: &[CacheEntry] = &[
    CacheEntry {
        rel_path:    "Library/Caches/go-build",
        kind:        TargetKindTag::GoCache,
        profile_key: "go_cache",
        exec:        Some(ExecSpec { cmd: "go", args: &["clean", "-cache"], description: "go clean -cache" }),
        repro:       1.0,
    },
    CacheEntry {
        rel_path:    "Library/Caches/vscode-cpptools",
        kind:        TargetKindTag::VsCodeCache,
        profile_key: "vscode_cache",
        exec:        None,
        repro:       1.0,
    },
    CacheEntry {
        rel_path:    "Library/Caches/ms-playwright",
        kind:        TargetKindTag::PlaywrightCache,
        profile_key: "playwright_cache",
        exec:        Some(ExecSpec {
            cmd:         "npx",
            args:        &["playwright", "uninstall", "--all"],
            description: "npx playwright uninstall --all",
        }),
        repro: 1.0,
    },
    CacheEntry {
        rel_path:    "Library/Caches/Google",
        kind:        TargetKindTag::BrowserCache,
        profile_key: "browser_cache",
        exec:        None,
        repro:       1.0,
    },
    CacheEntry {
        rel_path:    "Library/Application Support/Code/logs",
        kind:        TargetKindTag::VsCodeLogs,
        profile_key: "vscode_logs",
        exec:        None,
        repro:       1.0,
    },
];

pub fn scan(_root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot find home dir"))?;
    let mut candidates = Vec::new();

    for entry in CACHES {
        let config = profile.target_config(entry.profile_key);
        if !config.enabled {
            continue;
        }

        let path = home.join(entry.rel_path);
        if !path.exists() {
            continue;
        }

        let size_bytes = dir_size(&path);
        if profile.should_skip_size(size_bytes) {
            continue;
        }

        let meta          = std::fs::metadata(&path).ok();
        let last_modified = meta.as_ref()
            .and_then(|m| m.modified().ok())
            .map(chrono::DateTime::<chrono::Utc>::from);

        let action = match &entry.exec {
            Some(spec) => Action::Exec {
                cmd:         spec.cmd.to_string(),
                args:        spec.args.iter().map(|s| s.to_string()).collect(),
                description: spec.description.to_string(),
            },
            None => Action::Skip, // strategy::apply will promote to Delete based on score
        };

        candidates.push(Candidate {
            path,
            kind: entry.kind.into_kind(),
            size_bytes,
            last_modified,
            last_accessed: None,
            reproducibility: entry.repro,
            score: 0.0,
            tags: vec!["cache".to_string(), "reproducible".to_string()],
            action,
            group: Some("system-cache".to_string()),
        });
    }

    Ok(candidates)
}
