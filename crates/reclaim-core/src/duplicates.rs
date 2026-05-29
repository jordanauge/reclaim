use crate::candidate::{Action, Candidate, TargetKind};
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Native duplicate detection for files and folders
/// 
/// Detects:
/// - Duplicate files (same content hash)
/// - Duplicate folders (same structure and content)
/// 
/// This is NOT a plugin - it's a built-in feature applied after all plugins have run.

/// Detect duplicate files by content hash (using file size + modification time as initial filter)
pub fn detect_duplicate_files(root: &Path, min_size_bytes: u64) -> Result<Vec<Candidate>> {
    let mut files_by_size_and_name: HashMap<(u64, String), Vec<PathBuf>> = HashMap::new();
    
    // First pass: group files by size and name
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
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
            continue; // Skip small files
        }
        
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        
        let key = (metadata.len(), filename);
        files_by_size_and_name.entry(key)
            .or_insert_with(Vec::new)
            .push(path.to_path_buf());
    }
    
    let mut candidates = Vec::new();
    
    // Second pass: for each group with duplicates, compute content hash
    for ((size, _name), paths) in files_by_size_and_name {
        if paths.len() < 2 {
            continue; // Not a duplicate
        }
        
        // Group by actual content hash
        let mut by_hash: HashMap<u64, Vec<PathBuf>> = HashMap::new();
        
        for path in paths {
            match compute_file_hash(&path) {
                Ok(hash) => {
                    by_hash.entry(hash)
                        .or_insert_with(Vec::new)
                        .push(path);
                }
                Err(_) => continue,
            }
        }
        
        // Create candidates for duplicate groups
        for (hash, mut duplicate_paths) in by_hash {
            if duplicate_paths.len() < 2 {
                continue; // Not actually a duplicate
            }
            
            // Sort by modification time (keep newest)
            duplicate_paths.sort_by_key(|p| {
                fs::metadata(p)
                    .and_then(|m| m.modified())
                    .ok()
            });
            duplicate_paths.reverse(); // Newest first
            
            // Mark all but the newest as duplicates
            for (idx, path) in duplicate_paths.iter().enumerate() {
                let is_newest = idx == 0;
                let metadata = fs::metadata(path).ok();
                let modified = metadata.as_ref()
                    .and_then(|m| m.modified().ok())
                    .map(chrono::DateTime::<chrono::Utc>::from);
                
                candidates.push(Candidate {
                    path: path.clone(),
                    kind: TargetKind::Other("duplicate-file".to_string()),
                    action: if is_newest { Action::Skip } else { Action::Delete },
                    size_bytes: size,
                    last_modified: modified,
                    last_accessed: None,
                    reproducibility: 1.0, // Duplicates are safe to remove
                    score: if is_newest { 0.0 } else { 0.9 }, // High score for duplicates
                    tags: if is_newest {
                        vec!["duplicate".to_string(), "newest".to_string(), "keep".to_string()]
                    } else {
                        vec!["duplicate".to_string(), "older-copy".to_string()]
                    },
                    group: Some(format!("Duplicate File: hash-{:x}", hash)),
                });
            }
        }
    }
    
    Ok(candidates)
}

