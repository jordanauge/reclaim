# Semantic Taxonomy - Disk Organization & Navigation

## Vision

Transform file system chaos into a navigable **semantic hierarchy** with ~100 meta-groups maximum, enabling:
- Quick overview of disk usage by category
- Identification of optimization opportunities
- User annotations (backup priority, confidentiality, etc.)
- Smart suggestions (move, backup, cleanup)

## Core Architecture: DAG of Meta-Groups

### Top-Level Categories (L0)

```
Disk Root
├─ 📦 Code        (repos, projects, workspaces)
├─ 📄 Documents   (personal, work, archives)
├─ 🎬 Media       (photos, videos, music)
├─ 💾 System      (OS, apps, libraries)
├─ 🗂️  Caches     (temp, build, derived)
└─ 🔧 Tools       (binaries, SDKs, utilities)
```

### Example Expansion: Code Category (L1 → L2 → L3)

```
📦 Code (250 GB)
├─ Work Projects (120 GB)
│  ├─ cisco-eti/mas-framework (45 GB)
│  │  ├─ .venv/ (12 GB) [reproducible, unused 30d] 🟡
│  │  ├─ target/ (8 GB) [build, unused 7d] 🔴
│  │  └─ source code (25 GB) [critical, backup]
│  └─ cisco-eti/mas-lab (35 GB)
│     ├─ .venv/ (10 GB) [active]
│     └─ experiments/ (15 GB) [results, backup]
├─ Personal Projects (80 GB)
│  ├─ reclaim/ (2 GB) [active]
│  └─ old-experiments/ (78 GB) [unused 6m] 🟠
└─ Clones & Forks (50 GB)
   └─ [mostly unused, low priority]
```

## Classification Rules

### 1. Pattern-Based Recognition

```rust
pub struct ClassificationRule {
    pattern: PathPattern,
    category: Category,
    priority: u8,
    confidence: f32,
}

// Example rules
let rules = vec![
    Rule {
        pattern: "**/.venv/*",
        category: Category::Code(CodeType::VirtualEnv),
        parent_context: Some(Category::Code(CodeType::Repository)),
        confidence: 0.95,
    },
    Rule {
        pattern: "**/target/debug/**",
        category: Category::Caches(CacheType::Build),
        parent_context: Some(Category::Code(CodeType::RustProject)),
        confidence: 0.99,
    },
    Rule {
        pattern: "**/node_modules/**",
        category: Category::Caches(CacheType::PackageManager),
        parent_context: Some(Category::Code(CodeType::NodeProject)),
        confidence: 0.98,
    },
];
```

### 2. Contextual Hierarchy Detection

```rust
// .venv is part of a larger code structure
fn detect_context(path: &Path) -> Option<Context> {
    // Walk up to find indicators
    let mut current = path.parent()?;
    while let Some(parent) = current.parent() {
        if parent.join(".git").exists() {
            return Some(Context::Repository {
                root: parent,
                type: detect_repo_type(parent),
            });
        }
        if parent.join("Cargo.toml").exists() {
            return Some(Context::RustWorkspace { root: parent });
        }
        current = parent;
    }
    None
}
```

### 3. Content Inspection Plugins

```rust
pub trait ContentClassifier: Send + Sync {
    fn name(&self) -> &str;
    fn classify(&self, path: &Path) -> Result<Classification>;
    fn confidence(&self) -> f32;
}

// Example plugins
struct XcodeProjectClassifier;
impl ContentClassifier for XcodeProjectClassifier {
    fn classify(&self, path: &Path) -> Result<Classification> {
        if path.join("project.pbxproj").exists() {
            Ok(Classification {
                category: Category::Code(CodeType::XcodeProject),
                subcategories: vec![
                    detect_derived_data(path),
                    detect_archives(path),
                ],
            })
        }
    }
}

struct MediaLibraryClassifier;
impl ContentClassifier for MediaLibraryClassifier {
    fn classify(&self, path: &Path) -> Result<Classification> {
        let extensions = count_extensions(path)?;
        if extensions.media_ratio() > 0.8 {
            Ok(Classification {
                category: Category::Media(detect_media_type(&extensions)),
                attributes: Attributes {
                    backup_priority: High,
                    immutable: true,
                },
            })
        }
    }
}
```

## User Annotations & Inheritance

### Annotation Schema

```rust
pub struct Annotation {
    pub path: PathBuf,
    pub backup_priority: BackupPriority,  // Critical/High/Medium/Low/None
    pub confidentiality: Confidentiality, // Public/Internal/Confidential/Secret
    pub category_override: Option<Category>,
    pub tags: Vec<String>,
    pub notes: String,
}

pub enum BackupPriority {
    Critical,  // Must backup (personal files, work)
    High,      // Should backup (code, configs)
    Medium,    // Nice to have (media with copies)
    Low,       // Reproducible (caches, builds)
    None,      // Don't backup (system, temp)
}
```

