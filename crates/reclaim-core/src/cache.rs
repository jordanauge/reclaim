use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::candidate::Candidate;
use crate::selection::{CacheStatus, CandidateState, SelectionMode, SelectionState};

/// Result of quick metadata check (Tier 1)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuickCheckResult {
    /// Likely unchanged based on mtime (fast path)
    LikelyUnchanged,
    /// Needs deeper verification
    NeedsVerification,
}

/// Metadata for quick change detection
#[derive(Debug, Clone)]
pub struct DirectoryMetadata {
    pub mtime: SystemTime,
    pub child_count: usize,
    pub total_size: u64,
}

/// Compute shallow hash of directory (Tier 2)
/// Only hashes direct children names + sizes, not recursive
pub fn compute_shallow_hash(path: &Path) -> Result<String> {
    use std::fs;
    
    if !path.is_dir() {
        // For files: hash size + mtime
        let meta = fs::metadata(path)?;
        let hash_input = format!("{}-{}", 
            meta.len(),
            meta.modified()
                .ok()
                .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0)
        );
        return Ok(format!("{:016x}", seahash::hash(hash_input.as_bytes())));
    }
    
    // For directories: hash direct children only (not recursive)
    let mut entries: Vec<_> = fs::read_dir(path)?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|e| e.path());
    
    let mut hash_input = String::new();
    for entry in entries {
        let name = entry.file_name();
        hash_input.push_str(&name.to_string_lossy());
        hash_input.push('|');
        
        if let Ok(meta) = entry.metadata() {
            hash_input.push_str(&meta.len().to_string());
            hash_input.push('|');
            if let Ok(mtime) = meta.modified() {
                if let Ok(duration) = mtime.duration_since(SystemTime::UNIX_EPOCH) {
                    hash_input.push_str(&duration.as_secs().to_string());
                }
            }
        }
        hash_input.push('\n');
    }
    
    Ok(format!("{:016x}", seahash::hash(hash_input.as_bytes())))
}

/// Persistent cache for scan results and user selections
pub struct ScanCache {
    conn: Connection,
}