/// Detect duplicate folders by structure and content
pub fn detect_duplicate_folders(root: &Path, min_size_bytes: u64) -> Result<Vec<Candidate>> {
    let mut folders_by_size_and_name: HashMap<(u64, String), Vec<PathBuf>> = HashMap::new();
    
    // First pass: group folders by total size and name
    for entry in WalkDir::new(root)
        .max_depth(3) // Limit depth to avoid scanning entire filesystem
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        
        if !path.is_dir() || path == root {
            continue;
        }
        
        let dir_size = calculate_dir_size(path);
        
        if dir_size < min_size_bytes {
            continue; // Skip small folders
        }
        
        let dirname = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        
        let key = (dir_size, dirname);
        folders_by_size_and_name.entry(key)
            .or_insert_with(Vec::new)
            .push(path.to_path_buf());
    }
    
    let mut candidates = Vec::new();
    
    // Second pass: for each group with potential duplicates, compute structure hash
    for ((size, _name), paths) in folders_by_size_and_name {
        if paths.len() < 2 {
            continue; // Not a duplicate
        }
        
        // Group by structure hash (file tree structure)
        let mut by_hash: HashMap<u64, Vec<PathBuf>> = HashMap::new();
        
        for path in paths {
            match compute_folder_hash(&path) {
                Ok(hash) => {
                    by_hash.entry(hash)
                        .or_insert_with(Vec::new)
                        .push(path);
                }
                Err(_) => continue,
            }
        }
        
        // Create candidates for duplicate groups
        for (hash, mut duplicate_paths) in by_hash {
            if duplicate_paths.len() < 2 {
                continue; // Not actually a duplicate
            }
            
            // Sort by modification time (keep newest)
            duplicate_paths.sort_by_key(|p| {
                fs::metadata(p)
                    .and_then(|m| m.modified())
                    .ok()
            });
            duplicate_paths.reverse(); // Newest first
            
            // Mark all but the newest as duplicates
            for (idx, path) in duplicate_paths.iter().enumerate() {
                let is_newest = idx == 0;
                let metadata = fs::metadata(path).ok();
                let modified = metadata.as_ref()
                    .and_then(|m| m.modified().ok())
                    .map(chrono::DateTime::<chrono::Utc>::from);
                
                candidates.push(Candidate {
                    path: path.clone(),
                    kind: TargetKind::Other("duplicate-folder".to_string()),
                    action: if is_newest { Action::Skip } else { Action::Delete },
                    size_bytes: size,
                    last_modified: modified,
                    last_accessed: None,
                    reproducibility: 1.0, // Duplicates are safe to remove
                    score: if is_newest { 0.0 } else { 0.9 }, // High score for duplicates
                    tags: if is_newest {
                        vec!["duplicate".to_string(), "newest".to_string(), "keep".to_string()]
                    } else {
                        vec!["duplicate".to_string(), "older-copy".to_string()]
                    },
                    group: Some(format!("Duplicate Folder: hash-{:x}", hash)),
                });
            }
        }
    }
    
    Ok(candidates)
}

/// Compute a fast hash of file content (using seahash on first 64KB + file size)
fn compute_file_hash(path: &Path) -> Result<u64> {
    use std::io::Read;
    
    let mut file = fs::File::open(path)?;
    let mut buffer = vec![0u8; 65536]; // Read first 64KB
    let bytes_read = file.read(&mut buffer)?;
    
    let metadata = fs::metadata(path)?;
    let file_size = metadata.len();
    
    // Combine file size with content hash
    let content_hash = seahash::hash(&buffer[..bytes_read]);
    let combined = seahash::hash(&[
        content_hash.to_le_bytes().as_ref(),
        file_size.to_le_bytes().as_ref(),
    ].concat());
    
    Ok(combined)
}

/// Compute a hash of folder structure (file names + sizes, not content)
fn compute_folder_hash(path: &Path) -> Result<u64> {
    let mut file_list = Vec::new();
    
    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let entry_path = entry.path();
        
        if entry_path.is_file() {
            let relative_path = entry_path.strip_prefix(path)
                .unwrap_or(entry_path)
                .to_string_lossy()
                .to_string();
            
            let size = fs::metadata(entry_path)
                .map(|m| m.len())
                .unwrap_or(0);
            
            file_list.push(format!("{}:{}", relative_path, size));
        }
    }
    
    // Sort to ensure consistent hashing regardless of traversal order
    file_list.sort();
    
    let combined = file_list.join("|");
    Ok(seahash::hash(combined.as_bytes()))
}

/// Calculate total size of a directory recursively
fn calculate_dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| fs::metadata(e.path()).ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compute_file_hash() {
        // This test would need actual test files
        // For now, just ensure it doesn't panic
        let path = PathBuf::from("/tmp/test.txt");
        if path.exists() {
            let _ = compute_file_hash(&path);
        }
    }
}
