mod updater;

use anyhow::Result;
use chrono::{DateTime, Utc};
use crossbeam_channel::{unbounded, Receiver};
use eframe::egui;
use reclaim_core::{
    cache::{ScanCache, ScanStats},
    candidate::{Action, Candidate, TargetKind},
    discoverer::{DiscoveryMessage, HotPathsDiscoverer},
    disk_analyzer::{analyze_disk, DiskCategory, DiskStats},
    profile::Profile,
    report::Report,
    scanner,
    selection::{CandidateState, CacheStatus},
    strategy,
    verifier::{Tier1Verifier, VerificationMessage},
};
use std::collections::HashSet;
use std::path::PathBuf;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([1000.0, 650.0])
            .with_title("Reclaim — Modern Disk Space Analyzer"),
        ..Default::default()
    };

    eframe::run_native(
        "Reclaim",
        options,
        Box::new(|_cc| Ok(Box::new(ReclaimApp::default()))),
    )
}

struct ReclaimApp {
    // Configuration
    scan_roots:      Vec<PathBuf>,
    profile_name:    String,
    profile:         Option<Profile>,
    auto_scan_enabled: bool,

    // Scan state
    candidates:      Vec<CandidateState>,
    scan_status:     ScanStatus,
    scan_receiver:   Option<Receiver<ScanMessage>>,
    scan_progress:   ScanProgress,
    scan_new_count:  usize,  // New items found in current scan
    
    // Cache
    cache:           Option<ScanCache>,
    verification_receiver: Option<Receiver<VerificationMessage>>,
    verification_in_progress: bool,
    discovery_receiver: Option<Receiver<DiscoveryMessage>>,
    discovery_in_progress: bool,
    last_discovery: Option<DateTime<Utc>>,
    
    // UI state
    sort_by:         SortBy,
    group_by:        GroupBy,
    view_mode:       ViewMode,
    
    // Categorical filters (applied dynamically)
    enabled_kinds:   HashSet<String>,   // TargetKind labels
    enabled_actions: HashSet<String>,   // Action labels
    enabled_groups:  HashSet<String>,   // Group names
    
    // Available values (extracted from candidates after scan)
    available_kinds:   Vec<String>,
    available_actions: Vec<String>,
    available_groups:  Vec<String>,
    
    // Range filters (applied dynamically, no rescan needed)
    filter_min_size_mb: f32,  // In MB for slider convenience
    filter_max_size_gb: f32,  // In GB for slider convenience
    filter_min_age_days: u32,
    filter_max_age_days: u32,
    filter_min_score: f32,
    show_only_selected: bool,
    scroll_to_top:   bool,

    // Actions
    status_message:  String,
    operation_state: OperationState,
    
    // Disk Overview
    disk_stats:      Option<DiskStats>,
    disk_analysis_receiver: Option<Receiver<DiskAnalysisMessage>>,
    disk_analysis_depth: usize,
    disk_selected_category: Option<DiskCategory>,
    
    // Permissions & Settings
    show_permission_dialog: bool,
    permission_check_done: bool,
    dont_ask_permissions: bool,
    show_settings: bool,
    settings: reclaim_core::settings::Settings,
    
    // Grouping
    groups: Vec<reclaim_core::grouping::CandidateGroup>,
    expanded_groups: HashSet<String>, // Group IDs that are expanded
    show_groups: bool, // Toggle between grouped and flat view
    selected_group_for_context: Option<String>, // Group ID for genealogy exploration
    show_genealogy_window: bool,
    genealogy_context: Option<reclaim_core::grouping::DirectoryContext>,
    
    // Updates
    install_method: updater::InstallMethod,
    update_available: Option<String>, // Version string if update available
    checking_update: bool,
}

impl Default for ReclaimApp {
    fn default() -> Self {
        // Load persisted settings
        let settings = reclaim_core::settings::Settings::load_or_default();
        let show_groups = settings.show_groups_by_default;
        let dont_ask_permissions = settings.dont_ask_permissions_at_startup;
        
        Self {
            scan_roots: Vec::new(),
            profile_name: String::new(),
            profile: None,
            auto_scan_enabled: false,
            candidates: Vec::new(),
            scan_status: ScanStatus::default(),
            scan_receiver: None,
            scan_progress: ScanProgress::default(),
            scan_new_count: 0,
            cache: None,
            verification_receiver: None,
            verification_in_progress: false,
            discovery_receiver: None,
            discovery_in_progress: false,
            last_discovery: None,
            sort_by: SortBy::default(),
            group_by: GroupBy::default(),
            view_mode: ViewMode::default(),
            enabled_kinds: HashSet::new(),
            enabled_actions: HashSet::new(),
            enabled_groups: HashSet::new(),
            available_kinds: Vec::new(),
            available_actions: Vec::new(),
            available_groups: Vec::new(),
            filter_min_size_mb: 0.0,
            filter_max_size_gb: 0.0,
            filter_min_age_days: 0,
            filter_max_age_days: 0,
            filter_min_score: 0.0,
            show_only_selected: false,
            scroll_to_top: false,
            status_message: String::new(),
            operation_state: OperationState::default(),
            disk_stats: None,
            disk_analysis_receiver: None,
            disk_analysis_depth: 5, // Default: scan 5 levels deep
            disk_selected_category: None,
            
            show_permission_dialog: false,
            permission_check_done: false,
            dont_ask_permissions,
            show_settings: false,
            settings,
            
            groups: Vec::new(),
            expanded_groups: HashSet::new(),
            show_groups,
            selected_group_for_context: None,
            show_genealogy_window: false,
            genealogy_context: None,
            
            install_method: updater::detect_install_method(),
            update_available: None,
            checking_update: false,
        }
    }
}

enum DiskAnalysisMessage {
    Complete(DiskStats),
    Error(String),
}

#[derive(Default, PartialEq, Clone)]
enum OperationState {
    #[default]
    Idle,
    DryRun {
        items: usize,
        total_size: u64,
    },
    ApplyConfirm {
        items: usize,
        total_size: u64,
    },
    Applying {
        total: usize,
    },
}

#[derive(Default, PartialEq, Clone)]
enum ScanStatus {
    /// Initial state before any scan
    #[default]
    NotStarted,
    /// Actively scanning filesystem
    Scanning { phase: ScanPhase },
    /// Paused by user (can be resumed)
    Paused,
    /// Idle - scan complete, waiting for next trigger
    Idle,
    /// Error occurred
    Error(String),
}

#[derive(Default, PartialEq, Clone)]
enum ScanPhase {
    #[default]
    CacheLoad,      // Phase 0: Load from cache (instant)
    QuickVerify,    // Phase 1: Quick verification of cached items (1-5s)
    FullScan,       // Phase 2: Complete filesystem scan (10-60s)
    DiskAnalysis,   // Phase 3: Full disk categorization (30-120s, optional)
}

impl ScanPhase {
    fn description(&self) -> &'static str {
        match self {
            Self::CacheLoad => "Loading cached data instantly",
            Self::QuickVerify => "Verifying cached items still exist (priority: largest first)",
            Self::FullScan => "Complete scan for all reclaimable items",
            Self::DiskAnalysis => "Full disk categorization for overview (optional)",
        }
    }
    
    fn estimated_duration(&self) -> &'static str {
        match self {
            Self::CacheLoad => "< 1s",
            Self::QuickVerify => "1-5s",
            Self::FullScan => "10-60s",
            Self::DiskAnalysis => "30-120s",
        }
    }
    
    fn icon(&self) -> &'static str {
        match self {
            Self::CacheLoad => "💾",
            Self::QuickVerify => "⚡",
            Self::FullScan => "🔍",
            Self::DiskAnalysis => "💽",
        }
    }
}

enum ScanMessage {
    PhaseChange { phase: ScanPhase, started_at: DateTime<Utc> },
    Progress { current: String, total: usize, done: usize, phase_progress: f32 },
    CacheLoaded { candidates: Vec<CandidateState> }, // Phase 0: Cached data
    QuickVerified { updated: Vec<CandidateState> }, // Phase 1: Verified items
    Complete { candidates: Vec<CandidateState>, stats: ScanStats, profile: Profile },
    Error(String),
}

#[derive(Clone)]
struct ScanProgress {
    current_path: String,
    total_roots: usize,
    roots_done: usize,
    phase_progress: f32,  // 0.0 to 1.0 for current phase
    started_at: DateTime<Utc>,
    phase_started_at: DateTime<Utc>,
}

impl Default for ScanProgress {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            current_path: String::new(),
            total_roots: 0,
            roots_done: 0,
            phase_progress: 0.0,
            started_at: now,
            phase_started_at: now,
        }
    }
}

impl ScanProgress {
    fn elapsed(&self) -> chrono::Duration {
        Utc::now() - self.started_at
    }
    
    fn phase_elapsed(&self) -> chrono::Duration {
        Utc::now() - self.phase_started_at
    }
    
    fn estimated_remaining(&self, phase: &ScanPhase) -> Option<chrono::Duration> {
        if self.phase_progress < 0.01 {
            return None;
        }
        let elapsed = self.phase_elapsed();
        let total_estimated = elapsed.num_seconds() as f32 / self.phase_progress;
        let remaining = total_estimated - elapsed.num_seconds() as f32;
        Some(chrono::Duration::seconds(remaining.max(0.0) as i64))
    }
}

#[derive(Default, Clone, Copy, PartialEq)]
enum SortBy {
    #[default]
    Score,
    Size,
    Age,
    Kind,
}

impl SortBy {
}

#[derive(Default, Clone, Copy, PartialEq)]
enum GroupBy {
    #[default]
    None,
    Kind,
    Path,
}

impl GroupBy {
}

#[derive(Default, Clone, Copy, PartialEq)]
enum ViewMode {
    #[default]
    Table,
    Treemap,
    DiskOverview,
}

impl ViewMode {
    fn label(&self) -> &str {
        match self {
            Self::Table => "Table",
            Self::Treemap => "Treemap",
            Self::DiskOverview => "Disk Overview",
        }
    }
}

