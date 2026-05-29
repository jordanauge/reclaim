use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use serde::{Deserialize, Serialize};

/// Main disk space categories (non-reclaimable by default)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiskCategory {
    /// Operating system, binaries, libraries
    System,
    /// Photos, videos, audio files
    Media,
    /// Documents, PDFs, Office files, text
    Documents,
    /// Source code repositories
    Code,
    /// Reclaimable artifacts (caches, build artifacts, etc.)
    Reclaimable,
    /// Everything else
    Other,
}

impl DiskCategory {
    pub fn label(&self) -> &str {
        match self {
            Self::System => "System",
            Self::Media => "Media",
            Self::Documents => "Documents",
            Self::Code => "Code",
            Self::Reclaimable => "Reclaimable",
            Self::Other => "Other",
        }
    }
    
    pub fn emoji(&self) -> &str {
        match self {
            Self::System => "⚙️",
            Self::Media => "🎬",
            Self::Documents => "📄",
            Self::Code => "💻",
            Self::Reclaimable => "♻️",
            Self::Other => "📦",
        }
    }
}

/// Detailed subcategories for better classification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiskSubcategory {
    // System
    SystemBinaries,
    SystemLibraries,
    SystemKernel,
    SystemCaches,
    SystemLogs,
    
    // Media
    Photos,
    Videos,
    Audio,
    MediaBackups,
    
    // Documents
    PDFs,
    OfficeDocuments,
    TextFiles,
    Presentations,
    Spreadsheets,
    Archives,
    
    // Code
    SourceCode,
    Dependencies,      // node_modules, .venv, vendor
    BuildArtifacts,    // build/, target/, dist/
    GitRepositories,
    
    // Reclaimable
    Caches,
    Logs,
    TempFiles,
    Duplicates,
    OldBackups,
    
    // Other
    Unknown,
}

impl DiskSubcategory {
    pub fn label(&self) -> &str {
        match self {
            Self::SystemBinaries => "Binaries",
            Self::SystemLibraries => "Libraries",
            Self::SystemKernel => "Kernel",
            Self::SystemCaches => "System Caches",
            Self::SystemLogs => "System Logs",
            
            Self::Photos => "Photos",
            Self::Videos => "Videos",
            Self::Audio => "Audio",
            Self::MediaBackups => "Media Backups",
            
            Self::PDFs => "PDF Documents",
            Self::OfficeDocuments => "Office Documents",
            Self::TextFiles => "Text Files",
            Self::Presentations => "Presentations",
            Self::Spreadsheets => "Spreadsheets",
            Self::Archives => "Archives",
            
            Self::SourceCode => "Source Code",
            Self::Dependencies => "Dependencies",
            Self::BuildArtifacts => "Build Artifacts",
            Self::GitRepositories => "Git Repos",
            
            Self::Caches => "Caches",
            Self::Logs => "Logs",
            Self::TempFiles => "Temp Files",
            Self::Duplicates => "Duplicates",
            Self::OldBackups => "Old Backups",
            
            Self::Unknown => "Unknown",
        }
    }
    
    /// Is this subcategory potentially reclaimable?
    pub fn is_reclaimable(&self) -> bool {
        matches!(self,
            Self::SystemCaches | Self::SystemLogs |
            Self::MediaBackups |
            Self::Archives |
            Self::Dependencies | Self::BuildArtifacts |
            Self::Caches | Self::Logs | Self::TempFiles | 
            Self::Duplicates | Self::OldBackups
        )
    }
    
    /// Parent category
    pub fn category(&self) -> DiskCategory {
        match self {
            Self::SystemBinaries | Self::SystemLibraries | Self::SystemKernel => 
                DiskCategory::System,
            Self::SystemCaches | Self::SystemLogs => 
                DiskCategory::Reclaimable,
                
            Self::Photos | Self::Videos | Self::Audio => 
                DiskCategory::Media,
            Self::MediaBackups => 
                DiskCategory::Reclaimable,
                
            Self::PDFs | Self::OfficeDocuments | Self::TextFiles | 
            Self::Presentations | Self::Spreadsheets => 
                DiskCategory::Documents,
            Self::Archives => 
                DiskCategory::Reclaimable,
                
            Self::SourceCode | Self::GitRepositories => 
                DiskCategory::Code,
            Self::Dependencies | Self::BuildArtifacts => 
                DiskCategory::Reclaimable,
                
            Self::Caches | Self::Logs | Self::TempFiles | 
            Self::Duplicates | Self::OldBackups => 
                DiskCategory::Reclaimable,
                
            Self::Unknown => 
                DiskCategory::Other,
        }
    }
}

