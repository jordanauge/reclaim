use crate::candidate::{Action, Candidate, TargetKind};
use crate::profile::Profile;
use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::Path;

/// Scan Cisco enterprise application logs
/// 
/// Common culprits:
/// - Cisco Data Shift.log (can grow to 10GB+)
/// - Cisco AnyConnect logs
/// - Cisco Webex logs
/// - Cisco Jabber logs
/// 
/// These are enterprise tools that generate verbose logs
/// Safe to delete logs older than 7-30 days
pub fn scan(_root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("cisco-logs");
    if !config.enabled {
        return Ok(vec![]);
    }
    
    let min_age_days = profile.min_age_for("cisco-logs");
    
    let logs_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join("Library/Logs");
    
    if !logs_dir.exists() {
        return Ok(vec![]);
    }
    
    let mut candidates = Vec::new();
    
    // Check for the notorious Cisco Data Shift log
    let data_shift_log = logs_dir.join("Cisco Data Shift.log");
    if data_shift_log.exists() {
        if let Ok(metadata) = fs::metadata(&data_shift_log) {
            let size = metadata.len();
            let last_modified = metadata.modified()
                .ok()
                .map(|t| chrono::DateTime::<Utc>::from(t));
            
            let age_days = last_modified
                .map(|dt| (Utc::now() - dt).num_days())
                .unwrap_or(0);
            
            if size >= 10_000_000 // 10MB
                && age_days >= min_age_days as i64 {
                candidates.push(Candidate {
                    path: data_shift_log,
                    kind: TargetKind::CiscoLogs,
                    size_bytes: size,
                    last_modified,
                    last_accessed: None,
                    reproducibility: 1.0,
                    score: 0.0,
                    tags: vec!["enterprise".to_string(), "single-file".to_string(), "cisco-data-shift".to_string()],
                    action: Action::Delete,
                    group: Some("Cisco Logs".to_string()),
                });
            }
        }
    }
    
    // Check Cisco logs directory
    let cisco_logs = logs_dir.join("Cisco");
    if cisco_logs.exists() {
        for entry in fs::read_dir(&cisco_logs)? {
            let entry = entry?;
            let path = entry.path();
            
            if !path.is_file() {
                continue;
            }
            
            // Only process .log files
            if path.extension().map_or(false, |e| e == "log") {
                if let Ok(metadata) = fs::metadata(&path) {
                    let size = metadata.len();
                    let last_modified = metadata.modified()
                        .ok()
                        .map(|t| chrono::DateTime::<Utc>::from(t));
                    
                    let age_days = last_modified
                        .map(|dt| (Utc::now() - dt).num_days())
                        .unwrap_or(0);
                    
                    if age_days >= min_age_days as i64 {
                        candidates.push(Candidate {
                            path,
                            kind: TargetKind::CiscoLogs,
                            size_bytes: size,
                            last_modified,
                            last_accessed: None,
                            reproducibility: 1.0,
                            score: 0.0,
                            tags: vec!["enterprise".to_string(), "cisco".to_string()],
                            action: Action::Delete,
                            group: Some("Cisco Logs".to_string()),
                        });
                    }
                }
            }
        }
    }
    
    // Check Webex logs
    let webex_logs = logs_dir.join("Webex Meetings");
    if webex_logs.exists() {
        scan_log_directory(&webex_logs, "Webex Logs", min_age_days, &mut candidates)?;
    }
    
    Ok(candidates)
}

fn scan_log_directory(
    dir: &Path,
    group_name: &str,
    min_age_days: u32,
    candidates: &mut Vec<Candidate>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            if let Ok(metadata) = fs::metadata(&path) {
                let size = metadata.len();
                let last_modified = metadata.modified()
                    .ok()
                    .map(|t| chrono::DateTime::<Utc>::from(t));
                
                let age_days = last_modified
                    .map(|dt| (Utc::now() - dt).num_days())
                    .unwrap_or(0);
                
                if age_days >= min_age_days as i64 {
                    candidates.push(Candidate {
                        path,
                        kind: TargetKind::CiscoLogs,
                        size_bytes: size,
                        last_modified,
                        last_accessed: None,
                        reproducibility: 1.0,
                        score: 0.0,
                        tags: vec!["enterprise".to_string(), "webex".to_string()],
                        action: Action::Delete,
                        group: Some(group_name.to_string()),
                    });
                }
            }
        }
    }
    
    Ok(())
}