impl ReclaimApp {
    fn scan(&mut self) {
        // Check current status - allow rescan even if already scanning
        match self.scan_status {
            ScanStatus::Scanning { .. } => {
                // Already scanning - ignore
                return;
            }
            _ => {}
        }

        self.scan_status = ScanStatus::Scanning { phase: ScanPhase::CacheLoad };
        self.status_message = "💾 Loading cache...".to_string();
        self.scan_progress = ScanProgress::default();
        self.scan_new_count = 0;
        // DON'T clear candidates - keep them visible during rescan

        // Load profile
        let profile = match self.load_profile() {
            Ok(p) => p,
            Err(e) => {
                self.scan_status = ScanStatus::Error(format!("Failed to load profile: {e}"));
                return;
            }
        };

        // Use home dir if no roots specified
        let roots = if self.scan_roots.is_empty() {
            vec![dirs::home_dir().unwrap_or_default()]
        } else {
            self.scan_roots.clone()
        };

        self.scan_progress.total_roots = roots.len();
        let need_disk_analysis = self.view_mode == ViewMode::DiskOverview;

        // Spawn background thread for scanning
        let (tx, rx) = unbounded();
        self.scan_receiver = Some(rx);

        std::thread::spawn(move || {
            let scan_start = Utc::now();
            
            // Phase 0: CacheLoad - Load from cache instantly (< 1s)
            let _ = tx.send(ScanMessage::PhaseChange { 
                phase: ScanPhase::CacheLoad,
                started_at: Utc::now(),
            });
            
            let mut has_cache = false;
            match ScanCache::open_default() {
                Ok(cache) => {
                    // Load all cached entries immediately
                    match cache.load_all_cached() {
                        Ok(cached_entries) if !cached_entries.is_empty() => {
                            has_cache = true;
                            
                            // Convert CachedEntry to minimal CandidateState for display
                            let cached_states: Vec<CandidateState> = cached_entries
                                .into_iter()
                                .map(|entry| {
                                    // Create minimal Candidate from cache
                                    let candidate = Candidate {
                                        path: entry.path.clone(),
                                        kind: TargetKind::Other("cached".to_string()),
                                        size_bytes: entry.size_bytes,
                                        last_modified: Some(entry.last_seen),
                                        last_accessed: None,
                                        reproducibility: 0.8,
                                        score: 0.5,
                                        tags: vec!["cached".to_string()],
                                        action: Action::Skip,
                                        group: None,
                                    };
                                    
                                    let mut state = CandidateState::new(candidate);
                                    state.cache_status = CacheStatus::CachedUnverified;
                                    state.first_seen = entry.first_seen;
                                    state.last_seen = entry.last_seen;
                                    state
                                })
                                .collect();
                            
                            let _ = tx.send(ScanMessage::CacheLoaded {
                                candidates: cached_states,
                            });
                        }
                        _ => {
                            eprintln!("No cached data available, starting fresh scan");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Cache open failed: {e}, starting fresh scan");
                }
            }

            
            // Phase 1: QuickVerify - Verify cached items (1-5s)
            if has_cache {
                let _ = tx.send(ScanMessage::PhaseChange { 
                    phase: ScanPhase::QuickVerify,
                    started_at: Utc::now(),
                });
                
                // Quick verification would go here - for now, skip to full scan
                // TODO: Implement quick verification that checks if cached paths still exist
            }
            
            // Phase 2: FullScan - Complete scan (10-60s)
            let _ = tx.send(ScanMessage::PhaseChange { 
                phase: ScanPhase::FullScan,
                started_at: Utc::now(),
            });
            
            let mut all_candidates = Vec::new();
            
            for (idx, root) in roots.iter().enumerate() {
                let progress = (idx as f32) / (roots.len() as f32);
                let _ = tx.send(ScanMessage::Progress {
                    current: root.display().to_string(),
                    total: roots.len(),
                    done: idx,
                    phase_progress: progress,
                });

                match scanner::scan(&[root.clone()], &profile) {
                    Ok(mut candidates) => {
                        strategy::apply(&mut candidates, &profile);
                        all_candidates.extend(candidates);
                    }
                    Err(e) => {
                        let _ = tx.send(ScanMessage::Error(format!("Scan failed: {e}")));
                        return;
                    }
                }
            }
            
            // Merge with cache to detect new/changed items and restore user selections
            let candidate_states = match ScanCache::open_default() {
                Ok(mut cache) => {
                    match cache.merge_scan_results(all_candidates) {
                        Ok((states, stats)) => {
                            let _ = tx.send(ScanMessage::Complete {
                                candidates: states,
                                stats,
                                profile: profile.clone(),
                            });
                            (cache, true)
                        }
                        Err(e) => {
                            eprintln!("Cache merge failed: {e}");
                            let _ = tx.send(ScanMessage::Error(format!("Cache merge failed: {e}")));
                            return;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Cache open failed: {e}");
                    let _ = tx.send(ScanMessage::Error(format!("Cache error: {e}")));
                    return;
                }
            };

            // Phase 3: DiskAnalysis - Full disk categorization (30-120s, optional)
            if need_disk_analysis {
                let _ = tx.send(ScanMessage::PhaseChange { 
                    phase: ScanPhase::DiskAnalysis,
                    started_at: Utc::now(),
                });
                
                // Disk analysis is triggered separately via analyze_disk() method
            }
        });
    }

    /// Auto-start scan with default roots on app launch

    /// Auto-start scan with default roots on app launch
    fn start_auto_scan(&mut self) {
        // Check permissions first on macOS
        #[cfg(target_os = "macos")]
        {
            if !Self::check_full_disk_access() {
                // Don't start scan without permissions - show dialog instead
                self.show_permission_dialog = true;
                self.permission_check_done = true;
                self.auto_scan_enabled = false;
                return;
            }
        }
        
        // Set default roots if not configured
        if self.scan_roots.is_empty() {
            let home = dirs::home_dir().unwrap_or_default();
            
            // Default roots: home, repos, Library (macOS specific)
            let mut default_roots = vec![home.clone()];
            
            // Add common development directories
            let repos = home.join("repos");
            if repos.exists() {
                default_roots.push(repos);
            }
            
            let projects = home.join("Projects");
            if projects.exists() {
                default_roots.push(projects);
            }
            
            #[cfg(target_os = "macos")]
            {
                let library = home.join("Library");
                if library.exists() {
                    default_roots.push(library);
                }
            }
            
            self.scan_roots = default_roots;
        }
        
        // Load default profile if not set
        if self.profile_name.is_empty() {
            self.profile_name = "conservative".to_string();
        }
        
        self.auto_scan_enabled = true;
        self.scan();
    }

    /// Pause the background scan
    fn pause_scan(&mut self) {
        if let ScanStatus::Scanning { .. } = self.scan_status {
            self.scan_status = ScanStatus::Paused;
            self.status_message = "Scan paused by user".to_string();
            // Note: Current implementation doesn't actually pause the thread
            // TODO: Implement proper thread pause/resume with Arc<AtomicBool>
        }
    }

    /// Resume the paused scan
    fn resume_scan(&mut self) {
        if self.scan_status == ScanStatus::Paused {
            // For now, restart the scan
            // TODO: Implement proper thread resume
            self.scan();
        }
    }
    
    /// Check if Full Disk Access is granted on macOS
    #[cfg(target_os = "macos")]
    fn check_full_disk_access() -> bool {
        use std::fs;
        use std::path::PathBuf;
        
        // Try to access a protected directory (Safari cache)
        let home = dirs::home_dir().unwrap_or_default();
        let safari_cache = home.join("Library/Caches/com.apple.Safari");
        
        if !safari_cache.exists() {
            // If Safari cache doesn't exist, assume we don't need permission
            return true;
        }
        
        // Try to read the directory
        match fs::read_dir(&safari_cache) {
            Ok(_) => true,
            Err(e) => {
                // Permission denied means we need Full Disk Access
                e.kind() != std::io::ErrorKind::PermissionDenied
            }
        }
    }
    
    #[cfg(not(target_os = "macos"))]
    fn check_full_disk_access() -> bool {
        true // Not needed on other platforms
    }
    
    /// Open System Settings to Full Disk Access pane
    #[cfg(target_os = "macos")]
    fn open_system_settings_permissions() {
        use std::process::Command;
        
        // Try to open System Settings (macOS 13+) or System Preferences (older)
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .spawn();
    }
    
    #[cfg(not(target_os = "macos"))]
    fn open_system_settings_permissions() {
        // No-op on other platforms
    }
    
    fn check_and_prompt_permissions(&mut self) {
        if !self.permission_check_done && !self.dont_ask_permissions {
            self.permission_check_done = true;
            
            if !Self::check_full_disk_access() {
                self.show_permission_dialog = true;
            }
        }
    }
    
    /// Start disk analysis to get overview of space usage
    fn analyze_disk(&mut self) {
        if self.disk_analysis_receiver.is_some() {
            return; // Already analyzing
        }
        
        self.status_message = "Analyzing disk usage...".to_string();
        
        // Use home dir for analysis
        let root = dirs::home_dir().unwrap_or_default();
        let depth = self.disk_analysis_depth;
        
        // Create channel
        let (tx, rx) = unbounded();
        self.disk_analysis_receiver = Some(rx);
        
        // Spawn background thread for analysis
        std::thread::spawn(move || {
            // Analyze with configured depth
            match analyze_disk(&root, Some(depth)) {
                Ok((_entries, stats)) => {
                    let _ = tx.send(DiskAnalysisMessage::Complete(stats));
                }
                Err(e) => {
                    let _ = tx.send(DiskAnalysisMessage::Error(format!("Analysis failed: {}", e)));
                }
            }
        });
    }


    fn start_verification_thread(&mut self, ctx: &egui::Context) {
        if self.candidates.is_empty() {
            return;
        }

        self.verification_in_progress = true;
        self.status_message = "Starting cache verification...".to_string();

        let candidates = self.candidates.clone();
        let (tx, rx) = unbounded();
        self.verification_receiver = Some(rx);

        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            let verifier = Tier1Verifier::new(tx);
            match verifier.verify_all(candidates) {
                Ok(_) => {
                    // Messages already sent by verifier
                    ctx_clone.request_repaint();
                }
                Err(e) => {
                    eprintln!("Verification failed: {}", e);
                }
            }
        });
    }

    fn start_discovery_thread(&mut self, ctx: &egui::Context) {
        if self.scan_roots.is_empty() || self.profile.is_none() {
            return;
        }

        self.discovery_in_progress = true;
        self.status_message = "Starting hot paths discovery...".to_string();

        let roots = self.scan_roots.clone();
        let profile = self.profile.clone().unwrap();
        let known_paths: HashSet<PathBuf> = self.candidates.iter()
            .map(|c| c.candidate.path.clone())
            .collect();
        let last_scan = self.last_discovery;

        let (tx, rx) = unbounded();
        self.discovery_receiver = Some(rx);

        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            let discoverer = HotPathsDiscoverer::new(tx, profile, known_paths);
            match discoverer.discover(roots, last_scan) {
                Ok(_) => {
                    // Messages already sent by discoverer
                    ctx_clone.request_repaint();
                }
                Err(e) => {
                    eprintln!("Discovery failed: {}", e);
                }
            }
        });
    }

    fn load_profile(&self) -> Result<Profile> {
        let profile_path = match self.profile_name.as_str() {
            "" | "conservative" => self.bundled_profile("conservative"),
            "aggressive"        => self.bundled_profile("aggressive"),
            "dev"               => self.bundled_profile("dev"),
            custom              => PathBuf::from(custom),
        };
        
        // Try to load from file, fallback to built-in conservative profile
        match Profile::load(&profile_path) {
            Ok(profile) => Ok(profile),
            Err(_) => {
                // Fallback to built-in conservative profile
                eprintln!("Warning: Could not load profile from {:?}, using built-in conservative profile", profile_path);
                Ok(Profile::default_conservative())
            }
        }
    }

    fn bundled_profile(&self, name: &str) -> PathBuf {
        let binary_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_default();
        
        let cwd = std::env::current_dir().unwrap_or_default();
        
        // Search in multiple locations:
        // 1. Binary directory (for installed/release builds)
        // 2. Current working directory (for development)
        // 3. Parent directories (for running from target/release/)
        let mut search_paths: Vec<PathBuf> = vec![
            binary_dir.join("profiles"),
            cwd.join("profiles"),
        ];
        
        // Add parent directory paths if they exist
        if let Some(parent) = cwd.parent().and_then(|p| p.parent()) {
            search_paths.push(parent.join("profiles"));  // ../../profiles
        }
        if let Some(parent) = binary_dir.parent().and_then(|p| p.parent()) {
            search_paths.push(parent.join("profiles"));  // ../../profiles from binary
        }
        
        for base in search_paths {
            let candidate = base.join(format!("{name}.toml"));
            if candidate.exists() {
                return candidate;
            }
        }
        
        PathBuf::from(format!("profiles/{name}.toml"))
    }