/// A classified file or directory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedEntry {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub category: DiskCategory,
    pub subcategory: DiskSubcategory,
    pub is_reclaimable: bool,
}

/// Aggregate statistics for disk space analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskStats {
    pub total_bytes: u64,
    pub by_category: HashMap<DiskCategory, u64>,
    pub by_subcategory: HashMap<DiskSubcategory, u64>,
    pub reclaimable_bytes: u64,
    pub non_reclaimable_bytes: u64,
}

impl DiskStats {
    pub fn category_percentage(&self, category: &DiskCategory) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        let bytes = self.by_category.get(category).copied().unwrap_or(0);
        (bytes as f64 / self.total_bytes as f64 * 100.0) as f32
    }
    
    pub fn reclaimable_percentage(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.reclaimable_bytes as f64 / self.total_bytes as f64 * 100.0) as f32
    }
}

/// Analyze disk space usage by category
pub fn analyze_disk(root: &Path, max_depth: Option<usize>) -> Result<(Vec<ClassifiedEntry>, DiskStats)> {
    let mut entries = Vec::new();
    let mut stats = DiskStats::default();
    
    let walker = if let Some(depth) = max_depth {
        WalkDir::new(root).max_depth(depth).follow_links(false)
    } else {
        WalkDir::new(root).follow_links(false)
    };
    
    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        
        // Skip if we can't read metadata
        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        
        let size = if metadata.is_file() {
            metadata.len()
        } else {
            // For directories, we'll calculate size separately
            0
        };
        
        // Classify the entry
        let (category, subcategory) = classify_path(path);
        let is_reclaimable = subcategory.is_reclaimable();
        
        // Update stats
        stats.total_bytes += size;
        *stats.by_category.entry(category.clone()).or_insert(0) += size;
        *stats.by_subcategory.entry(subcategory.clone()).or_insert(0) += size;
        
        if is_reclaimable {
            stats.reclaimable_bytes += size;
        } else {
            stats.non_reclaimable_bytes += size;
        }
        
        if metadata.is_file() && size > 0 {
            entries.push(ClassifiedEntry {
                path: path.to_path_buf(),
                size_bytes: size,
                category,
                subcategory,
                is_reclaimable,
            });
        }
    }
    
    Ok((entries, stats))
}

/// Classify a path into category and subcategory
fn classify_path(path: &Path) -> (DiskCategory, DiskSubcategory) {
    let path_str = path.to_string_lossy().to_lowercase();
    let filename = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    // Check extension first
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext = ext.to_lowercase();
        
        // Media files
        if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "raw" | "cr2" | "nef") {
            return (DiskCategory::Media, DiskSubcategory::Photos);
        }
        if matches!(ext.as_str(), "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v") {
            return (DiskCategory::Media, DiskSubcategory::Videos);
        }
        if matches!(ext.as_str(), "mp3" | "wav" | "flac" | "m4a" | "aac" | "ogg" | "wma") {
            return (DiskCategory::Media, DiskSubcategory::Audio);
        }
        
        // Documents
        if matches!(ext.as_str(), "pdf") {
            return (DiskCategory::Documents, DiskSubcategory::PDFs);
        }
        if matches!(ext.as_str(), "doc" | "docx" | "odt") {
            return (DiskCategory::Documents, DiskSubcategory::OfficeDocuments);
        }
        if matches!(ext.as_str(), "txt" | "md" | "markdown" | "rst") {
            return (DiskCategory::Documents, DiskSubcategory::TextFiles);
        }
        if matches!(ext.as_str(), "ppt" | "pptx" | "key" | "odp") {
            return (DiskCategory::Documents, DiskSubcategory::Presentations);
        }
        if matches!(ext.as_str(), "xls" | "xlsx" | "csv" | "ods") {
            return (DiskCategory::Documents, DiskSubcategory::Spreadsheets);
        }
        if matches!(ext.as_str(), "zip" | "tar" | "gz" | "bz2" | "7z" | "rar") {
            return (DiskCategory::Reclaimable, DiskSubcategory::Archives);
        }
        
        // Code
        if matches!(ext.as_str(), "rs" | "py" | "js" | "ts" | "java" | "c" | "cpp" | "go" | "rb" | "php" | "swift" | "kt") {
            return (DiskCategory::Code, DiskSubcategory::SourceCode);
        }
        
        // System
        if matches!(ext.as_str(), "so" | "dylib" | "dll" | "a" | "lib") {
            return (DiskCategory::System, DiskSubcategory::SystemLibraries);
        }
        
        // Logs
        if matches!(ext.as_str(), "log") {
            return (DiskCategory::Reclaimable, DiskSubcategory::Logs);
        }
    }
    
    // Check path components
    if path_str.contains("/system/") || path_str.contains("/usr/bin") || path_str.contains("/usr/sbin") {
        return (DiskCategory::System, DiskSubcategory::SystemBinaries);
    }
    if path_str.contains("/usr/lib") || path_str.contains("/system/library") {
        return (DiskCategory::System, DiskSubcategory::SystemLibraries);
    }
    
    // Reclaimable patterns
    if filename == "node_modules" || path_str.contains("/node_modules/") {
        return (DiskCategory::Reclaimable, DiskSubcategory::Dependencies);
    }
    if filename == ".venv" || filename == "venv" || path_str.contains("/.venv/") {
        return (DiskCategory::Reclaimable, DiskSubcategory::Dependencies);
    }
    if filename == "target" && path_str.contains("/cargo/") {
        return (DiskCategory::Reclaimable, DiskSubcategory::BuildArtifacts);
    }
    if filename == "build" || filename == "dist" || filename == "__pycache__" {
        return (DiskCategory::Reclaimable, DiskSubcategory::BuildArtifacts);
    }
    if path_str.contains("/cache") || path_str.contains("/caches") {
        return (DiskCategory::Reclaimable, DiskSubcategory::Caches);
    }
    if path_str.contains("/logs/") || path_str.contains("/log/") {
        return (DiskCategory::Reclaimable, DiskSubcategory::Logs);
    }
    if filename == ".git" || path_str.contains("/.git/") {
        return (DiskCategory::Code, DiskSubcategory::GitRepositories);
    }
    
    // Code directories
    if path_str.contains("/repos/") || path_str.contains("/projects/") || path_str.contains("/src/") {
        return (DiskCategory::Code, DiskSubcategory::SourceCode);
    }
    
    // Media directories
    if path_str.contains("/photos/") || path_str.contains("/pictures/") {
        return (DiskCategory::Media, DiskSubcategory::Photos);
    }
    if path_str.contains("/videos/") || path_str.contains("/movies/") {
        return (DiskCategory::Media, DiskSubcategory::Videos);
    }
    if path_str.contains("/music/") || path_str.contains("/audio/") {
        return (DiskCategory::Media, DiskSubcategory::Audio);
    }
    
    // Documents directories
    if path_str.contains("/documents/") || path_str.contains("/docs/") {
        return (DiskCategory::Documents, DiskSubcategory::TextFiles);
    }
    
    // Default
    (DiskCategory::Other, DiskSubcategory::Unknown)
}

