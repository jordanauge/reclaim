use std::cmp::Ordering;
use std::path::{Path, PathBuf};

/// Scan priority heuristics for optimizing discovery order
#[derive(Debug, Clone)]
pub struct ScanPriority {
    pub path: PathBuf,
    pub score: f32,
    pub reason: PriorityReason,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PriorityReason {
    /// First scan: likely to contain large artifacts
    LikelyLarge,
    /// Rescan: likely to have changed since last scan
    LikelyChanged,
    /// Hot path (user-configured or learned)
    HotPath,
    /// Fallback
    Default,
}

impl ScanPriority {
    /// Compute priority for first-time scan (maximize large file discovery)
    pub fn for_first_scan(path: &Path) -> Self {
        let score = first_scan_score(path);
        Self {
            path: path.to_path_buf(),
            score,
            reason: PriorityReason::LikelyLarge,
        }
    }

    /// Compute priority for rescan (maximize change detection)
    pub fn for_rescan(path: &Path, last_scan_days_ago: u32) -> Self {
        let score = rescan_score(path, last_scan_days_ago);
        Self {
            path: path.to_path_buf(),
            score,
            reason: PriorityReason::LikelyChanged,
        }
    }

    /// Mark as hot path (highest priority)
    pub fn hot_path(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            score: 100.0,
            reason: PriorityReason::HotPath,
        }
    }
}

impl PartialEq for ScanPriority {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl Eq for ScanPriority {}

impl PartialOrd for ScanPriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse order: higher score = higher priority
        other.score.partial_cmp(&self.score)
    }
}

impl Ord for ScanPriority {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// First scan heuristic: prioritize paths likely to contain large artifacts
fn first_scan_score(path: &Path) -> f32 {
    let path_str = path.to_string_lossy().to_lowercase();
    let components: Vec<_> = path.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect();
    
    let mut score = 10.0; // Base score
    
    // High priority: common locations for large build artifacts
    if path_contains_any(&path_str, &["node_modules", "target", ".venv", "venv", "build", "dist"]) {
        score += 50.0;
    }
    
    // Medium-high: development directories
    if path_contains_any(&path_str, &["repos", "projects", "workspace", "code", "dev"]) {
        score += 30.0;
    }
    
    // Medium: caches and derived data
    if path_contains_any(&path_str, &["cache", "caches", "deriveddata", ".cache"]) {
        score += 40.0;
    }
    
    // Medium: Xcode artifacts (macOS)
    if path_contains_any(&path_str, &["xcode", "developer/xcode"]) {
        score += 35.0;
    }
    
    // Low-medium: Downloads (can have installers, archives)
    if path_contains_any(&path_str, &["downloads", "download"]) {
        score += 20.0;
    }
    
    // Penalty: system folders (usually nothing to clean)
    if path_contains_any(&path_str, &["/system/", "/applications/", "/library/frameworks"]) {
        score -= 20.0;
    }
    
    // Penalty: media folders (rarely have cleanable artifacts)
    if path_contains_any(&path_str, &["photos", "pictures", "music", "movies", "videos", "media"]) {
        score -= 10.0;
    }
    
    // Depth penalty: prefer scanning closer to root first
    let depth = components.len();
    if depth > 5 {
        score -= (depth - 5) as f32 * 2.0;
    }
    
    score.max(0.0)
}

/// Rescan heuristic: prioritize paths likely to have changed
fn rescan_score(path: &Path, last_scan_days_ago: u32) -> f32 {
    let path_str = path.to_string_lossy().to_lowercase();
    
    let mut score = 10.0; // Base score
    
    // Very high priority: active development paths (change daily)
    if path_contains_any(&path_str, &["target/debug", "target/release", "build/", "__pycache__"]) {
        score += 60.0;
    }
    
    // High priority: package managers (change with updates)
    if path_contains_any(&path_str, &["node_modules", ".venv", "venv", "vendor"]) {
        score += 50.0;
    }
    
    // High priority: temp/cache (changes frequently)
    if path_contains_any(&path_str, &["/tmp", "temp", "cache", ".cache", "caches"]) {
        score += 55.0;
    }
    
    // Medium-high: user workspace (active work)
    if path_contains_any(&path_str, &["desktop", "downloads", "documents"]) {
        score += 40.0;
    }
    
    // Medium: repos root (new clones, new projects)
    if path_contains_any(&path_str, &["repos", "projects", "workspace"]) && path_str.matches('/').count() <= 4 {
        score += 30.0;
    }
    
    // Low: system (rarely changes in terms of cleanable content)
    if path_contains_any(&path_str, &["/system/", "/applications/", "/usr/"]) {
        score -= 30.0;
    }
    
    // Very low: media (almost never changes)
    if path_contains_any(&path_str, &["photos", "pictures", "music", "movies"]) {
        score -= 40.0;
    }
    
    // Time factor: older scans need higher priority
    let time_factor = (last_scan_days_ago as f32 / 30.0).min(2.0); // Cap at 2x
    score *= 1.0 + time_factor;
    
    score.max(0.0)
}

fn path_contains_any(path: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| path.contains(p))
}