    /// Apply post-scan filters dynamically (no rescan needed).
    fn filtered_candidates(&self) -> Vec<&CandidateState> {
        self.candidates.iter().filter(|c| {
            let cand = &c.candidate;
            
            // Categorical filters
            if !self.enabled_kinds.is_empty() && !self.enabled_kinds.contains(cand.kind.label()) {
                return false;
            }
            if !self.enabled_actions.is_empty() && !self.enabled_actions.contains(&cand.action.display()) {
                return false;
            }

            // Size filters (0 = no limit)
            if self.filter_min_size_mb > 0.0 {
                let min_bytes = (self.filter_min_size_mb * 1024.0 * 1024.0) as u64;
                if cand.size_bytes < min_bytes {
                    return false;
                }
            }
            if self.filter_max_size_gb > 0.0 {
                let max_bytes = (self.filter_max_size_gb * 1024.0 * 1024.0 * 1024.0) as u64;
                if cand.size_bytes > max_bytes {
                    return false;
                }
            }

            // Age filters (0 = no limit)
            if let Some(age) = cand.age_days() {
                let age = age as u32;
                if self.filter_min_age_days > 0 && age < self.filter_min_age_days {
                    return false;
                }
                if self.filter_max_age_days > 0 && age > self.filter_max_age_days {
                    return false;
                }
            }

            // Score filter
            if cand.score < self.filter_min_score {
                return false;
            }

            // Selected-only filter (use CandidateState's selection tracking)
            if self.show_only_selected && !c.is_selected() {
                return false;
            }

            true
        }).collect()
    }

