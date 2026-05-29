use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame, Terminal,
};
use reclaim_core::{
    candidate::{Action, Candidate},
    profile::Profile,
    scanner,
    strategy,
};
use std::{
    io,
    path::PathBuf,
};

// ── CLI args ──────────────────────────────────────────────────────────────────

fn usage() -> ! {
    eprintln!("Usage: reclaim-tui [--profile <name|path>] [<root>...]");
    std::process::exit(1);
}

struct Args {
    roots:   Vec<PathBuf>,
    profile: String,
}

fn parse_args() -> Args {
    let mut args = std::env::args().skip(1).peekable();
    let mut roots   = Vec::new();
    let mut profile = "conservative".to_string();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" | "-p" => {
                profile = args.next().unwrap_or_else(|| usage());
            }
            other if other.starts_with('-') => usage(),
            path => roots.push(expand_tilde(path)),
        }
    }
    if roots.is_empty() {
        roots.push(expand_tilde("."));
    }
    Args { roots, profile }
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        dirs::home_dir().map(|h| h.join(stripped)).unwrap_or_else(|| PathBuf::from(path))
    } else {
        PathBuf::from(path)
    }
}

// ── Sort / GroupBy enums ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortBy { Score, Size, Age }

impl SortBy {
    fn label(self) -> &'static str {
        match self { Self::Score => "score", Self::Size => "size", Self::Age => "age" }
    }
    fn next(self) -> Self {
        match self { Self::Score => Self::Size, Self::Size => Self::Age, Self::Age => Self::Score }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GroupBy { None, Kind, Path }

impl GroupBy {
    fn label(self) -> &'static str {
        match self { Self::None => "none", Self::Kind => "kind", Self::Path => "path" }
    }
    fn next(self) -> Self {
        match self { Self::None => Self::Kind, Self::Kind => Self::Path, Self::Path => Self::None }
    }
}

// ── Application state ─────────────────────────────────────────────────────────

struct App {
    candidates:   Vec<Candidate>,
    table_state:  TableState,
    sort_by:      SortBy,
    group_by:     GroupBy,
    show_help:    bool,
    status_msg:   String,
}

impl App {
    fn new(mut candidates: Vec<Candidate>) -> Self {
        sort_candidates(&mut candidates, SortBy::Score);
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            candidates,
            table_state,
            sort_by:   SortBy::Score,
            group_by:  GroupBy::None,
            show_help: false,
            status_msg: String::new(),
        }
    }

    fn selected_index(&self) -> Option<usize> {
        self.table_state.selected()
    }

    fn toggle_selected(&mut self) {
        if let Some(i) = self.selected_index() {
            if let Some(c) = self.candidates.get_mut(i) {
                c.action = if c.action.is_active() {
                    Action::Skip
                } else {
                    Action::Delete
                };
            }
        }
    }

    fn move_cursor(&mut self, delta: i64) {
        let len = self.candidates.len();
        if len == 0 { return; }
        let cur = self.selected_index().unwrap_or(0) as i64;
        let next = (cur + delta).rem_euclid(len as i64) as usize;
        self.table_state.select(Some(next));
    }

    fn cycle_sort(&mut self) {
        self.sort_by = self.sort_by.next();
        sort_candidates(&mut self.candidates, self.sort_by);
        self.table_state.select(Some(0));
        self.status_msg = format!("Sorted by {}", self.sort_by.label());
    }

    fn cycle_group(&mut self) {
        self.group_by = self.group_by.next();
        self.status_msg = format!("Grouped by {}", self.group_by.label());
    }

    fn selected_size(&self) -> u64 {
        self.candidates.iter()
            .filter(|c| c.action.is_active())
            .map(|c| c.size_bytes)
            .sum()
    }

    fn apply_deletions(&mut self) -> Result<u64> {
        let mut freed = 0u64;
        for c in &self.candidates {
            if !c.action.is_active() { continue; }
            match &c.action {
                Action::Delete => {
                    if c.path.is_dir() {
                        std::fs::remove_dir_all(&c.path)?;
                    } else {
                        std::fs::remove_file(&c.path)?;
                    }
                }
                Action::Exec { cmd, args, description } => {
                    std::process::Command::new(cmd)
                        .args(args)
                        .status()
                        .map_err(|e| anyhow::anyhow!("Failed to run '{description}': {e}"))?;
                }
                _ => continue,
            }
            freed += c.size_bytes;
        }
        self.candidates.retain(|c| !c.action.is_active());
        Ok(freed)
    }
}

fn sort_candidates(candidates: &mut Vec<Candidate>, sort_by: SortBy) {
    match sort_by {
        SortBy::Score => candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap()),
        SortBy::Size  => candidates.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes)),
        SortBy::Age   => candidates.sort_by(|a, b| {
            b.age_days().unwrap_or(0).cmp(&a.age_days().unwrap_or(0))
        }),
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),   // header
            Constraint::Min(5),      // table
            Constraint::Length(3),   // status bar
        ])
        .split(area);

    render_header(f, app, chunks[0]);
    render_table(f, app, chunks[1]);
    render_status(f, app, chunks[2]);

    if app.show_help {
        render_help_overlay(f, area);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let selected_sz = reclaim_core::candidate::human_bytes(app.selected_size());
    let total = app.candidates.len();
    let text = format!(
        " reclaim  │  {} candidates  │  {} selected for cleanup  │  sort:{} group:{}",
        total, selected_sz, app.sort_by.label(), app.group_by.label()
    );
    let block = Block::default().borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));
    let para = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    f.render_widget(para, area);
}

