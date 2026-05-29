use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Scan for large archives, duplicates, and split archives
/// 
/// Detects:
/// - Duplicate archives (same base name, different versions/copies)
/// - Split archives (.part-000, .part-001, etc.)
/// - Large standalone archives for manual review
/// 
/// Archive formats: .zip, .tar, .tar.gz, .tgz, .tar.bz2, .tbz2, .tar.xz, .txz, .7z, .rar
pub fn scan(root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("large-archives");
    if !config.enabled {
        return Ok(vec![]);
    }
    
    let min_size_bytes = profile.min_size_bytes.unwrap_or(100 * 1024 * 1024); // Default 100MB
    
    let archive_extensions = vec![
        "zip", "tar", "gz", "tgz", "bz2", "tbz2", "xz", "txz", "7z", "rar",
        "tar.gz", "tar.bz2", "tar.xz",
    ];
    
    let mut archives: HashMap<String, Vec<ArchiveInfo>> = HashMap::new();
    let mut split_archives: HashMap<String, Vec<PathBuf>> = HashMap::new();
    
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        
        if !path.is_file() {
            continue;
        }
        
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        
        // Check if it's an archive
        let is_archive = archive_extensions.iter().any(|ext| {
            filename.to_lowercase().ends_with(&format!(".{}", ext))
        });
        
        if !is_archive {
            continue;
        }
        
        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        
        if metadata.len() < min_size_bytes {
            continue;
        }
        
        let modified = metadata.modified().ok()
            .map(chrono::DateTime::<chrono::Utc>::from);
        
        // Detect split archives (.part-000, .part-001, etc.)
        if let Some(base_name) = detect_split_archive(filename) {
            split_archives.entry(base_name.to_string())
                .or_insert_with(Vec::new)
                .push(path.to_path_buf());
            continue;
        }
        
        // Group potential duplicates by base name (strip version numbers, dates, etc.)
        let base_name = extract_base_name(filename);
        
        archives.entry(base_name)
            .or_insert_with(Vec::new)
            .push(ArchiveInfo {
                path: path.to_path_buf(),
                size: metadata.len(),
                modified,
            });
    }
    
    let mut candidates = Vec::new();
    
    // Process split archives
    for (base_name, parts) in split_archives {
        if parts.len() < 2 {
            continue; // Not really split if only one part
        }
        
        let total_size: u64 = parts.iter()
            .filter_map(|p| fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        
        let oldest_modified = parts.iter()
            .filter_map(|p| fs::metadata(p).ok()
                .and_then(|m| m.modified().ok())
                .map(chrono::DateTime::<chrono::Utc>::from))
            .min();
        
        // Use the parent directory as the target (group of split files)
        if let Some(parent) = parts.first().and_then(|p| p.parent()) {
            candidates.push(Candidate {
                path: parent.to_path_buf(),
                kind: TargetKind::LargeArchive,
                action: Action::Skip,
                size_bytes: total_size,
                last_modified: oldest_modified,
                last_accessed: None,
                reproducibility: 0.3, // Split archives may be needed
                score: 0.6, // Medium score - requires review
                tags: vec!["split-archive".to_string(), "review".to_string()],
                group: Some(format!("Split Archive: {}", base_name)),
            });
        }
    }
    
    // Process potential duplicate archives
    for (base_name, mut infos) in archives {
        if infos.len() < 2 {
            continue; // Not a duplicate if only one
        }
        
        // Sort by modification time (newest first)
        infos.sort_by(|a, b| {
            b.modified.cmp(&a.modified)
        });
        
        let total_size: u64 = infos.iter().map(|i| i.size).sum();
        let oldest = infos.last().and_then(|i| i.modified);
        
        // Mark all but the newest as potential duplicates
        for (idx, info) in infos.iter().enumerate() {
            let is_newest = idx == 0;
            
            candidates.push(Candidate {
                path: info.path.clone(),
                kind: TargetKind::LargeArchive,
                action: if is_newest { Action::Skip } else { Action::Delete },
                size_bytes: info.size,
                last_modified: info.modified,
                last_accessed: None,
                reproducibility: 0.9, // Duplicates are safe to remove (except newest)
                score: if is_newest { 0.4 } else { 0.7 }, // Higher score for older duplicates
                tags: if is_newest {
                    vec!["duplicate".to_string(), "newest".to_string(), "review".to_string()]
                } else {
                    vec!["duplicate".to_string(), "older-copy".to_string()]
                },
                group: Some(format!("Duplicate Archive: {}", base_name)),
            });
        }
    }
    
    Ok(candidates)
}

#[derive(Debug, Clone)]
struct ArchiveInfo {
    path: PathBuf,
    size: u64,
    modified: Option<chrono::DateTime<chrono::Utc>>,
}

/// Detect split archive pattern (.part-000, .part-001, etc.)
fn detect_split_archive(filename: &str) -> Option<&str> {
    // Patterns: file.zip.part-000, file.tar.gz.part-001
    if let Some(idx) = filename.rfind(".part-") {
        if idx > 0 {
            // Check if followed by digits
            let suffix = &filename[idx + 6..];
            if suffix.chars().all(|c| c.is_ascii_digit()) {
                return Some(&filename[..idx]);
            }
        }
    }
    None
}

/// Extract base name from archive filename (strip version, date, etc.)
fn extract_base_name(filename: &str) -> String {
    let mut name = filename.to_lowercase();
    
    // Remove extensions
    let exts = vec![
        ".tar.gz", ".tar.bz2", ".tar.xz",
        ".zip", ".tgz", ".tbz2", ".txz", ".7z", ".rar", ".tar",
    ];
    for ext in exts {
        if name.ends_with(ext) {
            name = name[..name.len() - ext.len()].to_string();
            break;
        }
    }
    
    // Remove common patterns: version numbers, dates, "copy", "backup", etc.
    name = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-' || c == '_').to_string();
    
    // Remove trailing parentheses like " (1)", " (copy)"
    if let Some(idx) = name.rfind('(') {
        name = name[..idx].trim_end().to_string();
    }
    
    name
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_split_archive() {
        assert_eq!(detect_split_archive("file.zip.part-000"), Some("file.zip"));
        assert_eq!(detect_split_archive("archive.tar.gz.part-001"), Some("archive.tar.gz"));
        assert_eq!(detect_split_archive("file.zip"), None);
    }
    
    #[test]
    fn test_extract_base_name() {
        assert_eq!(extract_base_name("file-v1.2.3.zip"), "file-v");
        assert_eq!(extract_base_name("backup-2026-05-28.tar.gz"), "backup");
        assert_eq!(extract_base_name("file (1).zip"), "file");
        assert_eq!(extract_base_name("archive_copy.tgz"), "archive_copy");
    }
}