    /// Extract unique filter values from candidates and initialize enabled sets if empty.
    fn extract_filter_values(&mut self) {
        // Extract unique kinds
        self.available_kinds = self.candidates.iter()
            .map(|c| c.candidate.kind.label().to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        self.available_kinds.sort();
        
        // Extract unique actions
        self.available_actions = self.candidates.iter()
            .map(|c| c.candidate.action.display().to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        self.available_actions.sort();
        
        // Extract unique groups
        self.available_groups = self.candidates.iter()
            .filter_map(|c| c.candidate.group.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        self.available_groups.sort();
        
        // Initialize enabled sets to "all" if empty (first scan)
        if self.enabled_kinds.is_empty() {
            self.enabled_kinds = self.available_kinds.iter().cloned().collect();
        }
        if self.enabled_actions.is_empty() {
            self.enabled_actions = self.available_actions.iter().cloned().collect();
        }
        if self.enabled_groups.is_empty() {
            self.enabled_groups = self.available_groups.iter().cloned().collect();
        }
    }
    
    /// Refresh groups from current candidates
    fn refresh_groups(&mut self) {
        // Convert CandidateState to Candidate for grouping
        let candidates: Vec<reclaim_core::candidate::Candidate> = self.candidates.iter()
            .map(|cs| cs.candidate.clone())
            .collect();
        
        self.groups = reclaim_core::grouping::group_candidates(&candidates);
        
        // Filter out Single-item groups - they're useless
        self.groups.retain(|g| g.group_type != reclaim_core::grouping::GroupType::Single);
        
        // Auto-expand Duplicate groups by default
        for group in &self.groups {
            if group.group_type == reclaim_core::grouping::GroupType::Duplicates {
                self.expanded_groups.insert(group.id.clone());
            }
        }
    }

    /// Get all unique TargetKind labels present in candidates.
    fn available_kinds(&self) -> Vec<String> {
        let mut kinds: Vec<_> = self.candidates.iter()
            .map(|c| c.candidate.kind.label().to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        kinds.sort();
        kinds
    }

    /// Get all unique Action labels present in candidates.
    fn available_actions(&self) -> Vec<String> {
        let mut actions: Vec<_> = self.candidates.iter()
            .map(|c| c.candidate.action.display().to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        actions.sort();
        actions
    }

    fn sort_by_mode(candidates: &mut Vec<CandidateState>, sort_by: SortBy) {
        match sort_by {
            SortBy::Score => candidates.sort_by(|a, b| b.candidate.score.partial_cmp(&a.candidate.score).unwrap()),
            SortBy::Size  => candidates.sort_by(|a, b| b.candidate.size_bytes.cmp(&a.candidate.size_bytes)),
            SortBy::Age   => candidates.sort_by(|a, b| {
                b.candidate.age_days().unwrap_or(0).cmp(&a.candidate.age_days().unwrap_or(0))
            }),
            SortBy::Kind  => candidates.sort_by(|a, b| a.candidate.kind.label().cmp(b.candidate.kind.label())),
        }
    }

    fn selected_size(&self) -> u64 {
        self.candidates.iter()
            .filter(|c| c.is_selected())
            .map(|c| c.candidate.size_bytes)
            .sum()
    }

    fn apply_cleanup(&mut self) -> (u64, Vec<String>) {
        let mut freed = 0u64;
        let mut errors = Vec::new();

        let mut to_remove = Vec::new();
        for (idx, c) in self.candidates.iter().enumerate() {
            if !c.is_selected() {
                continue;
            }

            let result = match &c.candidate.action {
                Action::Delete => {
                    if c.candidate.path.is_dir() {
                        std::fs::remove_dir_all(&c.candidate.path)
                    } else {
                        std::fs::remove_file(&c.candidate.path)
                    }
                }
                Action::Exec { cmd, args, .. } => {
                    std::process::Command::new(cmd)
                        .args(args)
                        .status()
                        .map(|_| ())
                }
                _ => continue,
            };

            match result {
                Ok(_) => {
                    freed += c.candidate.size_bytes;
                    to_remove.push(idx);
                }
                Err(e) => errors.push(format!("{}: {e}", c.candidate.path.display())),
            }
        }

        // Remove processed items in reverse order to preserve indices
        for idx in to_remove.iter().rev() {
            self.candidates.remove(*idx);
        }

        (freed, errors)
    }

    /// Render candidates as a table (classic view)
    fn render_table_view(&mut self, ui: &mut egui::Ui) {
        if self.show_groups && !self.groups.is_empty() {
            self.render_grouped_table_view(ui);
            return;
        }
        
        egui::Grid::new("candidates_grid")
            .num_columns(8)
            .striped(true)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                // Header - clickable for sorting
                ui.label("Select");
                ui.label("Status");
                
                if ui.button("Size ▼").clicked() {
                    self.sort_by = SortBy::Size;
                    Self::sort_by_mode(&mut self.candidates, self.sort_by);
                    self.scroll_to_top = true;
                }
                
                if ui.button("Kind ▼").clicked() {
                    self.sort_by = SortBy::Kind;
                    Self::sort_by_mode(&mut self.candidates, self.sort_by);
                    self.scroll_to_top = true;
                }
                
                if ui.button("Score ▼").clicked() {
                    self.sort_by = SortBy::Score;
                    Self::sort_by_mode(&mut self.candidates, self.sort_by);
                    self.scroll_to_top = true;
                }
                
                if ui.button("Age ▼").clicked() {
                    self.sort_by = SortBy::Age;
                    Self::sort_by_mode(&mut self.candidates, self.sort_by);
                    self.scroll_to_top = true;
                }
                
                ui.label("Action");
                ui.label("Path");
                ui.end_row();

                // Rows - apply filters inline
                for c in self.candidates.iter_mut() {
                    if !Self::passes_filters(
                        c,
                        &self.enabled_kinds,
                        &self.enabled_actions,
                        &self.enabled_groups,
                        self.filter_min_score,
                        self.show_only_selected,
                        self.filter_min_size_mb,
                        self.filter_max_size_gb,
                        self.filter_min_age_days,
                        self.filter_max_age_days,
                    ) {
                        continue;
                    }

                    let mut selected = c.is_selected();
                    if ui.checkbox(&mut selected, "").changed() {
                        c.set_checked(selected);
                    }

                    // Cache status badge
                    ui.label(c.cache_status.badge())
                        .on_hover_text(format!(
                            "Status: {}\nLast verified: {}",
                            match c.cache_status {
                                reclaim_core::selection::CacheStatus::Unknown => "Unknown",
                                reclaim_core::selection::CacheStatus::CachedUnverified => "Cached (not yet verified)",
                                reclaim_core::selection::CacheStatus::CachedVerified => "Verified unchanged",
                                reclaim_core::selection::CacheStatus::Changed => "Changed since cache",
                                reclaim_core::selection::CacheStatus::New => "New this scan",
                            },
                            c.last_verified.format("%Y-%m-%d %H:%M")
                        ));

                    // Size with estimation indicator
                    let size_text = if c.cache_status.is_estimation() {
                        format!("~{}", c.candidate.size_human())
                    } else {
                        c.candidate.size_human()
                    };
                    ui.label(size_text);
                    ui.label(c.candidate.kind.label());
                    
                    let score_color = Self::score_color(c.candidate.score);
                    ui.colored_label(score_color, format!("{:.2}", c.candidate.score));
                    
                    ui.label(c.candidate.age_days().map(|d| d.to_string()).unwrap_or("?".to_string()));
                    ui.label(c.candidate.action.display());
                    ui.label(c.candidate.path.display().to_string()).on_hover_text(c.candidate.path.display().to_string());
                    
                    ui.end_row();
                }
            });
    }
    
    /// Render candidates grouped (duplicates, similar names, same directory)
    fn render_grouped_table_view(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("grouped_candidates_grid")
            .num_columns(9)
            .striped(true)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                // Header
                ui.label(egui::RichText::new("").size(11.0)); // Expand
                ui.label(egui::RichText::new("Select").size(11.0).strong().color(egui::Color32::from_rgb(140, 140, 150)));
                ui.label(egui::RichText::new("Name/Group").size(11.0).strong().color(egui::Color32::from_rgb(140, 140, 150)));
                ui.label(egui::RichText::new("Kind").size(11.0).strong().color(egui::Color32::from_rgb(140, 140, 150)));
                ui.label(egui::RichText::new("Cache").size(11.0).strong().color(egui::Color32::from_rgb(140, 140, 150)));
                ui.label(egui::RichText::new("Size").size(11.0).strong().color(egui::Color32::from_rgb(140, 140, 150)));
                ui.label(egui::RichText::new("Score").size(11.0).strong().color(egui::Color32::from_rgb(140, 140, 150)));
                ui.label(egui::RichText::new("Location").size(11.0).strong().color(egui::Color32::from_rgb(140, 140, 150)));
                ui.label(egui::RichText::new("Action").size(11.0).strong().color(egui::Color32::from_rgb(140, 140, 150)));
                ui.end_row();
                
                // Clone groups to avoid borrow issues
                let groups = self.groups.clone();
                
                for group in groups.iter() {
                    let is_expanded = self.expanded_groups.contains(&group.id);
                    let expand_icon = if is_expanded { "▼" } else { "▶" };
                    
                    // Group header row
                    let button_text = egui::RichText::new(expand_icon)
                        .size(13.0)
                        .color(egui::Color32::from_rgb(100, 200, 255));
                    
                    if ui.button(button_text).clicked() {
                        if is_expanded {
                            self.expanded_groups.remove(&group.id);
                        } else {
                            self.expanded_groups.insert(group.id.clone());
                        }
                    }
                    
                    // Empty select column for group header
                    ui.label("");
                    
                    // Group name with color
                    let name_color = match &group.group_type {
                        reclaim_core::grouping::GroupType::Duplicates => egui::Color32::from_rgb(255, 150, 100),
                        reclaim_core::grouping::GroupType::SimilarNames { .. } => egui::Color32::from_rgb(150, 200, 255),
                        _ => egui::Color32::from_rgb(200, 200, 200),
                    };
                    ui.label(egui::RichText::new(&group.name).strong().color(name_color));
                    
                    let type_label = match &group.group_type {
                        reclaim_core::grouping::GroupType::Duplicates => "🔁 Duplicates",
                        reclaim_core::grouping::GroupType::SameDirectory => "📁 Same Dir",
                        reclaim_core::grouping::GroupType::CommonAncestor { depth } => 
                            &format!("📂 Common ({})", depth),
                        reclaim_core::grouping::GroupType::SimilarNames { .. } => "📄 Similar",
                        reclaim_core::grouping::GroupType::Single => "📄 Single",
                    };
                    ui.label(type_label);
                    
                    // Empty cache column for group header
                    ui.label("");
                    
                    // Size with color
                    let size_text = reclaim_core::candidate::human_bytes(group.total_size);
                    let size_color = if group.total_size > 1_000_000_000 {
                        egui::Color32::from_rgb(255, 100, 100)
                    } else if group.total_size > 100_000_000 {
                        egui::Color32::from_rgb(255, 200, 100)
                    } else {
                        egui::Color32::from_rgb(200, 200, 200)
                    };
                    ui.label(egui::RichText::new(&size_text).color(size_color).strong());
                    
                    // Show item count instead of score for group header
                    ui.label(egui::RichText::new(format!("{} items", group.candidates.len()))
                        .color(egui::Color32::from_rgb(140, 140, 150)));
                    
                    // Show common ancestor or parent
                    let location = if let Some(ref ancestor) = group.common_ancestor {
                        ancestor.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    } else if let Some(ref parent) = group.parent_path {
                        parent.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    } else {
                        "-".to_string()
                    };
                    
                    let full_path = if let Some(ref ancestor) = group.common_ancestor {
                        ancestor.display().to_string()
                    } else if let Some(ref parent) = group.parent_path {
                        parent.display().to_string()
                    } else {
                        "-".to_string()
                    };
                    
                    ui.label(egui::RichText::new(&location).color(egui::Color32::from_rgb(140, 140, 150)))
                        .on_hover_text(&full_path);
                    
                    // Action buttons for group
                    let mut show_genealogy = false;
                    let first_candidate_idx = group.candidates.first().copied();
                    
                    ui.horizontal(|ui| {
                        // Explore button - opens genealogy window
                        let explore_btn = egui::Button::new(egui::RichText::new("🔍").size(12.0))
                            .small();
                        if ui.add(explore_btn)
                            .on_hover_text("Explore folder hierarchy and genealogy")
                            .clicked() 
                        {
                            show_genealogy = true;
                        }
                    });
                    
                    // Handle actions outside the closure
                    if show_genealogy {
                        if let Some(first_idx) = first_candidate_idx {
                            if let Some(candidate) = self.candidates.get(first_idx) {
                                let all_candidates: Vec<reclaim_core::candidate::Candidate> = 
                                    self.candidates.iter().map(|cs| cs.candidate.clone()).collect();
                                self.genealogy_context = Some(
                                    reclaim_core::grouping::get_directory_context(
                                        &candidate.candidate.path,
                                        &all_candidates
                                    )
                                );
                                self.show_genealogy_window = true;
                            }
                        }
                    }
                    
                    ui.end_row();
                    
                    // Expanded group items
                    if is_expanded {
                        for &candidate_idx in &group.candidates {
                            if let Some(c) = self.candidates.get_mut(candidate_idx) {
                                // Apply filters
                                if !Self::passes_filters(
                                    c,
                                    &self.enabled_kinds,
                                    &self.enabled_actions,
                                    &self.enabled_groups,
                                    self.filter_min_score,
                                    self.show_only_selected,
                                    self.filter_min_size_mb,
                                    self.filter_max_size_gb,
                                    self.filter_min_age_days,
                                    self.filter_max_age_days,
                                ) {
                                    continue;
                                }
                                
                                // Empty expand column
                                ui.label("  ");
                                
                                // Checkbox
                                let mut selected = c.is_selected();
                                if ui.checkbox(&mut selected, "").changed() {
                                    c.set_checked(selected);
                                }
                                
                                // Filename (shortened within group)
                                let filename = c.candidate.path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();
                                ui.label(egui::RichText::new(&filename).color(egui::Color32::from_rgb(180, 180, 190)));
                                
                                // Kind badge (artifact type)
                                ui.label(egui::RichText::new(c.candidate.kind.label())
                                    .size(10.0)
                                    .color(egui::Color32::from_rgb(100, 180, 220)));
                                
                                // Cache status badge
                                ui.label(c.cache_status.badge());
                                
                                // Size
                                let size_text = if c.cache_status.is_estimation() {
                                    format!("~{}", c.candidate.size_human())
                                } else {
                                    c.candidate.size_human()
                                };
                                ui.label(size_text);
                                
                                // Score with color
                                let score_color = Self::score_color(c.candidate.score);
                                ui.colored_label(score_color, format!("{:.2}", c.candidate.score));
                                
                                // Relative path
                                let path_display = if let Some(ref ancestor) = group.common_ancestor {
                                    c.candidate.path.strip_prefix(ancestor)
                                        .map(|p| p.display().to_string())
                                        .unwrap_or_else(|_| c.candidate.path.display().to_string())
                                } else {
                                    c.candidate.path.parent()
                                        .and_then(|p| p.file_name())
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .to_string()
                                };
                                ui.label(egui::RichText::new(&path_display)
                                    .size(10.0)
                                    .color(egui::Color32::from_rgb(120, 120, 130)))
                                    .on_hover_text(c.candidate.path.display().to_string());
                                
                                // Action
                                ui.label(egui::RichText::new(c.candidate.action.display())
                                    .size(10.0)
                                    .color(egui::Color32::from_rgb(140, 140, 150)));
                                
                                ui.end_row();
                            }
                        }
                    }
                }
            });
    }

    /// Render treemap view - graphical hierarchical view with area-proportional rectangles
    fn render_treemap_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("🗺 Disk Treemap - Graphical View");
        ui.add_space(10.0);
        
        // Calculate total size for proportional display
        let total_size: u64 = self.groups.iter()
            .map(|g| {
                g.candidates.iter()
                    .filter_map(|&idx| self.candidates.get(idx))
                    .map(|c| c.candidate.size_bytes)
                    .sum::<u64>()
            })
            .sum();
        
        if total_size == 0 {
            ui.label("No data to display. Run a scan first.");
            return;
        }
        
        ui.label(format!("Total space: {}", reclaim_core::candidate::human_bytes(total_size)));
        ui.add_space(10.0);
        
        // Get available space for drawing
        let available_size = ui.available_size();
        let margin = 10.0;
        let total_width = available_size.x - margin * 2.0;
        let total_height = available_size.y - margin * 2.0;
        
        if total_width <= 0.0 || total_height <= 0.0 {
            return;
        }
        
        // Group rectangles data
        let mut rects: Vec<(String, u64, egui::Color32, String)> = Vec::new();
        
        for group in &self.groups {
            let group_size: u64 = group.candidates.iter()
                .filter_map(|&idx| self.candidates.get(idx))
                .map(|c| c.candidate.size_bytes)
                .sum();
            
            if group_size == 0 {
                continue;
            }
            
            // Color based on group type
            let color = match &group.group_type {
                reclaim_core::grouping::GroupType::Duplicates => egui::Color32::from_rgb(220, 100, 100),
                reclaim_core::grouping::GroupType::SimilarNames { .. } => egui::Color32::from_rgb(100, 180, 220),
                reclaim_core::grouping::GroupType::SameDirectory => egui::Color32::from_rgb(150, 200, 100),
                reclaim_core::grouping::GroupType::CommonAncestor { .. } => egui::Color32::from_rgb(180, 150, 200),
                reclaim_core::grouping::GroupType::Single => egui::Color32::from_rgb(120, 120, 130),
            };
            
            let label = group.name.clone();
            let group_type_label = match &group.group_type {
                reclaim_core::grouping::GroupType::Duplicates => "Duplicates",
                reclaim_core::grouping::GroupType::SimilarNames { .. } => "Similar Names",
                reclaim_core::grouping::GroupType::SameDirectory => "Same Directory",
                reclaim_core::grouping::GroupType::CommonAncestor { .. } => "Common Ancestor",
                reclaim_core::grouping::GroupType::Single => "Single",
            };
            let hover_text = format!(
                "{}\n{} items\n{}\n{}",
                group.name,
                group.candidates.len(),
                reclaim_core::candidate::human_bytes(group_size),
                group_type_label
            );
            
            rects.push((label, group_size, color, hover_text));
        }
        
        // Simple row-based layout (squarified treemap is complex, start simple)
        let mut y_offset = margin;
        let mut current_row_rects: Vec<(String, u64, egui::Color32, String, egui::Rect)> = Vec::new();
        let mut current_row_size = 0u64;
        let target_row_height = 80.0;
        
        for (label, size, color, hover) in rects {
            current_row_rects.push((label, size, color, hover, egui::Rect::NOTHING));
            current_row_size += size;
            
            // Check if row is full enough (simple heuristic: fill ~70% of total)
            let row_fraction = current_row_size as f64 / total_size as f64;
            if row_fraction >= 0.15 || y_offset + target_row_height >= total_height + margin {
                // Layout this row
                let row_height = if y_offset + target_row_height < total_height + margin {
                    target_row_height
                } else {
                    (total_height + margin - y_offset).max(30.0)
                };
                
                let mut x_offset = margin;
                for i in 0..current_row_rects.len() {
                    let (ref label, size, color, ref hover, _) = current_row_rects[i];
                    let rect_width = (size as f64 / current_row_size as f64 * total_width as f64) as f32;
                    
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(x_offset, y_offset),
                        egui::vec2(rect_width - 2.0, row_height - 2.0),
                    );
                    
                    current_row_rects[i].4 = rect;
                    x_offset += rect_width;
                }
                
                // Draw the row
                for (label, _size, color, hover, rect) in &current_row_rects {
                    ui.painter().rect_filled(*rect, 4.0, *color);
                    
                    // Draw label if rect is big enough
                    if rect.width() > 60.0 && rect.height() > 30.0 {
                        let text_pos = rect.min + egui::vec2(5.0, 5.0);
                        ui.painter().text(
                            text_pos,
                            egui::Align2::LEFT_TOP,
                            label,
                            egui::FontId::proportional(12.0),
                            egui::Color32::WHITE,
                        );
                    }
                    
                    // Interaction
                    let response = ui.interact(*rect, ui.id().with(label), egui::Sense::click())
                        .on_hover_text(hover);
                    if response.hovered() {
                        ui.painter().rect_stroke(*rect, 4.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                    }
                    
                    if response.clicked() {
                        // TODO: Drill down into group
                        // For now, just toggle expansion
                        let group_id = label.clone();
                        if self.expanded_groups.contains(&group_id) {
                            self.expanded_groups.remove(&group_id);
                        } else {
                            self.expanded_groups.insert(group_id);
                        }
                    }
                }
                
                y_offset += row_height;
                current_row_rects.clear();
                current_row_size = 0;
            }
        }
        
        // Draw remaining rects if any
        if !current_row_rects.is_empty() {
            let row_height = (total_height + margin - y_offset).max(30.0);
            let mut x_offset = margin;
            
            for (label, size, color, hover, _) in &current_row_rects {
                let rect_width = (*size as f64 / current_row_size as f64 * total_width as f64) as f32;
                
                let rect = egui::Rect::from_min_size(
                    egui::pos2(x_offset, y_offset),
                    egui::vec2(rect_width - 2.0, row_height - 2.0),
                );
                
                ui.painter().rect_filled(rect, 4.0, *color);
                
                if rect.width() > 60.0 && rect.height() > 30.0 {
                    let text_pos = rect.min + egui::vec2(5.0, 5.0);
                    ui.painter().text(
                        text_pos,
                        egui::Align2::LEFT_TOP,
                        label,
                        egui::FontId::proportional(12.0),
                        egui::Color32::WHITE,
                    );
                }
                
                let response = ui.interact(rect, ui.id().with(label), egui::Sense::click())
                    .on_hover_text(hover);
                if response.hovered() {
                    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                }
                
                if response.clicked() {
                    let group_id = label.clone();
                    if self.expanded_groups.contains(&group_id) {
                        self.expanded_groups.remove(&group_id);
                    } else {
                        self.expanded_groups.insert(group_id);
                    }
                }
                
                x_offset += rect_width;
            }
        }
    }

