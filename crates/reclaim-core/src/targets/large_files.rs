use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Scan for large files (>100MB) for manual review
/// 
/// This is a catch-all plugin to identify large files that don't fit
/// other categories. Useful for finding:
/// - Large media files (videos, ISOs, disk images)
/// - Database dumps
/// - Large datasets
/// - Forgotten downloads
/// 
/// All candidates are marked for Review (not automatic deletion)
pub fn scan(root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("large-files");
    if !config.enabled {
        return Ok(vec![]);
    }
    
    let min_size_bytes = profile.min_size_bytes
        .unwrap_or(100 * 1024 * 1024)
        .max(100 * 1024 * 1024); // At least 100MB
    
    let mut candidates = Vec::new();
    
    // Categories to skip (already handled by other plugins)
    let skip_extensions = vec![
        // Archives (large-archives plugin)
        "zip", "tar", "gz", "tgz", "bz2", "tbz2", "xz", "txz", "7z", "rar",
        // Build artifacts (existing plugins)
        "o", "obj", "a", "lib", "so", "dylib", "dll",
        // Logs (already handled)
        "log",
    ];
    
    let skip_dirs = vec![
        "node_modules",
        ".venv",
        "venv",
        ".git",
        "target",
        "build",
        "dist",
        "__pycache__",
        ".cache",
        "Cache",
        "Caches",
    ];
    
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            // Skip directories we don't want to scan
            if e.path().is_dir() {
                let dir_name = e.file_name().to_str().unwrap_or("");
                return !skip_dirs.iter().any(|skip| dir_name == *skip);
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        
        if !path.is_file() {
            continue;
        }
        
        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        
        if metadata.len() < min_size_bytes {
            continue;
        }
        
        // Skip files with extensions we handle elsewhere
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if skip_extensions.iter().any(|skip| ext.eq_ignore_ascii_case(skip)) {
                continue;
            }
        }
        
        let modified = metadata.modified().ok()
            .map(chrono::DateTime::<chrono::Utc>::from);
        
        // Categorize by file type
        let (file_category, score) = categorize_file(path);
        
        candidates.push(Candidate {
            path: path.to_path_buf(),
            kind: TargetKind::LargeFile,
            action: Action::Skip,
            size_bytes: metadata.len(),
            last_modified: modified,
            last_accessed: None,
            reproducibility: 0.0, // Unknown, requires manual review
            score,
            tags: vec!["large-file".to_string(), file_category.to_lowercase().replace(' ', "-"), "review".to_string()],
            group: Some(format!("Large File: {}", file_category)),
        });
    }
    
    Ok(candidates)
}

/// Categorize file by extension and assign appropriate score
fn categorize_file(path: &Path) -> (&'static str, f32) {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    match ext.as_str() {
        // Video files - medium score (might be in use)
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" => 
            ("Video", 0.5),
        
        // Disk images - higher score (often temporary)
        "iso" | "dmg" | "img" | "vdi" | "vmdk" | "qcow2" => 
            ("Disk Image", 0.7),
        
        // Database dumps - medium score
        "sql" | "dump" | "db" | "sqlite" | "sqlite3" => 
            ("Database", 0.5),
        
        // Data files - lower score (likely important)
        "csv" | "json" | "xml" | "parquet" | "feather" | "arrow" => 
            ("Dataset", 0.3),
        
        // Audio files - medium score
        "mp3" | "wav" | "flac" | "m4a" | "aac" | "ogg" | "wma" => 
            ("Audio", 0.5),
        
        // Compressed backup - higher score
        "bak" | "backup" | "old" | "tmp" => 
            ("Backup", 0.7),
        
        // PDF documents - lower score (likely important)
        "pdf" => 
            ("PDF Document", 0.3),
        
        // Firmware/binary - medium score
        "bin" | "fw" | "hex" | "elf" => 
            ("Binary/Firmware", 0.5),
        
        // Default - medium score
        _ => 
            ("Unknown", 0.5),
    }
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
    use std::path::PathBuf;
    
    #[test]
    fn test_categorize_file() {
        assert_eq!(categorize_file(&PathBuf::from("video.mp4")), ("Video", 0.5));
        assert_eq!(categorize_file(&PathBuf::from("ubuntu.iso")), ("Disk Image", 0.7));
        assert_eq!(categorize_file(&PathBuf::from("data.csv")), ("Dataset", 0.3));
        assert_eq!(categorize_file(&PathBuf::from("backup.bak")), ("Backup", 0.7));
        assert_eq!(categorize_file(&PathBuf::from("unknown.xyz")), ("Unknown", 0.5));
    }
}
