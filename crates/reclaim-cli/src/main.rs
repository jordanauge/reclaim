use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use reclaim_core::{
    candidate::{human_bytes, Action},
    profile::Profile,
    report::Report,
    scanner,
    strategy,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name    = "reclaim",
    version,
    about   = "Reclaim disk space by cleaning dev artifacts",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan directories and print a report of reclaimable space.
    Scan {
        /// Directories to scan.
        #[arg(default_values_t = vec![".".to_string()])]
        roots: Vec<String>,

        /// Profile to use: conservative, aggressive, or path to a .toml file.
        #[arg(short, long, default_value = "conservative")]
        profile: String,

        /// Group candidates by kind, path prefix, or none.
        #[arg(short, long, value_enum, default_value_t = GroupBy::None)]
        group_by: GroupBy,

        /// Output format.
        #[arg(short, long, value_enum, default_value_t = OutputFmt::Table)]
        output: OutputFmt,

        /// Only show candidates with score above this threshold (0.0–1.0).
        #[arg(long, default_value_t = 0.0)]
        min_score: f32,
    },

    /// Clean selected candidates (dry-run by default — use --apply to actually delete).
    Clean {
        /// Directories to scan.
        #[arg(default_values_t = vec![".".to_string()])]
        roots: Vec<String>,

        /// Profile to use.
        #[arg(short, long, default_value = "conservative")]
        profile: String,

        /// Apply changes.  Without this flag the command only prints what it would do.
        #[arg(long)]
        apply: bool,
    },

    /// Launch the interactive TUI (alias for `reclaim-tui`).
    Tui {
        #[arg(default_values_t = vec![".".to_string()])]
        roots: Vec<String>,
    },
}

#[derive(Clone, ValueEnum)]
enum GroupBy {
    None,
    Kind,
    Path,
}

#[derive(Clone, ValueEnum)]
enum OutputFmt {
    Table,
    Json,
    Csv,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Scan { roots, profile, group_by, output, min_score } => {
            cmd_scan(roots, &profile, group_by, output, min_score)
        }
        Commands::Clean { roots, profile, apply } => {
            cmd_clean(roots, &profile, apply)
        }
        Commands::Tui { roots: _ } => {
            eprintln!("Tip: run `reclaim-tui` for the full interactive experience.");
            eprintln!("TUI is not yet embedded in the CLI binary.");
            Ok(())
        }
    }
}

fn cmd_scan(
    roots: Vec<String>,
    profile_arg: &str,
    group_by: GroupBy,
    output: OutputFmt,
    min_score: f32,
) -> Result<()> {
    let profile = load_profile(profile_arg)?;
    let roots: Vec<PathBuf> = roots.iter().map(|r| expand_tilde(r)).collect();

    eprint!("Scanning {} root(s)...", roots.len());
    let mut candidates = scanner::scan(&roots, &profile)?;
    strategy::apply(&mut candidates, &profile);
    eprintln!("  done ({} candidates)", candidates.len());

    // Filter by min_score.
    let candidates: Vec<_> = candidates
        .into_iter()
        .filter(|c| c.score >= min_score)
        .collect();

    let report = Report::build(&candidates);

    match output {
        OutputFmt::Table => print_table(&candidates, &report, &group_by),
        OutputFmt::Json  => println!("{}", serde_json::to_string_pretty(&candidates)
            .unwrap_or_else(|_| "[]".to_string())),
        OutputFmt::Csv   => print_csv(&candidates),
    }

    println!("\n{}", report.summary_line());
    Ok(())
}

fn cmd_clean(roots: Vec<String>, profile_arg: &str, apply: bool) -> Result<()> {
    if !apply {
        println!("Dry-run mode — no files will be deleted.  Pass --apply to proceed.");
    }
    let profile = load_profile(profile_arg)?;
    let roots: Vec<PathBuf> = roots.iter().map(|r| expand_tilde(r)).collect();

    let mut candidates = scanner::scan(&roots, &profile)?;
    strategy::apply(&mut candidates, &profile);

    let to_clean: Vec<_> = candidates
        .iter()
        .filter(|c| c.action.is_active())
        .collect();

    println!("{} candidates selected for cleanup:", to_clean.len());
    for c in &to_clean {
        println!("  {:>10}  {}  {}", c.size_human(), c.action.display(), c.path.display());
    }

    if apply {
        let mut freed = 0u64;
        for c in &to_clean {
            match &c.action {
                Action::Delete => {
                    if c.path.is_dir() {
                        std::fs::remove_dir_all(&c.path)?;
                    } else {
                        std::fs::remove_file(&c.path)?;
                    }
                    freed += c.size_bytes;
                }
                Action::Exec { cmd, args, description } => {
                    println!("  Running: {description}");
                    let exit = std::process::Command::new(cmd)
                        .args(args)
                        .status()
                        .map_err(|e| anyhow::anyhow!("Failed to run '{cmd}': {e}"))?;
                    if !exit.success() {
                        eprintln!("  Warning: '{description}' exited with {exit}");
                    }
                    freed += c.size_bytes; // optimistic
                }
                _ => {}
            }
        }
        println!("Freed {}", human_bytes(freed));
    }

    Ok(())
}