/// Generate a human-readable report
pub fn generate_report(stats: &DiskStats) -> String {
    let mut report = String::new();
    
    report.push_str(&format!("📊 Disk Space Analysis\n"));
    report.push_str(&format!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n"));
    
    report.push_str(&format!("Total: {}\n\n", human_bytes(stats.total_bytes)));
    
    report.push_str("By Category:\n");
    let mut categories: Vec<_> = stats.by_category.iter().collect();
    categories.sort_by(|a, b| b.1.cmp(a.1));
    
    for (category, bytes) in categories {
        let pct = ((*bytes as f64 / stats.total_bytes as f64) * 100.0) as f32;
        report.push_str(&format!(
            "  {} {} {:>6.1}%  {}\n",
            category.emoji(),
            category.label(),
            pct,
            human_bytes(*bytes)
        ));
    }
    
    report.push_str(&format!("\n♻️  Reclaimable: {:.1}% ({})\n", 
        stats.reclaimable_percentage(),
        human_bytes(stats.reclaimable_bytes)
    ));
    report.push_str(&format!("🔒 Protected: {:.1}% ({})\n", 
        100.0 - stats.reclaimable_percentage(),
        human_bytes(stats.non_reclaimable_bytes)
    ));
    
    report
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
    fn test_classify_media() {
        let path = PathBuf::from("/home/user/photos/IMG_1234.jpg");
        let (cat, subcat) = classify_path(&path);
        assert_eq!(cat, DiskCategory::Media);
        assert_eq!(subcat, DiskSubcategory::Photos);
    }
    
    #[test]
    fn test_classify_code() {
        let path = PathBuf::from("/home/user/repos/project/node_modules/package/index.js");
        let (cat, subcat) = classify_path(&path);
        assert_eq!(cat, DiskCategory::Reclaimable);
        assert_eq!(subcat, DiskSubcategory::Dependencies);
    }
    
    #[test]
    fn test_reclaimable_detection() {
        assert!(DiskSubcategory::Caches.is_reclaimable());
        assert!(DiskSubcategory::Dependencies.is_reclaimable());
        assert!(!DiskSubcategory::Photos.is_reclaimable());
        assert!(!DiskSubcategory::SourceCode.is_reclaimable());
    }
}