    /// Render disk overview with category breakdown
    fn render_disk_overview(&mut self, ui: &mut egui::Ui) {
        ui.heading("📊 Disk Space Overview");
        ui.add_space(10.0);
        
        // Settings panel
        ui.horizontal(|ui| {
            ui.label("Scan Depth:");
            ui.add(egui::Slider::new(&mut self.disk_analysis_depth, 1..=10)
                .text("levels"));
            
            ui.separator();
            
            if self.disk_analysis_receiver.is_none() {
                if ui.button("🔍 Analyze").clicked() {
                    self.disk_stats = None;
                    self.disk_selected_category = None;
                    self.analyze_disk();
                }
            } else {
                ui.spinner();
                ui.label("Analyzing...");
            }
        });
        
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        
        // Show analysis in progress or start prompt
        if self.disk_stats.is_none() {
            if self.disk_analysis_receiver.is_none() {
                ui.vertical_centered(|ui| {
                    ui.heading("👆 Click 'Analyze' to start");
                    ui.add_space(5.0);
                    ui.label("This will scan your home directory and categorize all files");
                });
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.spinner();
                    ui.add_space(10.0);
                    ui.heading("Scanning disk...");
                });
            }
            return;
        }
        
        // Show results
        if let Some(stats) = &self.disk_stats.clone() {
            // Summary stats
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("📦 Total: {}", 
                    reclaim_core::candidate::human_bytes(stats.total_bytes)))
                    .size(16.0));
                ui.separator();
                ui.colored_label(
                    egui::Color32::from_rgb(76, 175, 80),
                    egui::RichText::new(format!("♻️ Reclaimable: {:.1}% ({})", 
                        stats.reclaimable_percentage(),
                        reclaim_core::candidate::human_bytes(stats.reclaimable_bytes)))
                        .size(16.0)
                );
            });
            
            ui.add_space(20.0);
            
            // Two-column layout: Pie chart + Details
            let stats_clone = stats.clone();
            let selected = self.disk_selected_category.clone();
            ui.horizontal(|ui| {
                // Left: Pie chart
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    self.render_pie_chart(ui, &stats_clone);
                });
                
                ui.separator();
                
                // Right: Category breakdown or drill-down
                ui.vertical(|ui| {
                    if let Some(sel) = &selected {
                        self.render_category_drilldown(ui, &stats_clone, sel);
                    } else {
                        self.render_category_list(ui, &stats_clone);
                    }
                });
            });
            
            ui.add_space(20.0);
            ui.separator();
            ui.add_space(10.0);
            
            // Action buttons
            let stats_for_export = stats.clone();
            ui.horizontal(|ui| {
                if ui.button("🔄 Refresh Analysis").clicked() {
                    self.disk_stats = None;
                    self.disk_selected_category = None;
                    self.analyze_disk();
                }
                
                if ui.button("📥 Export JSON").clicked() {
                    self.export_disk_report(&stats_for_export);
                }
                
                ui.add_space(10.0);
                
                if ui.button("➡️ View Reclaimable Items").clicked() {
                    self.view_mode = ViewMode::Table;
                }
            });
        }
    }
    
    /// Render pie chart visualization
    fn render_pie_chart(&self, ui: &mut egui::Ui, stats: &DiskStats) {
        let size = egui::vec2(250.0, 250.0);
        let (response, painter) = ui.allocate_painter(size, egui::Sense::hover());
        
        let rect = response.rect;
        let center = rect.center();
        let radius = rect.width().min(rect.height()) / 2.0 - 10.0;
        
        // Collect categories with non-zero size
        let mut categories: Vec<_> = stats.by_category.iter()
            .filter(|(_, bytes)| **bytes > 0)
            .collect();
        categories.sort_by(|a, b| b.1.cmp(a.1));
        
        let mut start_angle = 0.0;
        
        for (category, bytes) in categories {
            let percentage = stats.category_percentage(category);
            let angle = (percentage / 100.0) * std::f32::consts::TAU;
            
            if angle < 0.01 {
                continue; // Skip very small slices
            }
            
            // Color for category
            let color = match category {
                DiskCategory::System => egui::Color32::from_rgb(100, 100, 100),
                DiskCategory::Media => egui::Color32::from_rgb(233, 30, 99),
                DiskCategory::Documents => egui::Color32::from_rgb(33, 150, 243),
                DiskCategory::Code => egui::Color32::from_rgb(255, 152, 0),
                DiskCategory::Reclaimable => egui::Color32::from_rgb(76, 175, 80),
                DiskCategory::Other => egui::Color32::from_rgb(158, 158, 158),
            };
            
            // Draw slice
            let points = 32;
            let mut path = Vec::new();
            path.push(center);
            
            for i in 0..=points {
                let t = i as f32 / points as f32;
                let a = start_angle + t * angle;
                let x = center.x + radius * a.cos();
                let y = center.y + radius * a.sin();
                path.push(egui::pos2(x, y));
            }
            
            painter.add(egui::Shape::convex_polygon(
                path,
                color,
                egui::Stroke::new(2.0, egui::Color32::WHITE),
            ));
            
            // Draw label at mid-angle
            if percentage > 5.0 {
                let mid_angle = start_angle + angle / 2.0;
                let label_radius = radius * 0.7;
                let label_x = center.x + label_radius * mid_angle.cos();
                let label_y = center.y + label_radius * mid_angle.sin();
                
                painter.text(
                    egui::pos2(label_x, label_y),
                    egui::Align2::CENTER_CENTER,
                    format!("{:.0}%", percentage),
                    egui::FontId::proportional(12.0),
                    egui::Color32::WHITE,
                );
            }
            
            start_angle += angle;
        }
        
        // Legend below
        ui.add_space(10.0);
        for (category, bytes) in stats.by_category.iter() {
            if *bytes == 0 {
                continue;
            }
            
            ui.horizontal(|ui| {
                let color = match category {
                    DiskCategory::System => egui::Color32::from_rgb(100, 100, 100),
                    DiskCategory::Media => egui::Color32::from_rgb(233, 30, 99),
                    DiskCategory::Documents => egui::Color32::from_rgb(33, 150, 243),
                    DiskCategory::Code => egui::Color32::from_rgb(255, 152, 0),
                    DiskCategory::Reclaimable => egui::Color32::from_rgb(76, 175, 80),
                    DiskCategory::Other => egui::Color32::from_rgb(158, 158, 158),
                };
                
                let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, color);
                
                ui.label(format!("{} {}", category.emoji(), category.label()));
            });
        }
    }
    
    /// Render category list with clickable items
    fn render_category_list(&mut self, ui: &mut egui::Ui, stats: &DiskStats) {
        ui.heading("By Category");
        ui.add_space(10.0);
        
        let mut categories: Vec<_> = stats.by_category.iter().collect();
        categories.sort_by(|a, b| b.1.cmp(a.1));
        
        for (category, bytes) in categories {
            let pct = stats.category_percentage(category);
            
            if *bytes == 0 {
                continue;
            }
            
            let button_response = ui.add(
                egui::Button::new(
                    egui::RichText::new(format!("{} {} - {:.1}% ({})",
                        category.emoji(),
                        category.label(),
                        pct,
                        reclaim_core::candidate::human_bytes(*bytes)))
                        .size(14.0)
                )
                .min_size(egui::vec2(ui.available_width(), 30.0))
            );
            
            if button_response.clicked() {
                self.disk_selected_category = Some(category.clone());
            }
            
            ui.add_space(5.0);
        }
    }
    
    /// Render drill-down view for a specific category
    fn render_category_drilldown(&mut self, ui: &mut egui::Ui, stats: &DiskStats, category: &DiskCategory) {
        ui.horizontal(|ui| {
            if ui.button("← Back").clicked() {
                self.disk_selected_category = None;
            }
            ui.heading(format!("{} {}", category.emoji(), category.label()));
        });
        
        ui.add_space(10.0);
        
        let total_bytes = stats.by_category.get(category).copied().unwrap_or(0);
        ui.label(format!("Total: {} ({:.1}%)",
            reclaim_core::candidate::human_bytes(total_bytes),
            stats.category_percentage(category)
        ));
        
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        
        // Show subcategories for this category
        ui.label("Subcategories:");
        ui.add_space(5.0);
        
        let mut subcategories: Vec<_> = stats.by_subcategory.iter()
            .filter(|(subcat, _)| subcat.category() == *category)
            .collect();
        subcategories.sort_by(|a, b| b.1.cmp(a.1));
        
        for (subcategory, bytes) in subcategories {
            if *bytes == 0 {
                continue;
            }
            
            let pct = if total_bytes > 0 {
                (*bytes as f64 / total_bytes as f64 * 100.0) as f32
            } else {
                0.0
            };
            
            let is_reclaimable = subcategory.is_reclaimable();
            
            ui.horizontal(|ui| {
                if is_reclaimable {
                    ui.colored_label(
                        egui::Color32::from_rgb(76, 175, 80),
                        "♻️"
                    );
                } else {
                    ui.label("📁");
                }
                
                ui.label(subcategory.label());
                
                let bar_width = ui.available_width() - 120.0;
                ui.add(egui::ProgressBar::new(pct / 100.0)
                    .desired_width(bar_width)
                    .text(format!("{:.1}%", pct)));
                
                ui.label(reclaim_core::candidate::human_bytes(*bytes));
            });
            
            ui.add_space(3.0);
        }
    }
    
    /// Export disk analysis report to JSON
    fn export_disk_report(&self, stats: &DiskStats) {
        use std::fs;
        
        let json = match serde_json::to_string_pretty(stats) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("Failed to serialize stats: {}", e);
                return;
            }
        };
        
        let filename = format!("disk-report-{}.json", 
            chrono::Utc::now().format("%Y%m%d-%H%M%S"));
        
        let path = dirs::home_dir()
            .unwrap_or_default()
            .join(&filename);
        
        match fs::write(&path, json) {
            Ok(_) => {
                println!("✓ Report exported to: {}", path.display());
            }
            Err(e) => {
                eprintln!("Failed to write report: {}", e);
            }
        }
    }

    /// Check if candidate passes all active filters
    fn passes_filters(
        c: &CandidateState,
        enabled_kinds: &HashSet<String>,
        enabled_actions: &HashSet<String>,
        enabled_groups: &HashSet<String>,
        filter_min_score: f32,
        show_only_selected: bool,
        filter_min_size_mb: f32,
        filter_max_size_gb: f32,
        filter_min_age_days: u32,
        filter_max_age_days: u32,
    ) -> bool {
        let cand = &c.candidate;
        
        // Categorical filters
        if !enabled_kinds.contains(cand.kind.label()) {
            return false;
        }
        if !enabled_actions.contains(&cand.action.display()) {
            return false;
        }
        if let Some(ref group) = cand.group {
            if !enabled_groups.contains(group) {
                return false;
            }
        }
        
        // Score filter
        if cand.score < filter_min_score {
            return false;
        }
        
        // Show only selected
        if show_only_selected && !c.is_selected() {
            return false;
        }
        
        // Size filters
        if filter_min_size_mb > 0.0 {
            let min_bytes = (filter_min_size_mb * 1024.0 * 1024.0) as u64;
            if cand.size_bytes < min_bytes {
                return false;
            }
        }
        if filter_max_size_gb > 0.0 {
            let max_bytes = (filter_max_size_gb * 1024.0 * 1024.0 * 1024.0) as u64;
            if cand.size_bytes > max_bytes {
                return false;
            }
        }
        
        // Age filters
        if let Some(age) = cand.age_days() {
            let age = age as u32;
            if filter_min_age_days > 0 && age < filter_min_age_days {
                return false;
            }
            if filter_max_age_days > 0 && age > filter_max_age_days {
                return false;
            }
        }
        
        true
    }

    /// Get color for score visualization
    fn score_color(score: f32) -> egui::Color32 {
        if score >= 0.7 {
            egui::Color32::from_rgb(220, 50, 50)
        } else if score >= 0.5 {
            egui::Color32::from_rgb(220, 180, 50)
        } else {
            egui::Color32::GRAY
        }
    }
}

