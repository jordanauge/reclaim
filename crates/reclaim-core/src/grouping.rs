use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::candidate::Candidate;

/// Represents a group of related candidates
#[derive(Debug, Clone)]
pub struct CandidateGroup {
    pub id: String,
    pub name: String,
    pub group_type: GroupType,
    pub candidates: Vec<usize>, // Indices into original candidate list
    pub total_size: u64,
    pub parent_path: Option<PathBuf>,
    pub common_ancestor: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupType {
    /// Files with identical content (same size + similar name)
    Duplicates,
    /// Files in the same directory
    SameDirectory,
    /// Files sharing a common ancestor directory
    CommonAncestor { depth: usize },
    /// Files with similar names (e.g., logs with timestamps)
    SimilarNames { pattern: String },
    /// Ungrouped single item
    Single,
}

/// Group candidates intelligently based on various criteria
pub fn group_candidates(candidates: &[Candidate]) -> Vec<CandidateGroup> {
    let mut groups = Vec::new();
    let mut processed = vec![false; candidates.len()];
    
    // Phase 1: Group by exact duplicates (same size + similar name)
    let duplicate_groups = find_duplicate_groups(candidates);
    for (_, indices) in duplicate_groups {
        if indices.len() > 1 {
            let total_size: u64 = indices.iter()
                .filter_map(|&i| candidates.get(i))
                .map(|c| c.size_bytes)
                .sum();
            
            let common_ancestor = find_common_ancestor(
                &indices.iter()
                    .filter_map(|&i| candidates.get(i).map(|c| c.path.as_path()))
                    .collect::<Vec<_>>()
            );
            
            let name = format!("Duplicates: {} ({} files)", 
                candidates[indices[0]].path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                indices.len()
            );
            
            groups.push(CandidateGroup {
                id: format!("dup_{}", indices[0]),
                name,
                group_type: GroupType::Duplicates,
                candidates: indices.clone(),
                total_size,
                parent_path: candidates.get(indices[0]).and_then(|c| c.path.parent().map(|p| p.to_path_buf())),
                common_ancestor,
            });
            
            for &idx in &indices {
                processed[idx] = true;
            }
        }
    }
    
    // Phase 2: Group by similar names (logs, timestamps, etc.)
    let similar_name_groups = find_similar_name_groups(candidates, &processed);
    for (pattern, indices) in similar_name_groups {
        if indices.len() > 1 {
            let total_size: u64 = indices.iter()
                .filter_map(|&i| candidates.get(i))
                .map(|c| c.size_bytes)
                .sum();
            
            let common_ancestor = find_common_ancestor(
                &indices.iter()
                    .filter_map(|&i| candidates.get(i).map(|c| c.path.as_path()))
                    .collect::<Vec<_>>()
            );
            
            groups.push(CandidateGroup {
                id: format!("similar_{}", pattern),
                name: format!("{} ({} files)", pattern, indices.len()),
                group_type: GroupType::SimilarNames { pattern },
                candidates: indices.clone(),
                total_size,
                parent_path: common_ancestor.clone(),
                common_ancestor,
            });
            
            for &idx in &indices {
                processed[idx] = true;
            }
        }
    }
    
    // Phase 3: Group by same directory
    let dir_groups = find_directory_groups(candidates, &processed);
    for (dir, indices) in dir_groups {
        if indices.len() > 1 {
            let total_size: u64 = indices.iter()
                .filter_map(|&i| candidates.get(i))
                .map(|c| c.size_bytes)
                .sum();
            
            groups.push(CandidateGroup {
                id: format!("dir_{}", dir.display()),
                name: format!("{} ({} items)", 
                    dir.file_name().unwrap_or_default().to_string_lossy(),
                    indices.len()
                ),
                group_type: GroupType::SameDirectory,
                candidates: indices.clone(),
                total_size,
                parent_path: dir.parent().map(|p| p.to_path_buf()),
                common_ancestor: Some(dir.clone()),
            });
            
            for &idx in &indices {
                processed[idx] = true;
            }
        }
    }
    
    // Phase 4: Remaining items as singles
    for (idx, &is_processed) in processed.iter().enumerate() {
        if !is_processed {
            if let Some(candidate) = candidates.get(idx) {
                groups.push(CandidateGroup {
                    id: format!("single_{}", idx),
                    name: candidate.path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    group_type: GroupType::Single,
                    candidates: vec![idx],
                    total_size: candidate.size_bytes,
                    parent_path: candidate.path.parent().map(|p| p.to_path_buf()),
                    common_ancestor: candidate.path.parent().map(|p| p.to_path_buf()),
                });
            }
        }
    }
    
    // Sort by total size (largest first)
    groups.sort_by(|a, b| b.total_size.cmp(&a.total_size));
    
    groups
}

/// Find groups of files with same size (potential duplicates)
fn find_duplicate_groups(candidates: &[Candidate]) -> HashMap<u64, Vec<usize>> {
    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    
    for (idx, candidate) in candidates.iter().enumerate() {
        by_size.entry(candidate.size_bytes)
            .or_insert_with(Vec::new)
            .push(idx);
    }
    
    // Filter to only groups with 2+ items and same size
    by_size.into_iter()
        .filter(|(_, indices)| indices.len() > 1)
        .collect()
}

/// Find groups of files with similar names (e.g., "Log.2024-01-01", "Log.2024-01-02")
fn find_similar_name_groups(candidates: &[Candidate], processed: &[bool]) -> HashMap<String, Vec<usize>> {
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    
    for (idx, candidate) in candidates.iter().enumerate() {
        if processed[idx] {
            continue;
        }
        
        let filename = candidate.path.file_name()
            .unwrap_or_default()
            .to_string_lossy();
        
        // Extract pattern by removing date/timestamp patterns
        let pattern = extract_name_pattern(&filename);
        
        if !pattern.is_empty() && pattern != filename {
            groups.entry(pattern)
                .or_insert_with(Vec::new)
                .push(idx);
        }
    }
    
    groups.into_iter()
        .filter(|(_, indices)| indices.len() > 1)
        .collect()
}

/// Extract common pattern from filename by removing dates, timestamps, numbers
fn extract_name_pattern(filename: &str) -> String {
    let mut pattern = String::new();
    let chars: Vec<char> = filename.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        let ch = chars[i];
        
        // Check for date pattern (4 digits followed by 2 digits twice: YYYY-MM-DD or YYYYMMDD)
        if ch.is_ascii_digit() && i + 7 < chars.len() {
            let slice: String = chars[i..i+8].iter().collect();
            if is_date_like(&slice) {
                pattern.push('*');
                i += 8;
                continue;
            }
        }
        
        // Check for time pattern (HH:MM:SS or HHMMSS)
        if ch.is_ascii_digit() && i + 5 < chars.len() {
            let slice: String = chars[i..i+6].iter().collect();
            if is_time_like(&slice) {
                pattern.push('*');
                i += 6;
                continue;
            }
        }
        
        // Check for sequence numbers (.001, .1, (1), -001, _001)
        if (ch == '.' || ch == '-' || ch == '_' || ch == '(') && i + 1 < chars.len() {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 {
                // We found digits after separator
                if j < chars.len() && (chars[j] == ')' || chars[j] == '.') {
                    j += 1; // Include closing bracket or dot
                }
                pattern.push('*');
                i = j;
                continue;
            }
        }
        
        pattern.push(ch);
        i += 1;
    }
    
