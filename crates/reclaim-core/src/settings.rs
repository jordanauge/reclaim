use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Result, Context};

/// User settings stored persistently
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Don't prompt for permissions at startup
    pub dont_ask_permissions_at_startup: bool,
    
    /// Last window size
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    
    /// Last selected view mode
    pub default_view_mode: Option<String>,
    
    /// Show groups by default
    pub show_groups_by_default: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dont_ask_permissions_at_startup: false,
            window_width: None,
            window_height: None,
            default_view_mode: None,
            show_groups_by_default: true,
        }
    }
}

impl Settings {
    /// Get the default settings file path
    pub fn default_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .context("Could not determine home directory")?;
        
        let config_dir = home.join(".config").join("reclaim");
        
        // Create config directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)
                .context("Failed to create config directory")?;
        }
        
        Ok(config_dir.join("settings.toml"))
    }
    
    /// Load settings from default path
    pub fn load() -> Result<Self> {
        let path = Self::default_path()?;
        
        if !path.exists() {
            // Return defaults if file doesn't exist
            return Ok(Self::default());
        }
        
        let contents = fs::read_to_string(&path)
            .context("Failed to read settings file")?;
        
        let settings: Settings = toml::from_str(&contents)
            .context("Failed to parse settings file")?;
        
        Ok(settings)
    }
    
    /// Save settings to default path
    pub fn save(&self) -> Result<()> {
        let path = Self::default_path()?;
        
        let contents = toml::to_string_pretty(self)
            .context("Failed to serialize settings")?;
        
        fs::write(&path, contents)
            .context("Failed to write settings file")?;
        
        Ok(())
    }
    
    /// Load settings, returning defaults on error
    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_default()
    }
}