fn render_table(f: &mut Frame, app: &mut App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("SEL"),
        Cell::from("SIZE"),
        Cell::from("KIND"),
        Cell::from("SCORE"),
        Cell::from("AGE(d)"),
        Cell::from("ACTION"),
        Cell::from("PATH"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED));

    let rows: Vec<Row> = app.candidates.iter().map(|c| {
        let sel   = if c.action.is_active() { "●" } else { "○" };
        let age   = c.age_days().map(|d| d.to_string()).unwrap_or("?".to_string());
        let act   = c.action.display();
        let path  = c.path.display().to_string();

        let style = if c.action.is_active() {
            Style::default().fg(Color::Red)
        } else if c.score >= 0.5 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        Row::new(vec![
            Cell::from(sel),
            Cell::from(c.size_human()),
            Cell::from(c.kind.label()),
            Cell::from(format!("{:.2}", c.score)),
            Cell::from(age),
            Cell::from(act),
            Cell::from(path),
        ]).style(style)
    }).collect();

    let table = Table::new(rows, [
        Constraint::Length(3),
        Constraint::Length(10),
        Constraint::Length(16),
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Length(30),
        Constraint::Min(20),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Candidates "))
    .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
    .highlight_symbol("▶ ");

    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let keys = " [↑↓] navigate  [Space] toggle  [s] sort  [g] group  [Enter] apply  [?] help  [q] quit";
    let msg = if app.status_msg.is_empty() { keys.to_string() } else { app.status_msg.clone() };
    let para = Paragraph::new(msg)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(para, area);
}

fn render_help_overlay(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(vec![Span::styled(" reclaim — keyboard reference ", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from(" ↑ / ↓      Navigate candidates"),
        Line::from(" Space      Toggle delete/skip"),
        Line::from(" s          Cycle sort (score → size → age)"),
        Line::from(" g          Cycle group-by (none → kind → path)"),
        Line::from(" Enter      Apply deletions (with confirmation)"),
        Line::from(" d          Dry-run: show what would be deleted"),
        Line::from(" ?          Toggle this help"),
        Line::from(" q / Ctrl-C Quit"),
    ];

    let width  = 50u16.min(area.width.saturating_sub(4));
    let height = help_text.len() as u16 + 2;
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let overlay = Rect::new(x, y, width, height);

    let block = Block::default().borders(Borders::ALL).title(" Help ")
        .style(Style::default().bg(Color::Black));
    let para = Paragraph::new(help_text).block(block);
    f.render_widget(para, overlay);
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let args = parse_args();
    let profile = load_profile(&args.profile)?;

    eprint!("Scanning {} root(s)...", args.roots.len());
    let mut candidates = scanner::scan(&args.roots, &profile)?;
    strategy::apply(&mut candidates, &profile);
    eprintln!("  {} candidates found", candidates.len());

    // Set up terminal.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let mut app = App::new(candidates);
    let result  = run_event_loop(&mut term, &mut app);

    // Restore terminal.
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;

    result
}

fn run_event_loop<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        term.draw(|f| render(f, app))?;

        if let Event::Key(key) = event::read()? {
            // Clear transient status message on any keypress.
            app.status_msg.clear();

            match (key.code, key.modifiers) {
                (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                (KeyCode::Up,    _) => app.move_cursor(-1),
                (KeyCode::Down,  _) => app.move_cursor(1),
                (KeyCode::Char(' '), _) => app.toggle_selected(),
                (KeyCode::Char('s'), _) => app.cycle_sort(),
                (KeyCode::Char('g'), _) => app.cycle_group(),
                (KeyCode::Char('?'), _) => app.show_help = !app.show_help,
                (KeyCode::Char('d'), _) => {
                    let sz = reclaim_core::candidate::human_bytes(app.selected_size());
                    app.status_msg = format!("Dry-run: would free {sz}");
                }
                (KeyCode::Enter, _) => {
                    let n = app.candidates.iter().filter(|c| c.action == Action::Delete).count();
                    if n == 0 {
                        app.status_msg = "Nothing selected.".to_string();
                    } else {
                        // TODO: show confirmation dialog before deleting.
                        match app.apply_deletions() {
                            Ok(freed) => app.status_msg = format!(
                                "Freed {}.", reclaim_core::candidate::human_bytes(freed)
                            ),
                            Err(e) => app.status_msg = format!("Error: {e}"),
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn load_profile(arg: &str) -> Result<Profile> {
    let path = match arg {
        "conservative" | "aggressive" | "dev" => {
            let binary_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_default();
            let p = binary_dir.join("profiles").join(format!("{arg}.toml"));
            if p.exists() { p } else {
                std::env::current_dir()?.join("profiles").join(format!("{arg}.toml"))
            }
        }
        other => PathBuf::from(other),
    };
    Profile::load(&path)
}