impl ScanCache {
    /// Open or create cache database at given path
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)
            .context(format!("Failed to open cache at {}", path.display()))?;

        let mut cache = Self { conn };
        cache.init_schema()?;
        Ok(cache)
    }

    /// Open cache in default location (~/.cache/reclaim/scan-cache.db)
    pub fn open_default() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".cache")))
            .context("Cannot determine cache directory")?;
        let db_path = cache_dir.join("reclaim").join("scan-cache.db");
        Self::open(&db_path)
    }

    /// Initialize database schema
    fn init_schema(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS cached_entries (
                path TEXT PRIMARY KEY,
                content_hash TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                mtime INTEGER NOT NULL,
                first_seen INTEGER NOT NULL,
                last_seen INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_last_seen ON cached_entries(last_seen);

            CREATE TABLE IF NOT EXISTS user_selections (
                path TEXT PRIMARY KEY,
                is_checked BOOLEAN NOT NULL,
                selection_mode TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY (path) REFERENCES cached_entries(path)
            );

            CREATE TABLE IF NOT EXISTS scan_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                started_at INTEGER NOT NULL,
                completed_at INTEGER,
                roots TEXT NOT NULL,
                profile_name TEXT NOT NULL,
                items_found INTEGER,
                new_items INTEGER,
                changed_items INTEGER
            );
            "#,
        )?;
        Ok(())
    }

    /// Compute content hash for a path (fast, based on metadata only)
    pub fn content_hash(path: &Path) -> Result<String> {
        let meta = std::fs::metadata(path)?;
        let mtime = meta
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let size = meta.len();

        // Fast hash without reading file content
        let hash_input = format!("{}-{}", mtime, size);
        let hash = seahash::hash(hash_input.as_bytes());
        Ok(format!("{:016x}", hash))
    }

    /// Get cached entry for a path
    pub fn get_cached_entry(&self, path: &Path) -> Result<Option<CachedEntry>> {
        let path_str = path.to_string_lossy();
        let mut stmt = self.conn.prepare_cached(
            "SELECT content_hash, size_bytes, mtime, first_seen, last_seen 
             FROM cached_entries WHERE path = ?",
        )?;

        let entry = stmt
            .query_row(params![path_str.as_ref()], |row| {
                Ok(CachedEntry {
                    path: path.to_path_buf(),
                    content_hash: row.get(0)?,
                    size_bytes: row.get(1)?,
                    mtime: row.get(2)?,
                    first_seen: Utc.timestamp_opt(row.get(3)?, 0).single().unwrap(),
                    last_seen: Utc.timestamp_opt(row.get(4)?, 0).single().unwrap(),
                })
            })
            .optional()?;

        Ok(entry)
    }

    /// Get user selection for a path
    pub fn get_user_selection(&self, path: &Path) -> Result<Option<UserSelection>> {
        let path_str = path.to_string_lossy();
        let mut stmt = self.conn.prepare_cached(
            "SELECT is_checked, selection_mode, timestamp 
             FROM user_selections WHERE path = ?",
        )?;

        let selection = stmt
            .query_row(params![path_str.as_ref()], |row| {
                let is_checked: bool = row.get(0)?;
                let mode_str: String = row.get(1)?;
                let timestamp: i64 = row.get(2)?;

                Ok(UserSelection {
                    path: path.to_path_buf(),
                    is_checked,
                    selection_mode: match mode_str.as_str() {
                        "manual" => SelectionMode::Manual,
                        _ => SelectionMode::Auto,
                    },
                    timestamp: Utc.timestamp_opt(timestamp, 0).single().unwrap(),
                })
            })
            .optional()?;

        Ok(selection)
    }

    /// Update or insert cached entry
    pub fn upsert_cached_entry(&mut self, entry: &CachedEntry) -> Result<()> {
        let path_str = entry.path.to_string_lossy();
        self.conn.execute(
            "INSERT OR REPLACE INTO cached_entries 
             (path, content_hash, size_bytes, mtime, first_seen, last_seen)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                path_str.as_ref(),
                &entry.content_hash,
                entry.size_bytes,
                entry.mtime,
                entry.first_seen.timestamp(),
                entry.last_seen.timestamp(),
            ],
        )?;
        Ok(())
    }

    /// Update or insert user selection
    pub fn upsert_user_selection(&mut self, selection: &UserSelection) -> Result<()> {
        let path_str = selection.path.to_string_lossy();
        let mode_str = match selection.selection_mode {
            SelectionMode::Manual => "manual",
            _ => "auto",
        };
        self.conn.execute(
            "INSERT OR REPLACE INTO user_selections 
             (path, is_checked, selection_mode, timestamp)
             VALUES (?, ?, ?, ?)",
            params![
                path_str.as_ref(),
                selection.is_checked,
                mode_str,
                selection.timestamp.timestamp(),
            ],
        )?;
        Ok(())
    }

    /// Process candidates from a new scan and merge with cache
    pub fn merge_scan_results(
        &mut self,
        candidates: Vec<Candidate>,
    ) -> Result<(Vec<CandidateState>, ScanStats)> {
        let now = Utc::now();
        let mut states = Vec::new();
        let mut stats = ScanStats::default();

        for candidate in candidates {
            stats.items_found += 1;

            // Compute current hash
            let current_hash = Self::content_hash(&candidate.path).unwrap_or_default();

            // Check cache
            let cached = self.get_cached_entry(&candidate.path)?;
            let user_sel = self.get_user_selection(&candidate.path)?;

            let (is_new, is_changed, first_seen) = match cached {
                Some(ref cached_entry) => {
                    let changed = cached_entry.content_hash != current_hash;
                    if changed {
                        stats.changed_items += 1;
                    }
                    (false, changed, cached_entry.first_seen)
                }
                None => {
                    stats.new_items += 1;
                    (true, false, now)
                }
            };

            // Create state
            let state = if let Some(sel) = user_sel {
                // Restore user selection
                let selection_state = if sel.is_checked {
                    SelectionState::Checked
                } else {
                    SelectionState::Unchecked
                };
                
                let cache_status = if is_changed {
                    CacheStatus::Changed
                } else if is_new {
                    CacheStatus::New
                } else {
                    CacheStatus::CachedUnverified
                };
                
                CandidateState::from_cached(
                    candidate.clone(),
                    sel.selection_mode,
                    selection_state,
                    cache_status,
                    cached.as_ref().map(|c| c.size_bytes),
                    is_new,
                    is_changed,
                    first_seen,
                    now,
                )
            } else {
                // New or auto-selected
                let mut s = CandidateState::new(candidate.clone());
                s.is_new = is_new;
                s.is_changed = is_changed;
                s.first_seen = first_seen;
                s.cache_status = if is_changed {
                    CacheStatus::Changed
                } else if is_new {
                    CacheStatus::New
                } else {
                    CacheStatus::CachedUnverified
                };
                s
            };

            // Update cache entry
            let entry = CachedEntry {
                path: candidate.path.clone(),
                content_hash: current_hash,
                size_bytes: candidate.size_bytes,
                mtime: 0, // We don't track mtime separately in Candidate
                first_seen,
                last_seen: now,
            };
            self.upsert_cached_entry(&entry)?;

            states.push(state);
        }

        Ok((states, stats))
    }

    /// Save all user selections from current state
    pub fn save_user_selections(&mut self, states: &[CandidateState]) -> Result<()> {
        let now = Utc::now();
        for state in states {
            if state.selection_mode == SelectionMode::Manual {
                let selection = UserSelection {
                    path: state.candidate.path.clone(),
                    is_checked: state.selection_state.is_checked(),
                    selection_mode: state.selection_mode,
                    timestamp: now,
                };
                self.upsert_user_selection(&selection)?;
            }
        }
        Ok(())
    }

    /// Purge old cache entries (not seen in last N days)
    pub fn purge_old_entries(&mut self, max_age_days: u32) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
        let cutoff_ts = cutoff.timestamp();

        let deleted = self.conn.execute(
            "DELETE FROM cached_entries WHERE last_seen < ?",
            params![cutoff_ts],
        )?;

        // Cascade delete orphaned user_selections
        self.conn.execute(
            "DELETE FROM user_selections 
             WHERE path NOT IN (SELECT path FROM cached_entries)",
            params![],
        )?;

        Ok(deleted)
    }

    /// Get total number of cached entries
    pub fn count_entries(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM cached_entries", params![], |row| {
                row.get(0)
            })?;
        Ok(count as usize)
    }

    /// Get number of manual selections
    pub fn count_manual_selections(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM user_selections WHERE selection_mode = 'manual'",
            params![],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Load all cached entries as CandidateState for instant display
    /// This is used at startup to show cached results immediately
    /// Returns candidates with CachedUnverified status
    pub fn load_all_cached(&self) -> Result<Vec<CachedEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, content_hash, size_bytes, mtime, first_seen, last_seen 
             FROM cached_entries 
             ORDER BY size_bytes DESC 
             LIMIT 1000"  // Limit to top 1000 by size for performance
        )?;

        let entries = stmt
            .query_map(params![], |row| {
                let path_str: String = row.get(0)?;
                Ok(CachedEntry {
                    path: PathBuf::from(path_str),
                    content_hash: row.get(1)?,
                    size_bytes: row.get(2)?,
                    mtime: row.get(3)?,
                    first_seen: Utc.timestamp_opt(row.get(4)?, 0).single().unwrap(),
                    last_seen: Utc.timestamp_opt(row.get(5)?, 0).single().unwrap(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }
}

/// Cached metadata for a file/directory
#[derive(Debug, Clone)]
pub struct CachedEntry {
    pub path: PathBuf,
    pub content_hash: String,
    pub size_bytes: u64,
    pub mtime: u64,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

/// User selection record
#[derive(Debug, Clone)]
pub struct UserSelection {
    pub path: PathBuf,
    pub is_checked: bool,
    pub selection_mode: SelectionMode,
    pub timestamp: DateTime<Utc>,
}

/// Statistics from a scan merge operation
#[derive(Debug, Default, Clone)]
pub struct ScanStats {
    pub items_found: usize,
    pub new_items: usize,
    pub changed_items: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::{Action, TargetKind};

    fn make_candidate(path: &str, score: f64) -> Candidate {
        Candidate {
            path: PathBuf::from(path),
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
    fn test_cache_open_in_memory() {
        let cache = ScanCache::open(Path::new(":memory:"));
        assert!(cache.is_ok());
    }

    #[test]
    fn test_merge_new_items() {
        let mut cache = ScanCache::open(Path::new(":memory:")).unwrap();
        let candidates = vec![make_candidate("/test/venv1", 0.8)];

        let (states, stats) = cache.merge_scan_results(candidates).unwrap();

        assert_eq!(stats.items_found, 1);
        assert_eq!(stats.new_items, 1);
        assert_eq!(stats.changed_items, 0);
        assert_eq!(states.len(), 1);
        assert!(states[0].is_new);
        assert!(states[0].is_selected()); // Score 0.8 -> auto-selected
    }

    #[test]
    fn test_persist_manual_selection() {
        let mut cache = ScanCache::open(Path::new(":memory:")).unwrap();

        // First scan
        let candidates = vec![make_candidate("/test/venv1", 0.3)];
        let (mut states, _) = cache.merge_scan_results(candidates).unwrap();

        // User toggles it
        states[0].toggle();
        assert!(states[0].is_selected());
        assert_eq!(states[0].selection_mode, SelectionMode::Manual);

        // Save selection
        cache.save_user_selections(&states).unwrap();

        // Second scan - should restore Manual selection
        let candidates2 = vec![make_candidate("/test/venv1", 0.3)];
        let (states2, stats2) = cache.merge_scan_results(candidates2).unwrap();

        assert_eq!(stats2.new_items, 0); // Not new anymore
        assert!(states2[0].is_selected()); // Manual selection preserved
        assert_eq!(states2[0].selection_mode, SelectionMode::Manual);
    }
}