### Inheritance Rules

```rust
impl MetaGroup {
    fn effective_annotation(&self) -> Annotation {
        let mut annotation = self.direct_annotation.clone();
        
        // Walk up hierarchy for inherited values
        let mut current = self.parent;
        while let Some(parent) = current {
            if annotation.backup_priority.is_none() {
                annotation.backup_priority = parent.annotation.backup_priority;
            }
            if annotation.confidentiality.is_none() {
                annotation.confidentiality = parent.annotation.confidentiality;
            }
            current = parent.parent;
        }
        
        annotation
    }
}
```

### Outlier Detection

```rust
// Find items that don't match group's dominant pattern
pub fn detect_outliers(group: &MetaGroup) -> Vec<Outlier> {
    let dominant_annotation = group.majority_annotation();
    
    group.children
        .iter()
        .filter(|child| {
            child.annotation.significantly_differs(&dominant_annotation)
        })
        .map(|child| Outlier {
            path: child.path.clone(),
            reason: OutlierReason::AnnotationMismatch,
            suggestion: format!(
                "Expected {}, found {}",
                dominant_annotation, child.annotation
            ),
        })
        .collect()
}
```

## Smart Grouping Heuristics

### Goal: Minimize Group Count

```rust
pub struct GroupingStrategy {
    pub name: String,
    pub rules: Vec<GroupingRule>,
    pub max_groups: usize,  // Target ~100
}

// Example: Group by project + type
let strategy = GroupingStrategy {
    name: "By Project Context".into(),
    rules: vec![
        GroupBy::Repository,      // Group all files by repo root
        ThenGroupBy::ArtifactType, // Within repo: venv, build, source
    ],
    max_groups: 100,
};

// Example: Group by recency
let strategy = GroupingStrategy {
    name: "By Last Access".into(),
    rules: vec![
        GroupBy::AccessTime(vec![
            "Active (< 7d)",
            "Recent (< 30d)",
            "Old (< 6m)",
            "Stale (> 6m)",
        ]),
        ThenGroupBy::Category,
    ],
    max_groups: 100,
};
```

### Dynamic Regrouping

```rust
impl MetaGroupView {
    // User selects group → right-click → "Regroup by..."
    pub fn regroup(&mut self, strategy: GroupingStrategy) {
        // Flatten current hierarchy
        let all_items: Vec<_> = self.flatten().collect();
        
        // Apply new strategy
        self.root = strategy.apply(&all_items);
        
        // Preserve user selections across regrouping
        for item in all_items {
            if let Some(old_state) = self.selection_cache.get(&item.path) {
                item.restore_selection(old_state);
            }
        }
    }
}
```

## Analysis & Suggestions

### Automatic Detection

```rust
pub struct DiskAnalysis {
    pub duplicates: Vec<DuplicateSet>,
    pub old_unused: Vec<UnusedItem>,
    pub large_files: Vec<LargeFile>,
    pub build_artifacts: Vec<BuildArtifact>,
    pub backup_candidates: Vec<BackupCandidate>,
    pub move_suggestions: Vec<MoveSuggestion>,
}

// Duplicate detection (content hash)
pub fn find_duplicates(root: &Path) -> Result<Vec<DuplicateSet>> {
    let mut hash_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    
    for entry in WalkDir::new(root).min_depth(1) {
        let path = entry?.path();
        if path.is_file() {
            let hash = content_hash(path)?;
            hash_map.entry(hash).or_default().push(path.to_path_buf());
        }
    }
    
    Ok(hash_map
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .map(|(hash, paths)| DuplicateSet { hash, paths })
        .collect())
}

// Unused detection (atime)
pub fn find_unused(groups: &[MetaGroup], threshold_days: u32) -> Vec<UnusedItem> {
    groups
        .iter()
        .filter(|g| g.last_accessed_days() > threshold_days)
        .map(|g| UnusedItem {
            path: g.root.clone(),
            size: g.total_size,
            last_access: g.last_accessed,
            suggestion: SuggestAction::Archive,
        })
        .collect()
}
```

### Move Suggestions

```rust
pub struct MoveSuggestion {
    pub current_path: PathBuf,
    pub suggested_path: PathBuf,
    pub reason: MoveReason,
    pub confidence: f32,
}

pub enum MoveReason {
    // .venv in ~/Desktop → move to project folder
    MisplacedArtifact { expected_parent: PathBuf },
    
    // Large media files in ~/Documents → move to ~/Media
    CategoryMismatch { expected_category: Category },
    
    // Old code in ~/repos → move to ~/Archives/code
    AgeBasedArchival { age_days: u32 },
}
```