fn load_profile(arg: &str) -> Result<Profile> {
    let path = match arg {
        "conservative" => bundled_profile("conservative"),
        "aggressive"   => bundled_profile("aggressive"),
        "dev"          => bundled_profile("dev"),
        other          => return Profile::load(std::path::Path::new(other)),
    };
    Profile::load(&path)
}

/// Resolve a bundled profile relative to the binary location or workspace root.
fn bundled_profile(name: &str) -> PathBuf {
    // Look for profiles/ next to the binary first, then relative to cwd.
    let binary_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();

    for base in [binary_dir, std::env::current_dir().unwrap_or_default()] {
        let candidate = base.join("profiles").join(format!("{name}.toml"));
        if candidate.exists() {
            return candidate;
        }
    }
    // Fallback: let Profile::load fail with a clear error.
    PathBuf::from(format!("profiles/{name}.toml"))
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        dirs::home_dir()
            .map(|h| h.join(stripped))
            .unwrap_or_else(|| PathBuf::from(path))
    } else {
        PathBuf::from(path)
    }
}

fn print_table(candidates: &[reclaim_core::candidate::Candidate], _report: &Report, group_by: &GroupBy) {
    match group_by {
        GroupBy::None => {
            println!("{:<12} {:<14} {:>6}  {:<8}  {:<30}  {}", "SIZE", "KIND", "SCORE", "AGE(d)", "ACTION", "PATH");
            println!("{}", "-".repeat(110));
            for c in candidates {
                println!(
                    "{:<12} {:<14} {:>6.2}  {:<8}  {:<30}  {}",
                    c.size_human(),
                    c.kind.label(),
                    c.score,
                    c.age_days().map(|d| d.to_string()).unwrap_or("?".to_string()),
                    c.action.display(),
                    c.path.display(),
                );
            }
        }
        GroupBy::Kind => {
            // Sort and group by kind label.
            let mut by_kind: std::collections::HashMap<&str, Vec<_>> =
                std::collections::HashMap::new();
            for c in candidates {
                by_kind.entry(c.kind.label()).or_default().push(c);
            }
            let mut kinds: Vec<_> = by_kind.keys().copied().collect();
            kinds.sort();
            for kind in kinds {
                let group = &by_kind[kind];
                let total: u64 = group.iter().map(|c| c.size_bytes).sum();
                println!("\n[{}]  {} items  {}", kind, group.len(), human_bytes(total));
                for c in group {
                    println!("  {:>10}  score={:.2}  {}", c.size_human(), c.score, c.path.display());
                }
            }
        }
        GroupBy::Path => {
            // Group by parent directory.
            let mut by_parent: std::collections::HashMap<String, Vec<_>> =
                std::collections::HashMap::new();
            for c in candidates {
                let parent = c.path.parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                by_parent.entry(parent).or_default().push(c);
            }
            let mut parents: Vec<_> = by_parent.keys().cloned().collect();
            parents.sort();
            for parent in parents {
                let group = &by_parent[&parent];
                let total: u64 = group.iter().map(|c| c.size_bytes).sum();
                println!("\n[{}]  {}", parent, human_bytes(total));
                for c in group {
                    println!("  {:>10}  {}  {}", c.size_human(), c.kind.label(), c.path.file_name().unwrap_or_default().to_string_lossy());
                }
            }
        }
    }
}

fn print_csv(candidates: &[reclaim_core::candidate::Candidate]) {
    println!("path,kind,size_bytes,score,age_days,action");
    for c in candidates {
        println!(
            "{},{},{},{:.3},{},{}",
            c.path.display(),
            c.kind.label(),
            c.size_bytes,
            c.score,
            c.age_days().map(|d| d.to_string()).unwrap_or_default(),
            c.action.label(),
        );
    }
}
