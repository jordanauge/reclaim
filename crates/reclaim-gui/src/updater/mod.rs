/// Update detection and management module
/// 
/// Detects installation method and conditionally enables auto-update:
/// - Standalone (AppImage, DMG, manual): auto-update enabled
/// - System packages (apt, flatpak, snap, homebrew): auto-update disabled
use std::path::PathBuf;

pub mod backends;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMethod {
    /// Standalone installation (AppImage, DMG, manual binary)
    /// Auto-update is ENABLED
    Standalone,
    
    /// System package manager (apt, flatpak, snap, homebrew)
    /// Auto-update is DISABLED - user should use their package manager
    SystemPackage,
}

impl InstallMethod {
    /// Check if auto-update should be enabled for this installation
    pub fn can_auto_update(&self) -> bool {
        matches!(self, InstallMethod::Standalone)
    }
    
    /// Get user-friendly update instructions
    pub fn update_instructions(&self) -> &'static str {
        match self {
            InstallMethod::Standalone => "Use the built-in updater or download from GitHub releases",
            InstallMethod::SystemPackage => "Use your system package manager to update",
        }
    }
}

/// Detect how Reclaim was installed
/// 
/// This determines whether auto-update should be enabled or not.
/// System packages should NOT auto-update to avoid conflicts.
pub fn detect_install_method() -> InstallMethod {
    // Linux: Check for various package managers
    #[cfg(target_os = "linux")]
    {
        // Debian/Ubuntu: Check if installed via apt/dpkg
        if std::path::Path::new("/var/lib/dpkg/info/reclaim.list").exists() {
            return InstallMethod::SystemPackage;
        }
        
        // Flatpak: Check FLATPAK_ID environment variable
        if std::env::var("FLATPAK_ID").is_ok() {
            return InstallMethod::SystemPackage;
        }
        
        // Snap: Check SNAP environment variable
        if std::env::var("SNAP").is_ok() {
            return InstallMethod::SystemPackage;
        }
        
        // AppImage: Check APPIMAGE environment variable
        if std::env::var("APPIMAGE").is_ok() {
            return InstallMethod::Standalone;
        }
    }
    
    // macOS: Check for Homebrew installation
    #[cfg(target_os = "macos")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            let exe_str = exe_path.to_string_lossy();
            
            // Homebrew Cellar paths
            if exe_str.contains("/opt/homebrew") || 
               exe_str.contains("/usr/local/Cellar") ||
               exe_str.contains("/usr/local/opt") {
                return InstallMethod::SystemPackage;
            }
        }
    }
    
    // Windows: Check for installer-managed installation
    #[cfg(target_os = "windows")]
    {
        // Check if running from Program Files (installed via NSIS/MSI)
        if let Ok(exe_path) = std::env::current_exe() {
            let exe_str = exe_path.to_string_lossy();
            
            if exe_str.contains("Program Files") || exe_str.contains("Program Files (x86)") {
                // Check for uninstaller (indicates package manager installation)
                if let Some(parent) = exe_path.parent() {
                    if parent.join("uninstall.exe").exists() {
                        return InstallMethod::SystemPackage;
                    }
                }
            }
        }
    }
    
    // Default: Standalone (manual installation, portable, etc.)
    InstallMethod::Standalone
}

/// Get current executable path for update purposes
pub fn current_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_install_detection() {
        let method = detect_install_method();
        println!("Detected install method: {:?}", method);
        
        // Should always return a valid method
        assert!(matches!(method, InstallMethod::Standalone | InstallMethod::SystemPackage));
    }
    
    #[test]
    fn test_standalone_can_update() {
        assert!(InstallMethod::Standalone.can_auto_update());
        assert!(!InstallMethod::SystemPackage.can_auto_update());
    }
}