## UI Design

### Hierarchical Tree with Expand/Collapse

```
📦 Code (250 GB) [50 items hidden] ▶
📄 Documents (80 GB) ▼
  └─ Work (60 GB) ▼
      ├─ reports-2026/ (15 GB) [backup:high] 🟢
      ├─ drafts/ (5 GB) [backup:medium]
      └─ archive-2025/ (40 GB) [unused 4m] 🟠 → Suggest: Move to Archive?
🎬 Media (500 GB) ▶
💾 System (120 GB) [read-only] ▶
🗂️  Caches (85 GB) ▼
  ├─ Build Artifacts (45 GB) 🔴 → Cleanup available
  └─ Package Managers (40 GB) 🔴 → Cleanup available
```

### Context Menus

```
Right-click on group:
┌─────────────────────────────────┐
│ Expand All                      │
│ Collapse All                    │
│ ─────────────────────────────── │
│ Regroup by... ▶                 │
│   ├─ Project                    │
│   ├─ File Type                  │
│   ├─ Last Modified              │
│   └─ Custom...                  │
│ ─────────────────────────────── │
│ Annotate... ▶                   │
│   ├─ Set Backup Priority        │
│   ├─ Set Confidentiality        │
│   ├─ Add Tags                   │
│   └─ Add Note                   │
│ ─────────────────────────────── │
│ Suggest Actions ▶               │
│   ├─ Find Duplicates            │
│   ├─ Find Unused (>6m)          │
│   ├─ Move to Archive            │
│   └─ Create Backup Plan         │
│ ─────────────────────────────── │
│ Show in Finder                  │
│ Properties...                   │
└─────────────────────────────────┘
```

### Modified Groups Banner

```
╔═══════════════════════════════════════════════════════════╗
║ 📢 Scan Update: 5 groups changed, 3 new groups detected  ║
║                                                           ║
║ Modified:                                                 ║
║   🟠 ~/repos/mas-lab (.venv grew +2.5 GB)               ║
║   🟠 ~/repos/reclaim (new target/ directory)             ║
║                                                           ║
║ New:                                                      ║
║   🔵 ~/Downloads/project-backup/ (15 GB)                 ║
║                                                           ║
║   [Review Changes]  [Update View]  [Dismiss]            ║
╚═══════════════════════════════════════════════════════════╝
```

## Implementation Roadmap

### Phase 1: Classification Engine (2-3 days)
- [ ] Category enum + subcategories
- [ ] Pattern-based rules
- [ ] Context detection (repo, workspace, etc.)
- [ ] Plugin architecture for content classifiers

### Phase 2: Annotation System (1-2 days)
- [ ] Annotation schema + SQLite storage
- [ ] Inheritance logic
- [ ] Outlier detection
- [ ] UI for annotation editing

### Phase 3: Grouping Strategies (2-3 days)
- [ ] Grouping rule DSL
- [ ] Dynamic regrouping
- [ ] Selection preservation
- [ ] Multiple simultaneous views

### Phase 4: Analysis & Suggestions (2-3 days)
- [ ] Duplicate finder (content hashing)
- [ ] Unused detector (atime)
- [ ] Large file finder
- [ ] Move suggestions engine

### Phase 5: UI Overhaul (3-4 days)
- [ ] Tree view with expand/collapse
- [ ] Context menus
- [ ] Drag-and-drop for moving
- [ ] Batch operations on groups
- [ ] Export/import annotation ground truth

## Example: Full Classification

```
Input: ~/repos/cisco-eti/mas-lab/

Output:
MetaGroup {
    path: ~/repos/cisco-eti/mas-lab/,
    category: Code(PythonRepository),
    context: Repository { remote: "git@github.com:cisco-eti/mas-lab" },
    size: 35 GB,
    annotation: Annotation {
        backup_priority: Critical,
        confidentiality: Internal,
        tags: ["work", "research", "active"],
    },
    children: [
        MetaGroup {
            path: .venv/,
            category: Caches(VirtualEnv),
            size: 10 GB,
            annotation: Annotation {
                backup_priority: Low,  // Inherited + overridden
            },
            analysis: Some(UnusedAnalysis {
                last_used: 2 days ago,
                reproducible: true,
            }),
        },
        MetaGroup {
            path: experiments/,
            category: Code(ExperimentResults),
            size: 15 GB,
            annotation: Annotation {
                backup_priority: High,
                tags: ["results", "paper-data"],
            },
        },
    ],
}
```

This creates a **navigable, annotated, smart hierarchy** instead of raw file lists.
