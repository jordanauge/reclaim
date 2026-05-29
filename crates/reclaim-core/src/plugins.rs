use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use chrono::{DateTime, Utc};

/// System indexing service plugins for fast change detection
pub trait IndexPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn get_modified_since(&self, root: &Path, since: DateTime<Utc>) -> Result<Vec<PathBuf>>;
}

/// macOS Spotlight (mdfind)
#[cfg(target_os = "macos")]
pub struct SpotlightPlugin;

#[cfg(target_os = "macos")]
impl IndexPlugin for SpotlightPlugin {
    fn name(&self) -> &str {
        "Spotlight"
    }

    fn is_available(&self) -> bool {
        Command::new("mdfind")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn get_modified_since(&self, root: &Path, since: DateTime<Utc>) -> Result<Vec<PathBuf>> {
        // mdfind query: files modified after timestamp
        let timestamp = since.format("%Y-%m-%d %H:%M:%S").to_string();
        
        let output = Command::new("mdfind")
            .arg("-onlyin")
            .arg(root)
            .arg(format!("kMDItemFSContentChangeDate > '{}'", timestamp))
            .output()
            .context("Failed to run mdfind")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("mdfind failed: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }

        let paths = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(PathBuf::from)
            .collect();

        Ok(paths)
    }
}

/// Linux locate/updatedb
#[cfg(target_os = "linux")]
pub struct LocatePlugin;

#[cfg(target_os = "linux")]
impl IndexPlugin for LocatePlugin {
    fn name(&self) -> &str {
        "locate/updatedb"
    }

    fn is_available(&self) -> bool {
        Command::new("locate")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn get_modified_since(&self, root: &Path, since: DateTime<Utc>) -> Result<Vec<PathBuf>> {
        // First, get all files under root
        let output = Command::new("locate")
            .arg("-r")
            .arg(format!("^{}", root.display()))
            .output()
            .context("Failed to run locate")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("locate failed"));
        }

        // Filter by modification time (requires stat)
        let since_ts = since.timestamp();
        let paths: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let path = PathBuf::from(line);
                if let Ok(meta) = std::fs::metadata(&path) {
                    if let Ok(modified) = meta.modified() {
                        let modified_ts = modified.duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        if modified_ts > since_ts {
                            return Some(path);
                        }
                    }
                }
                None
            })
            .collect();

        Ok(paths)
    }
}

/// KDE Baloo indexer
#[cfg(target_os = "linux")]
pub struct BalooPlugin;

#[cfg(target_os = "linux")]
impl IndexPlugin for BalooPlugin {
    fn name(&self) -> &str {
        "Baloo"
    }

    fn is_available(&self) -> bool {
        Command::new("baloosearch")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn get_modified_since(&self, root: &Path, since: DateTime<Utc>) -> Result<Vec<PathBuf>> {
        // Baloo query for modified files
        let timestamp = since.format("%Y-%m-%d").to_string();
        
        let output = Command::new("baloosearch")
            .arg(format!("modified>={}", timestamp))
            .arg("--type")
            .arg("File")
            .arg("--url")
            .arg(format!("file://{}", root.display()))
            .output()
            .context("Failed to run baloosearch")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("baloosearch failed"));
        }

        let paths = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                if line.starts_with("file://") {
                    Some(PathBuf::from(&line[7..]))
                } else {
                    Some(PathBuf::from(line))
                }
            })
            .collect();

        Ok(paths)
    }
}

/// Plugin manager
pub struct IndexPluginManager {
    plugins: Vec<Box<dyn IndexPlugin>>,
}

impl IndexPluginManager {
    pub fn new() -> Self {
        let mut plugins: Vec<Box<dyn IndexPlugin>> = Vec::new();

        #[cfg(target_os = "macos")]
        plugins.push(Box::new(SpotlightPlugin));

        #[cfg(target_os = "linux")]
        {
            plugins.push(Box::new(LocatePlugin));
            plugins.push(Box::new(BalooPlugin));
        }

        Self { plugins }
    }

    /// Get the first available plugin
    pub fn get_available(&self) -> Option<&dyn IndexPlugin> {
        self.plugins
            .iter()
            .find(|p| p.is_available())
            .map(|p| p.as_ref())
    }

    /// Try to use system indexer, fallback to None if unavailable
    pub fn find_changes(&self, root: &Path, since: DateTime<Utc>) -> Option<Vec<PathBuf>> {
        if let Some(plugin) = self.get_available() {
            eprintln!("Using {} for change detection", plugin.name());
            match plugin.get_modified_since(root, since) {
                Ok(paths) => {
                    eprintln!("Found {} changed paths via {}", paths.len(), plugin.name());
                    return Some(paths);
                }
                Err(e) => {
                    eprintln!("Plugin {} failed: {}, falling back to manual scan", plugin.name(), e);
                }
            }
        }
        None
    }
}

impl Default for IndexPluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Piggyback on system indexer to detect changed directories
pub fn detect_changed_dirs(root: &Path, since: DateTime<Utc>) -> Option<Vec<PathBuf>> {
    let manager = IndexPluginManager::new();
    
    if let Some(changed_files) = manager.find_changes(root, since) {
        // Roll up changed files to their parent directories
        let mut changed_dirs: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
        
        for file in changed_files {
            // Add all ancestor directories
            let mut current = file.as_path();
            while let Some(parent) = current.parent() {
                if parent == root || !parent.starts_with(root) {
                    break;
                }
                changed_dirs.insert(parent.to_path_buf());
                current = parent;
            }
        }
        
        let mut dirs: Vec<_> = changed_dirs.into_iter().collect();
        dirs.sort();
        
        return Some(dirs);
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manager() {
        let manager = IndexPluginManager::new();
        
        // Should have at least one plugin per platform
        #[cfg(target_os = "macos")]
        assert_eq!(manager.plugins.len(), 1);
        
        #[cfg(target_os = "linux")]
        assert_eq!(manager.plugins.len(), 2);
    }

    #[test]
    fn test_detect_changed_dirs() {
        let home = dirs::home_dir().unwrap();
        let since = Utc::now() - chrono::Duration::hours(1);
        
        // This may or may not return results depending on system indexer availability
        if let Some(dirs) = detect_changed_dirs(&home, since) {
            println!("Detected {} changed directories", dirs.len());
            for dir in dirs.iter().take(5) {
                println!("  - {}", dir.display());
            }
        } else {
            println!("No system indexer available or no changes detected");
        }
    }
}