    // Clean up multiple asterisks
    let mut cleaned = String::new();
    let mut last_was_star = false;
    for ch in pattern.chars() {
        if ch == '*' {
            if !last_was_star {
                cleaned.push(ch);
            }
            last_was_star = true;
        } else {
            cleaned.push(ch);
            last_was_star = false;
        }
    }
    
    cleaned
}

fn is_date_like(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 8 {
        return false;
    }
    
    // YYYY-MM-DD or YYYYMMDD
    chars[0].is_ascii_digit() && chars[1].is_ascii_digit() 
        && chars[2].is_ascii_digit() && chars[3].is_ascii_digit()
        && (chars[4] == '-' || chars[4].is_ascii_digit())
        && (chars[5].is_ascii_digit() || (chars[4] == '-' && chars[5].is_ascii_digit()))
}

fn is_time_like(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 6 {
        return false;
    }
    
    // HH:MM:SS or HHMMSS
    chars[0].is_ascii_digit() && chars[1].is_ascii_digit()
        && (chars[2] == ':' || chars[2].is_ascii_digit())
        && (chars[3].is_ascii_digit() || (chars[2] == ':' && chars[3].is_ascii_digit()))
}

/// Find groups of files in the same directory
fn find_directory_groups(candidates: &[Candidate], processed: &[bool]) -> HashMap<PathBuf, Vec<usize>> {
    let mut groups: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    
    for (idx, candidate) in candidates.iter().enumerate() {
        if processed[idx] {
            continue;
        }
        
        if let Some(parent) = candidate.path.parent() {
            groups.entry(parent.to_path_buf())
                .or_insert_with(Vec::new)
                .push(idx);
        }
    }
    
    groups.into_iter()
        .filter(|(_, indices)| indices.len() > 1)
        .collect()
}