impl eframe::App for ReclaimApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Configure modern visual style
        let mut style = (*ctx.style()).clone();
        style.visuals.window_rounding = 10.0.into();
        style.visuals.window_shadow = egui::epaint::Shadow {
            offset: egui::vec2(0.0, 4.0),
            blur: 16.0,
            spread: 0.0,
            color: egui::Color32::from_black_alpha(100),
        };
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.visuals.widgets.noninteractive.rounding = 6.0.into();
        style.visuals.widgets.inactive.rounding = 6.0.into();
        style.visuals.widgets.hovered.rounding = 6.0.into();
        style.visuals.widgets.active.rounding = 6.0.into();
        
        // Modern color palette
        style.visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(50, 50, 60);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(60, 60, 70);
        style.visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(70, 70, 80);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(80, 80, 90);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(100, 150, 200);
        ctx.set_style(style);
        
        // Check permissions on startup
        if !self.permission_check_done {
            self.check_and_prompt_permissions();
        }
        
        // Auto-start scan on first update
        if self.scan_status == ScanStatus::NotStarted {
            self.start_auto_scan();
        }
        
        // Start verification thread on first update if we have unverified candidates
        if self.verification_receiver.is_none() && !self.verification_in_progress {
            let unverified_count = self.candidates.iter()
                .filter(|c| c.cache_status == reclaim_core::selection::CacheStatus::CachedUnverified)
                .count();
            
            if unverified_count > 0 {
                self.start_verification_thread(ctx);
            }
        }
        
        // Start discovery thread after verification if enabled
        if self.verification_receiver.is_none() 
            && !self.verification_in_progress
            && self.discovery_receiver.is_none()
            && !self.discovery_in_progress
            && !self.candidates.is_empty()
        {
            // Only discover once per session or after manual trigger
            if self.last_discovery.is_none() {
                self.start_discovery_thread(ctx);
            }
        }
        
        // Poll verification messages
        let mut verification_complete = false;
        let mut verification_error = None;
        let mut verified_candidates = None;
        let mut verification_stats = None;
        
        if let Some(rx) = &self.verification_receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    VerificationMessage::Progress { current, total, path: _ } => {
                        self.status_message = format!("Verifying cache... {}/{}", current, total);
                        ctx.request_repaint();
                    }
                    VerificationMessage::Complete { verified, stats } => {
                        verified_candidates = Some(verified);
                        verification_stats = Some(stats);
                        verification_complete = true;
                    }
                    VerificationMessage::Error(e) => {
                        verification_error = Some(e);
                    }
                }
            }
        }
        
        // Apply verification results after releasing borrow
        if verification_complete {
            if let (Some(candidates), Some(stats)) = (verified_candidates, verification_stats) {
                self.candidates = candidates;
                self.status_message = format!(
                    "✓ Verified: {} unchanged, {} changed, {} unavailable",
                    stats.verified_unchanged,
                    stats.changed,
                    stats.unavailable
                );
            }
            self.verification_receiver = None;
            self.verification_in_progress = false;
            ctx.request_repaint();
        }
        if let Some(e) = verification_error {
            self.status_message = format!("Verification error: {}", e);
            self.verification_receiver = None;
            self.verification_in_progress = false;
        }
        
        // Poll discovery messages
        let mut discovery_complete = false;
        let mut discovery_error = None;
        let mut new_discovered = None;
        let mut discovery_stats = None;
        
        if let Some(rx) = &self.discovery_receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    DiscoveryMessage::Progress { current, paths_scanned } => {
                        self.status_message = format!("Discovering... {} paths scanned", paths_scanned);
                        ctx.request_repaint();
                    }
                    DiscoveryMessage::Complete { new_candidates: new_cands, stats } => {
                        new_discovered = Some(new_cands);
                        discovery_stats = Some(stats);
                        discovery_complete = true;
                    }
                    DiscoveryMessage::Error(e) => {
                        discovery_error = Some(e);
                    }
                }
            }
        }
        
        // Apply discovery results after releasing borrow
        if discovery_complete {
            if let (Some(new_cands), Some(stats)) = (new_discovered, discovery_stats) {
                if stats.new_items > 0 {
                    // Merge new candidates into existing list
                    self.candidates.extend(new_cands);
                    Self::sort_by_mode(&mut self.candidates, self.sort_by);
                }
                self.status_message = format!(
                    "✓ Discovery complete: {} new items, {} paths scanned",
                    stats.new_items,
                    stats.paths_scanned
                );
            }
            self.discovery_receiver = None;
            self.discovery_in_progress = false;
            self.last_discovery = Some(Utc::now());
            ctx.request_repaint();
        }
        if let Some(e) = discovery_error {
            self.status_message = format!("Discovery error: {}", e);
            self.discovery_receiver = None;
            self.discovery_in_progress = false;
        }
        
        // Poll scan messages
        let mut scan_complete = false;
        let mut scan_error = None;
        let mut new_candidates = None;
        let mut new_profile = None;
        let mut new_stats = None;
        let mut cache_loaded = None;
        let mut quick_verified = None;

        if let Some(rx) = &self.scan_receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    ScanMessage::PhaseChange { phase, started_at } => {
                        self.scan_status = ScanStatus::Scanning { phase: phase.clone() };
                        self.scan_progress.phase_started_at = started_at;
                        self.scan_progress.phase_progress = 0.0;
                        
                        let phase_msg = format!("{} {}", phase.icon(), phase.description());
                        self.status_message = phase_msg;
                        ctx.request_repaint();
                    }
                    ScanMessage::CacheLoaded { candidates } => {
                        // Store for processing after loop
                        cache_loaded = Some(candidates);
                        ctx.request_repaint();
                    }
                    ScanMessage::QuickVerified { updated } => {
                        // Store for processing after loop
                        quick_verified = Some(updated);
                        ctx.request_repaint();
                    }
                    ScanMessage::Progress { current, total, done, phase_progress } => {
                        self.scan_progress.current_path = current;
                        self.scan_progress.total_roots = total;
                        self.scan_progress.roots_done = done;
                        self.scan_progress.phase_progress = phase_progress;
                        ctx.request_repaint();
                    }
                    ScanMessage::Complete { candidates, stats, profile } => {
                        new_candidates = Some(candidates);
                        new_profile = Some(profile);
                        new_stats = Some(stats);
                        scan_complete = true;
                    }
                    ScanMessage::Error(e) => {
                        scan_error = Some(e);
                    }
                }
            }
        }

        // Apply cache loaded results immediately (instant display)
        if let Some(mut cached_cands) = cache_loaded {
            Self::sort_by_mode(&mut cached_cands, self.sort_by);
            self.candidates = cached_cands;
            self.extract_filter_values();
            self.refresh_groups();
            self.status_message = format!("💾 Loaded {} items from cache (verifying...)", 
                self.candidates.len());
        }

        // Apply quick verification results
        if let Some(verified_cands) = quick_verified {
            // Update existing candidates with verified data
            for verified in verified_cands {
                if let Some(existing) = self.candidates.iter_mut()
                    .find(|c| c.candidate.path == verified.candidate.path) {
                    *existing = verified;
                }
            }
            Self::sort_by_mode(&mut self.candidates, self.sort_by);
            ctx.request_repaint();
        }

        // Apply scan results after releasing borrow
        if scan_complete {
            if let (Some(mut new_cands), Some(stats)) = (new_candidates, new_stats) {
                Self::sort_by_mode(&mut new_cands, self.sort_by);
                
                // ALWAYS merge results - never replace completely (preserve UI interaction)
                if !self.candidates.is_empty() {
                    // Rescan: merge new items, update changed ones
                    use std::collections::HashMap;
                    let mut existing: HashMap<_, _> = self.candidates.drain(..)
                        .map(|c| (c.candidate.path.clone(), c))
                        .collect();
                    
                    for new_c in new_cands {
                        // Update or insert
                        existing.insert(new_c.candidate.path.clone(), new_c);
                    }
                    
                    self.candidates = existing.into_values().collect();
                    Self::sort_by_mode(&mut self.candidates, self.sort_by);
                    
                    self.scan_new_count = stats.new_items;
                    self.status_message = format!(
                        "✓ Scan complete: {} total ({} new, {} changed)",
                        self.candidates.len(), stats.new_items, stats.changed_items
                    );
                } else {
                    // First scan
                    self.candidates = new_cands;
                    self.scan_new_count = 0;
                    self.status_message = format!("✓ Found {} candidates", self.candidates.len());
                }
                
                self.profile = new_profile;
                self.extract_filter_values();
                self.refresh_groups();
                self.scan_status = ScanStatus::Idle;
            }
            self.scan_receiver = None;
        }
        if let Some(e) = scan_error {
            self.scan_status = ScanStatus::Error(e);
            self.scan_receiver = None;
        }

        // Poll disk analysis messages
        if let Some(rx) = &self.disk_analysis_receiver {
            if let Ok(msg) = rx.try_recv() {
                match msg {
                    DiskAnalysisMessage::Complete(stats) => {
                        self.disk_stats = Some(stats);
                        self.status_message = "✓ Disk analysis complete".to_string();
                        self.disk_analysis_receiver = None;
                        ctx.request_repaint();
                    }
                    DiskAnalysisMessage::Error(e) => {
                        self.status_message = format!("Disk analysis error: {}", e);
                        self.disk_analysis_receiver = None;
                    }
                }
            }
        }

        egui::TopBottomPanel::top("top_panel")
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(18, 18, 22))
                .inner_margin(egui::Margin::symmetric(20.0, 16.0))
                .shadow(egui::epaint::Shadow {
                    offset: egui::vec2(0.0, 2.0),
                    blur: 8.0,
                    spread: 0.0,
                    color: egui::Color32::from_black_alpha(80),
                }))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // App title with gradient effect
                ui.heading(egui::RichText::new("🔍 Reclaim")
                    .size(26.0)
                    .strong()
                    .color(egui::Color32::from_rgb(100, 200, 255)));
                ui.add_space(20.0);
                
                ui.add_space(20.0);
                ui.separator();
                ui.add_space(15.0);
                
                let selected_sz = reclaim_core::candidate::human_bytes(self.selected_size());
                // Selected size badge with vibrant color
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(45, 45, 55))
                    .inner_margin(egui::Margin::symmetric(12.0, 6.0))
                    .rounding(6.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(format!("📦 Selected: {}", selected_sz))
                            .size(14.0)
                            .strong()
                            .color(egui::Color32::from_rgb(120, 220, 255)));
                    });
                
                // Settings button with modern styling
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let settings_btn = egui::Button::new(egui::RichText::new("⚙").size(18.0))
                        .fill(egui::Color32::from_rgb(45, 45, 55))
                        .rounding(6.0);
                    if ui.add(settings_btn)
                        .on_hover_text("Settings")
                        .clicked() {
                        self.show_settings = !self.show_settings;
                    }
                });
            });
        });

        // Update notification banner
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_scanning = matches!(self.scan_status, ScanStatus::Scanning { .. });
                    let can_apply = !is_scanning && self.candidates.iter().any(|c| c.is_selected());
                    
                    ui.add_enabled_ui(can_apply, |ui| {
                        if ui.button("Apply Cleanup").clicked() {
                            let n = self.candidates.iter().filter(|c| c.is_selected()).count();
                            let sz = self.selected_size();
                            self.operation_state = OperationState::ApplyConfirm {
                                items: n,
                                total_size: sz,
                            };
                        }
                    });
                    
                    if !can_apply && is_scanning {
                        ui.label("⏳ Scan in progress...");
                    }
                    
                    if ui.button("Dry-run").clicked() {
                        let n = self.candidates.iter().filter(|c| c.is_selected()).count();
                        let sz = self.selected_size();
                        self.operation_state = OperationState::DryRun {
                            items: n,
                            total_size: sz,
                        };
                    }
                });
            });
        });

        egui::SidePanel::left("controls")
            .min_width(320.0)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(20, 20, 25))
                .inner_margin(egui::Margin::same(20.0)))
            .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Modern card-based layout
                ui.add_space(5.0);
                ui.heading(egui::RichText::new("🎮 Controls")
                    .size(20.0)
                    .strong()
                    .color(egui::Color32::from_rgb(120, 220, 255)));
                ui.add_space(15.0);

                // Sort control in a card
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(30, 30, 38))
                    .inner_margin(egui::Margin::same(16.0))
                    .rounding(8.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("🔢 Sort by:").size(15.0).strong());
                        ui.add_space(10.0);
                        ui.radio_value(&mut self.sort_by, SortBy::Score, "Score");
                        ui.radio_value(&mut self.sort_by, SortBy::Size, "Size");
                        ui.radio_value(&mut self.sort_by, SortBy::Age, "Age");
                        ui.radio_value(&mut self.sort_by, SortBy::Kind, "Kind");
                    });
                
                ui.add_space(16.0);
                
                // Group control in a card
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(30, 30, 38))
                    .inner_margin(egui::Margin::same(16.0))
                    .rounding(8.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("📁 Group by:").size(15.0).strong());
                        ui.add_space(10.0);
                        ui.radio_value(&mut self.group_by, GroupBy::None, "None");
                        ui.radio_value(&mut self.group_by, GroupBy::Kind, "Kind");
                        ui.radio_value(&mut self.group_by, GroupBy::Path, "Path/Group");
                    });

                ui.add_space(10.0);

                ui.separator();
                ui.heading("Categorical Filters");
                
                // Kind filter
                ui.collapsing("Target Kind", |ui| {
                    let all_enabled = self.available_kinds.iter()
                        .all(|k| self.enabled_kinds.contains(k));
                    let mut select_all = all_enabled;
                    
                    if ui.checkbox(&mut select_all, "Select All").changed() {
                        if select_all {
                            self.enabled_kinds = self.available_kinds.iter().cloned().collect();
                        } else {
                            self.enabled_kinds.clear();
                        }
                    }
                    
                    ui.separator();
                    for kind in &self.available_kinds {
                        let mut enabled = self.enabled_kinds.contains(kind);
                        if ui.checkbox(&mut enabled, kind).changed() {
                            if enabled {
                                self.enabled_kinds.insert(kind.clone());
                            } else {
                                self.enabled_kinds.remove(kind);
                            }
                        }
                    }
                });
                
                // Action filter
                ui.collapsing("Action", |ui| {
                    let all_enabled = self.available_actions.iter()
                        .all(|a| self.enabled_actions.contains(a));
                    let mut select_all = all_enabled;
                    
                    if ui.checkbox(&mut select_all, "Select All").changed() {
                        if select_all {
                            self.enabled_actions = self.available_actions.iter().cloned().collect();
                        } else {
                            self.enabled_actions.clear();
                        }
                    }
                    
                    ui.separator();
                    for action in &self.available_actions {
                        let mut enabled = self.enabled_actions.contains(action);
                        if ui.checkbox(&mut enabled, action).changed() {
                            if enabled {
                                self.enabled_actions.insert(action.clone());
                            } else {
                                self.enabled_actions.remove(action);
                            }
                        }
                    }
                });
                
                // Group filter
                if !self.available_groups.is_empty() {
                    ui.collapsing("Group/Project", |ui| {
                        let all_enabled = self.available_groups.iter()
                            .all(|g| self.enabled_groups.contains(g));
                        let mut select_all = all_enabled;
                        
                        if ui.checkbox(&mut select_all, "Select All").changed() {
                            if select_all {
                                self.enabled_groups = self.available_groups.iter().cloned().collect();
                            } else {
                                self.enabled_groups.clear();
                            }
                        }
                        
                        ui.separator();
                        for group in &self.available_groups {
                            let mut enabled = self.enabled_groups.contains(group);
                            if ui.checkbox(&mut enabled, group).changed() {
                                if enabled {
                                    self.enabled_groups.insert(group.clone());
                                } else {
                                    self.enabled_groups.remove(group);
                                }
                            }
                        }
                    });
                }

                ui.add_space(10.0);
                ui.separator();
                ui.heading("Range Filters");
                
                ui.horizontal(|ui| {
                    ui.label("Min score:");
                    ui.add(egui::Slider::new(&mut self.filter_min_score, 0.0..=1.0).step_by(0.1));
                });

                ui.horizontal(|ui| {
                    ui.label("Min size (MB):");
                    ui.add(egui::Slider::new(&mut self.filter_min_size_mb, 0.0..=1000.0).step_by(10.0));
                });

                ui.horizontal(|ui| {
                    ui.label("Max size (GB):");
                    ui.add(egui::Slider::new(&mut self.filter_max_size_gb, 0.0..=100.0).step_by(1.0));
                });

                ui.horizontal(|ui| {
                    ui.label("Min age (days):");
                    ui.add(egui::Slider::new(&mut self.filter_min_age_days, 0..=365).step_by(1.0));
                });

                ui.horizontal(|ui| {
                    ui.label("Max age (days):");
                    ui.add(egui::Slider::new(&mut self.filter_max_age_days, 0..=730).step_by(1.0));
                });

                ui.checkbox(&mut self.show_only_selected, "Show only selected");

                if ui.button("Clear All Filters").clicked() {
                    self.filter_min_size_mb = 0.0;
                    self.filter_max_size_gb = 0.0;
                    self.filter_min_age_days = 0;
                    self.filter_max_age_days = 0;
                    self.filter_min_score = 0.0;
                    self.show_only_selected = false;
                    self.enabled_kinds = self.available_kinds.iter().cloned().collect();
                    self.enabled_actions = self.available_actions.iter().cloned().collect();
                    self.enabled_groups = self.available_groups.iter().cloned().collect();
                }

                ui.add_space(10.0);
                ui.separator();
                
                if !self.candidates.is_empty() {
                    // Count filtered candidates
                    let filtered_count = self.candidates.iter().filter(|c| {
                        self.enabled_kinds.contains(c.candidate.kind.label()) &&
                        self.enabled_actions.contains(&c.candidate.action.display()) &&
                        (c.candidate.group.as_ref().map(|g| self.enabled_groups.contains(g)).unwrap_or(true)) &&
                        c.candidate.score >= self.filter_min_score &&
                        (!self.show_only_selected || c.is_selected())
                    }).count();
                    
                    ui.label(format!("Showing: {} / {} candidates", filtered_count, self.candidates.len()));
                    
                    let inner_candidates: Vec<Candidate> = self.candidates.iter().map(|c| c.candidate.clone()).collect();
                    let report = Report::build(&inner_candidates);
                    ui.label(format!("Total size: {}", 
                        reclaim_core::candidate::human_bytes(report.total_size_bytes)));
                    ui.label(format!("Selected: {}", 
                        reclaim_core::candidate::human_bytes(report.selected_size_bytes)));
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // Tab bar for view modes
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.view_mode, ViewMode::Table, "📊 Table");
                ui.selectable_value(&mut self.view_mode, ViewMode::Treemap, "🗺 Treemap");
                ui.separator();
                ui.selectable_value(&mut self.view_mode, ViewMode::DiskOverview, "💽 Disk Overview");
                
                // Add grouping toggle
                ui.separator();
                let group_icon = if self.show_groups { "📦" } else { "📄" };
                let group_label = if self.show_groups { "Grouped" } else { "Flat" };
                if ui.button(format!("{} {}", group_icon, group_label))
                    .on_hover_text("Toggle grouped/flat view")
                    .clicked() {
                    self.show_groups = !self.show_groups;
                }
            });
            ui.separator();
            
            // Show appropriate state when no candidates
            if self.candidates.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        
                        match &self.scan_status {
                            ScanStatus::NotStarted => {
                                ui.heading("🔍 Initializing...");
                                ui.add_space(10.0);
                                ui.label("Starting automatic scan");
                            }
                            ScanStatus::Scanning { phase } => {
                                ui.spinner();
                                ui.add_space(10.0);
                                ui.heading(format!("{} {}", phase.icon(), match phase {
                                    ScanPhase::CacheLoad => "Loading cache...",
                                    ScanPhase::QuickVerify => "Verifying items...",
                                    ScanPhase::FullScan => "Scanning disk...",
                                    ScanPhase::DiskAnalysis => "Analyzing disk...",
                                }));
                                
                                ui.add_space(10.0);
                                ui.label(phase.description());
                                ui.add_space(20.0);
                                
                                // Progress bar
                                if self.scan_progress.phase_progress > 0.0 {
                                    let progress = self.scan_progress.phase_progress;
                                    let progress_text = if self.scan_progress.total_roots > 0 {
                                        format!("{} / {} roots ({:.0}%)", 
                                            self.scan_progress.roots_done,
                                            self.scan_progress.total_roots,
                                            progress * 100.0)
                                    } else {
                                        format!("{:.0}%", progress * 100.0)
                                    };
                                    
                                    ui.add(
                                        egui::ProgressBar::new(progress)
                                            .text(progress_text)
                                    );
                                    ui.add_space(10.0);
                                }
                                
                                // Timing information
                                let elapsed = self.scan_progress.elapsed();
                                ui.label(format!("⏱ Elapsed: {}s", elapsed.num_seconds()));
                                
                                if let Some(remaining) = self.scan_progress.estimated_remaining(phase) {
                                    if remaining.num_seconds() > 0 {
                                        ui.label(format!("⏳ Remaining: ~{}s", remaining.num_seconds()));
                                    }
                                }
                                
                                if !self.scan_progress.current_path.is_empty() {
                                    ui.add_space(10.0);
                                    ui.label(format!("📁 Scanning: {}", self.scan_progress.current_path));
                                }
                            }
                            ScanStatus::Paused => {
                                ui.heading("⏸ Scan paused");
                                ui.add_space(10.0);
                                ui.label("No items found yet");
                                ui.add_space(20.0);
                                if ui.button("▶ Resume").clicked() {
                                    self.resume_scan();
                                }
                            }
                            ScanStatus::Idle => {
                                ui.heading("✓ Scan complete");
                                ui.add_space(10.0);
                                ui.label("No reclaimable items found");
                                ui.add_space(20.0);
                                if ui.button("🔄 Refresh Scan").clicked() {
                                    self.scan();
                                }
                            }
                            ScanStatus::Error(e) => {
                                ui.colored_label(egui::Color32::RED, "❌ Scan error");
                                ui.add_space(10.0);
                                ui.label(e.clone());
                                ui.add_space(20.0);
                                if ui.button("🔄 Retry").clicked() {
                                    self.scan();
                                }
                            }
                        }
                    });
                });
                return;
            }

            let mut scroll = egui::ScrollArea::vertical()
                .auto_shrink([false, false]);
            
            if self.scroll_to_top {
                scroll = scroll.vertical_scroll_offset(0.0);
                self.scroll_to_top = false;
            }

            scroll.show(ui, |ui| {
                match self.view_mode {
                    ViewMode::Table => self.render_table_view(ui),
                    ViewMode::Treemap => self.render_treemap_view(ui),
                    ViewMode::DiskOverview => self.render_disk_overview(ui),
                }
            });
        });

        // Operation modals
        let operation_state = self.operation_state.clone();
        match operation_state {
            OperationState::DryRun { items, total_size } => {
                egui::Window::new("Dry-run Preview")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.heading("📊 Cleanup Preview");
                        ui.add_space(10.0);
                        
                        ui.label(format!("Items to clean: {items}"));
                        ui.label(format!("Space to reclaim: {}", 
                            reclaim_core::candidate::human_bytes(total_size)));
                        
                        ui.add_space(10.0);
                        ui.separator();
                        ui.label("⚠️  No files will be modified in dry-run mode.");
                        
                        ui.add_space(10.0);
                        if ui.button("Close").clicked() {
                            self.operation_state = OperationState::Idle;
                        }
                    });
            }
            OperationState::ApplyConfirm { items, total_size } => {
                egui::Window::new("Confirm Cleanup")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.heading("⚠️  Confirm Cleanup");
                        ui.add_space(10.0);
                        
                        ui.label(format!("Clean {items} items?"));
                        ui.label(format!("Reclaim: {}", 
                            reclaim_core::candidate::human_bytes(total_size)));
                        
                        ui.add_space(10.0);
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 50, 50),
                            "⚠️  This action cannot be undone!"
                        );
                        
                        ui.add_space(15.0);
                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                self.operation_state = OperationState::Idle;
                            }
                            ui.add_space(10.0);
                            if ui.button("Apply").clicked() {
                                self.operation_state = OperationState::Applying {
                                    total: items,
                                };
                                ctx.request_repaint(); // Force immediate repaint
                            }
                        });
                    });
            }
            OperationState::Applying { total } => {
                egui::Window::new("Cleanup in Progress")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.heading("🔄 Cleaning up...");
                        ui.add_space(10.0);
                        
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(format!("Processing {} items...", total));
                        });
                        
                        ui.add_space(10.0);
                        ui.label("Please wait...");
                        
                        // Actually perform cleanup (blocks UI but shows spinner first)
                        let (freed, errors) = self.apply_cleanup();
                        
                        if errors.is_empty() {
                            self.status_message = format!("✓ Freed {}", 
                                reclaim_core::candidate::human_bytes(freed));
                        } else {
                            self.status_message = format!("Freed {} ({} errors)", 
                                reclaim_core::candidate::human_bytes(freed),
                                errors.len());
                        }
                        self.operation_state = OperationState::Idle;
                    });
            }
            OperationState::Idle => {}
        }
        
        // Permission dialog
        if self.show_permission_dialog {
            egui::Window::new("🔒 Full Disk Access Required")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.heading("Full Disk Access Required");
                    ui.add_space(10.0);
                    
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 150, 0),
                        "⚠️  Important: Without Full Disk Access, macOS will show many permission popups!"
                    );
                    ui.add_space(10.0);
                    
                    ui.label("Reclaim needs Full Disk Access to scan protected directories:");
                    ui.add_space(5.0);
                    ui.label("  • Downloads, Documents, Desktop");
                    ui.label("  • Browser caches (Safari, Chrome, Firefox)");
                    ui.label("  • System caches and logs");
                    ui.add_space(10.0);
                    
                    ui.colored_label(
                        egui::Color32::from_rgb(120, 220, 255),
                        "💡 This prevents macOS from showing 20+ permission dialogs!"
                    );
                    
                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(10.0);
                    
                    ui.label(egui::RichText::new("To grant Full Disk Access:").strong());
                    ui.add_space(5.0);
                    ui.label("1. Click 'Open System Settings' below");
                    ui.label("2. Click the 🔒 lock and authenticate");
                    ui.label("3. Find and enable 'Reclaim' in the list");
                    ui.label("   (or click + to add /Applications/Reclaim.app)");
                    ui.add_space(5.0);
                    ui.label(egui::RichText::new("4. Click 'Start Scan' below (no restart needed!)").strong().color(egui::Color32::from_rgb(120, 220, 255)));
                    
                    ui.add_space(15.0);
                    ui.horizontal(|ui| {
                        if ui.button("Open System Settings").clicked() {
                            Self::open_system_settings_permissions();
                        }
                        ui.add_space(10.0);
                        if ui.button("Start Scan Anyway").clicked() {
                            self.show_permission_dialog = false;
                            self.auto_scan_enabled = true;
                            self.scan();
                        }
                        ui.add_space(10.0);
                        if ui.button("Ignore").clicked() {
                            self.show_permission_dialog = false;
                        }
                    });
                    
                    ui.add_space(10.0);
                    ui.colored_label(
                        egui::Color32::from_rgb(150, 150, 160),
                        "💡 Tip: After granting access, click 'Start Scan Anyway' - no restart needed!"
                    );
                    ui.add_space(5.0);
                    if ui.checkbox(&mut self.dont_ask_permissions, "Don't ask again at startup").changed() {
                        self.settings.dont_ask_permissions_at_startup = self.dont_ask_permissions;
                        let _ = self.settings.save();
                    }
                });
        }
        
        // Settings dialog
        if self.show_settings {
            egui::Window::new("⚙ Settings")
                .collapsible(false)
                .resizable(false)
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.heading("Settings");
                    ui.add_space(10.0);
                    
                    ui.separator();
                    ui.label("Permissions");
                    ui.add_space(5.0);
                    
                    let has_access = Self::check_full_disk_access();
                    let status_text = if has_access {
                        "✅ Full Disk Access: Granted"
                    } else {
                        "⚠️  Full Disk Access: Not Granted"
                    };
                    let color = if has_access {
                        egui::Color32::from_rgb(76, 175, 80)
                    } else {
                        egui::Color32::from_rgb(255, 150, 0)
                    };
                    
                    ui.colored_label(color, status_text);
                    ui.add_space(5.0);
                    
                    if !has_access {
                        if ui.button("Grant Full Disk Access...").clicked() {
                            Self::open_system_settings_permissions();
                        }
                    }
                    
                    ui.add_space(10.0);
                    if ui.checkbox(&mut self.dont_ask_permissions, "Don't ask for permissions at startup").changed() {
                        self.settings.dont_ask_permissions_at_startup = self.dont_ask_permissions;
                        let _ = self.settings.save();
                    }
                    
                    ui.add_space(15.0);
                    ui.separator();
                    ui.label("View");
                    ui.add_space(5.0);
                    
                    if ui.checkbox(&mut self.show_groups, "Show groups by default").changed() {
                        self.settings.show_groups_by_default = self.show_groups;
                        let _ = self.settings.save();
                    }
                    
                    ui.add_space(15.0);
                    ui.separator();
                    ui.label("Updates");
                    ui.add_space(5.0);
                    
                    // Show install method and update availability
                    match self.install_method {
                        updater::InstallMethod::Standalone => {
                            ui.label("📦 Standalone installation");
                            ui.add_space(3.0);
                            ui.label(egui::RichText::new("Auto-update: Enabled")
                                .color(egui::Color32::from_rgb(76, 175, 80)));
                            ui.add_space(5.0);
                            
                            if let Some(ref version) = self.update_available {
                                ui.colored_label(
                                    egui::Color32::from_rgb(120, 220, 255),
                                    format!("🔄 Update available: {}", version)
                                );
                                if ui.button("Download & Install").clicked() {
                                    // TODO: Implement actual update download
                                    self.status_message = format!("Update to {} would be downloaded", version);
                                }
                            } else if self.checking_update {
                                ui.label("⏳ Checking for updates...");
                            } else {
                                ui.label("✓ You're up to date");
                                if ui.button("Check for Updates").clicked() {
                                    // TODO: Implement update check
                                    self.checking_update = true;
                                    self.status_message = "Checking for updates...".to_string();
                                }
                            }
                        }
                        updater::InstallMethod::SystemPackage => {
                            ui.label("📦 System package installation");
                            ui.add_space(3.0);
                            ui.label(egui::RichText::new("Auto-update: Disabled")
                                .color(egui::Color32::from_rgb(180, 180, 190)));
                            ui.add_space(5.0);
                            
                            ui.label("Update via your package manager:");
                            ui.add_space(3.0);
                            
                            #[cfg(target_os = "macos")]
                            {
                                ui.code("brew upgrade reclaim");
                            }
                            
                            #[cfg(target_os = "linux")]
                            {
                                ui.code("sudo apt update && sudo apt upgrade reclaim");
                            }
                            
                            #[cfg(target_os = "windows")]
                            {
                                ui.code("winget upgrade reclaim");
                            }
                        }
                    }
                    
                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(10.0);
                    
                    if ui.button("Close").clicked() {
                        self.show_settings = false;
                    }
                });
        }
        
        // Genealogy exploration window
        if self.show_genealogy_window {
            if let Some(ref context) = self.genealogy_context {
                // Clone context to avoid borrow issues
                let context_clone = context.clone();
                
                egui::Window::new("🗂 Folder Genealogy")
                    .collapsible(false)
                    .resizable(true)
                    .default_width(600.0)
                    .show(ctx, |ui| {
                        ui.heading("Folder Hierarchy");
                        ui.add_space(10.0);
                        
                        // Current path
                        ui.label(egui::RichText::new("Current Location:").strong());
                        ui.label(egui::RichText::new(context_clone.path.display().to_string())
                            .color(egui::Color32::from_rgb(150, 200, 255)));
                        ui.add_space(10.0);
                        
                        // Parent folder
                        if let Some(ref parent) = context_clone.parent {
                            ui.separator();
                            ui.add_space(5.0);
                            ui.label(egui::RichText::new("📁 Parent Folder:").strong());
                            ui.label(parent.display().to_string());
                            ui.add_space(5.0);
                        }
                        
                        // Siblings in parent folder
                        if !context_clone.siblings.is_empty() {
                            ui.separator();
                            ui.add_space(5.0);
                            ui.label(egui::RichText::new(format!("👥 {} Siblings:", context_clone.sibling_count)).strong());
                            ui.add_space(5.0);
                            
                            egui::ScrollArea::vertical()
                                .max_height(200.0)
                                .show(ui, |ui| {
                                    for sibling in &context_clone.siblings {
                                        let name = sibling.file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy();
                                        ui.label(egui::RichText::new(format!("  • {}", name))
                                            .color(egui::Color32::from_rgb(180, 180, 190)));
                                    }
                                });
                        }
                        
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(5.0);
                        
                        // Total size in parent
                        ui.label(egui::RichText::new(format!("📊 Total size in parent: {}", 
                            reclaim_core::candidate::human_bytes(context_clone.total_size_in_parent)))
                            .strong()
                            .color(egui::Color32::from_rgb(100, 200, 255)));
                        
                        ui.add_space(10.0);
                        
                        // Expand up button - regroup at parent level
                        if context_clone.parent.is_some() {
                            if ui.button("⬆ Expand to Parent Level")
                                .on_hover_text("Regroup all items at parent folder level")
                                .clicked() 
                            {
                                self.show_genealogy_window = false;
                                self.refresh_groups();
                            }
                        }
                        
                        ui.add_space(15.0);
                    });
                
                // Add close button in a separate section to avoid borrow issues
                // The window can also be closed by clicking X
            }
            
            // Check if we should close the genealogy window (e.g., user pressed ESC)
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.show_genealogy_window {
                self.show_genealogy_window = false;
                self.genealogy_context = None;
            }
        }
    }
}