/// Scan phases for progressive reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanPhase {
    /// Initial quick scan (hot paths, high priority)
    Quick,
    /// Medium priority paths
    Medium,
    /// Low priority paths (thorough)
    Thorough,
    /// Complete
    Complete,
}

impl ScanPhase {
    /// Get threshold for transitioning to this phase
    pub fn threshold_items(&self) -> usize {
        match self {
            Self::Quick => 50,      // Report after 50 items
            Self::Medium => 200,    // Report after 200 total
            Self::Thorough => 1000, // Report after 1000 total
            Self::Complete => usize::MAX,
        }
    }
    
    /// Get threshold for total size discovered
    pub fn threshold_size_gb(&self) -> f32 {
        match self {
            Self::Quick => 10.0,    // Report after 10 GB
            Self::Medium => 50.0,   // Report after 50 GB
            Self::Thorough => 200.0, // Report after 200 GB
            Self::Complete => f32::MAX,
        }
    }
    
    pub fn next(&self) -> Option<Self> {
        match self {
            Self::Quick => Some(Self::Medium),
            Self::Medium => Some(Self::Thorough),
            Self::Thorough => Some(Self::Complete),
            Self::Complete => None,
        }
    }
    
    pub fn label(&self) -> &'static str {
        match self {
            Self::Quick => "Quick Scan",
            Self::Medium => "Medium Scan",
            Self::Thorough => "Thorough Scan",
            Self::Complete => "Complete",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_scan_priorities() {
        let node_modules = ScanPriority::for_first_scan(Path::new("/home/user/project/node_modules"));
        let photos = ScanPriority::for_first_scan(Path::new("/home/user/Photos"));
        let venv = ScanPriority::for_first_scan(Path::new("/home/user/repos/project/.venv"));
        
        assert!(node_modules.score > photos.score);
        assert!(venv.score > photos.score);
    }

    #[test]
    fn test_rescan_priorities() {
        let build = ScanPriority::for_rescan(Path::new("/home/user/project/target/debug"), 1);
        let media = ScanPriority::for_rescan(Path::new("/home/user/Photos"), 1);
        
        assert!(build.score > media.score);
    }

    #[test]
    fn test_time_factor() {
        let recent = ScanPriority::for_rescan(Path::new("/home/user/repos"), 1);
        let old = ScanPriority::for_rescan(Path::new("/home/user/repos"), 60);
        
        // Older scans should have higher priority
        assert!(old.score > recent.score);
    }

    #[test]
    fn test_hot_path() {
        let hot = ScanPriority::hot_path(Path::new("/any/path"));
        let normal = ScanPriority::for_first_scan(Path::new("/home/user/node_modules"));
        
        assert!(hot.score > normal.score);
    }

    #[test]
    fn test_scan_phases() {
        assert_eq!(ScanPhase::Quick.next(), Some(ScanPhase::Medium));
        assert_eq!(ScanPhase::Complete.next(), None);
    }
}