/// Find common ancestor directory for a set of paths
pub fn find_common_ancestor(paths: &[&Path]) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }
    
    if paths.len() == 1 {
        return paths[0].parent().map(|p| p.to_path_buf());
    }
    
    // Start with first path's ancestors
    let mut ancestors: Vec<PathBuf> = paths[0]
        .ancestors()
        .map(|p| p.to_path_buf())
        .collect();
    
    // Filter to only ancestors common to all paths
    ancestors.retain(|ancestor| {
        paths.iter().all(|path| path.starts_with(ancestor))
    });
    
    // Return the deepest common ancestor (first in list)
    ancestors.first().cloned()
}

/// Get directory context for a path (parent, siblings, etc.)
#[derive(Debug, Clone)]
pub struct DirectoryContext {
    pub path: PathBuf,
    pub parent: Option<PathBuf>,
    pub siblings: Vec<PathBuf>,
    pub total_size_in_parent: u64,
    pub sibling_count: usize,
}

pub fn get_directory_context(path: &Path, all_candidates: &[Candidate]) -> DirectoryContext {
    let parent = path.parent().map(|p| p.to_path_buf());
    
    let siblings: Vec<PathBuf> = if let Some(ref parent_path) = parent {
        all_candidates.iter()
            .filter(|c| c.path.parent() == Some(parent_path.as_path()))
            .filter(|c| c.path != path)
            .map(|c| c.path.clone())
            .collect()
    } else {
        Vec::new()
    };
    
    let total_size_in_parent: u64 = if let Some(ref parent_path) = parent {
        all_candidates.iter()
            .filter(|c| c.path.parent() == Some(parent_path.as_path()))
            .map(|c| c.size_bytes)
            .sum()
    } else {
        0
    };
    
    DirectoryContext {
        path: path.to_path_buf(),
        parent,
        siblings: siblings.clone(),
        total_size_in_parent,
        sibling_count: siblings.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_name_pattern() {
        // Pattern extraction: dates, numbers get replaced with *
        // Note: separators and sequences are replaced together
        assert_eq!(extract_name_pattern("log.2024-01-01.txt"), "log*txt");
        assert_eq!(extract_name_pattern("backup-20240101.tar"), "backup*tar");
        assert_eq!(extract_name_pattern("file.001.dat"), "file*dat");
    }
    
    #[test]
    fn test_find_common_ancestor() {
        let paths = vec![
            Path::new("/Users/test/logs/app.log"),
            Path::new("/Users/test/logs/error.log"),
        ];
        let ancestor = find_common_ancestor(&paths);
        assert_eq!(ancestor, Some(PathBuf::from("/Users/test/logs")));
    }
}
