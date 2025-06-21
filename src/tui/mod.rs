// src/tui/mod.rs
// TUI dashboard module using ratatui

use std::collections::VecDeque;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::Terminal;
use ratatui::text::{Span, Line};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Row, Table, Cell};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use tokio::sync::{mpsc, RwLock};
use ratatui::Frame;
use regex::Regex;
use chrono::{DateTime, Local};

// --- Shared State Structures ---
#[derive(Clone, Debug)]
pub struct FeedStatus {
    pub name: String,
    pub last_value: String,
    pub last_update: Instant,
    pub next_update: Instant,
    pub error: Option<String>,
}

impl Default for FeedStatus {
    fn default() -> Self {
        Self {
            name: String::new(),
            last_value: String::new(),
            last_update: Instant::now(),
            next_update: Instant::now(),
            error: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NetworkStatus {
    pub name: String,
    pub rpc_ok: bool,
    pub chain_id: Option<u64>,
    pub block_number: Option<u64>,
    pub wallet_status: String,
}

#[derive(Clone, Debug, Default)]
pub struct MetricsState {
    pub feed_count: usize,
    pub error_count: usize,
    pub tx_count: usize,
    pub last_tx_cost: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct DashboardState {
    pub logs: VecDeque<LogEntry>,
    pub feeds: Vec<FeedStatus>,
    pub networks: Vec<NetworkStatus>,
    pub metrics: MetricsState,
    pub alerts: Vec<String>,
    pub selected_panel: usize,
    pub log_scroll: usize,
    pub filter: Option<String>,
    pub compact_logs: bool,
    pub group_state: LogGroupState,
}

#[derive(Clone, Debug, Default)]
pub struct LogGroupState {
    pub collapsed: std::collections::HashSet<String>, // target/module names
    pub search: Option<String>,
    pub search_active: bool,
    pub last_error_count: usize,
    pub last_warn_count: usize,
    pub animate_alert: bool,
    pub animate_tick: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn color(self) -> Color {
        match self {
            LogLevel::Info => Color::Cyan,
            LogLevel::Warn => Color::Yellow,
            LogLevel::Error => Color::Red,
            LogLevel::Debug => Color::Green,
            LogLevel::Trace => Color::Gray,
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            LogLevel::Info => "ℹ️ ",
            LogLevel::Warn => "⚠️ ",
            LogLevel::Error => "❌",
            LogLevel::Debug => "🐛",
            LogLevel::Trace => "🔍",
        }
    }
}

// --- Log Channel and Tracing Layer ---
pub struct ChannelWriter(pub mpsc::Sender<LogEntry>);
impl std::io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let msg = String::from_utf8_lossy(buf).to_string();
        // Accept both compact and default tracing-subscriber formats
        // Compact: [2025-06-21T12:34:56.789Z INFO omikuji::main] message\n
        let re = Regex::new(r"^\[(?P<ts>[^\]]+) (?P<level>\w+) (?P<target>[^\]]+)] (?P<msg>.*)$").unwrap();
        let (timestamp, level, target, message) = if let Some(caps) = re.captures(&msg) {
            let ts = DateTime::parse_from_rfc3339(&caps["ts"]).ok().map(|dt| dt.with_timezone(&Local)).unwrap_or(Local::now());
            let level = match &caps["level"] {
                "ERROR" => LogLevel::Error,
                "WARN" => LogLevel::Warn,
                "DEBUG" => LogLevel::Debug,
                "TRACE" => LogLevel::Trace,
                _ => LogLevel::Info,
            };
            (ts, level, caps["target"].to_string(), caps["msg"].to_string())
        } else {
            // Fallback: try to parse level and target from the line, else treat as info
            let level = if msg.contains("ERROR") {
                LogLevel::Error
            } else if msg.contains("WARN") {
                LogLevel::Warn
            } else if msg.contains("DEBUG") {
                LogLevel::Debug
            } else if msg.contains("TRACE") {
                LogLevel::Trace
            } else {
                LogLevel::Info
            };
            (Local::now(), level, "app".to_string(), msg.trim().to_string())
        };
        let entry = LogEntry { timestamp, level, target, message };
        if let Err(e) = self.0.try_send(entry) {
            eprintln!("TUI ChannelWriter failed to send log: {}", e);
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

pub async fn start_tui_dashboard() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Provide a dummy dashboard for now
    let dashboard = Arc::new(RwLock::new(DashboardState::default()));
    let res = run_app(&mut terminal, dashboard).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    res
}

// --- TUI Main Entry ---
pub async fn start_tui_dashboard_with_state(
    dashboard: Arc<RwLock<DashboardState>>,
    mut log_rx: mpsc::Receiver<LogEntry>,
) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Spawn a task to receive logs and update dashboard state
    let dashboard_clone = dashboard.clone();
    tokio::spawn(async move {
        while let Some(entry) = log_rx.recv().await {
            let mut dash = dashboard_clone.write().await;
            if dash.logs.len() > 1000 { dash.logs.pop_front(); }
            dash.logs.push_back(entry);
        }
        // If the channel closes, print a message for debugging
        eprintln!("TUI log receiver channel closed");
    });

    let res = run_app(&mut terminal, dashboard.clone()).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    res
}

// --- TUI Event Loop and Rendering ---
async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    dashboard: Arc<RwLock<DashboardState>>,
) -> io::Result<()> {
    use ratatui::widgets::Tabs;
    use ratatui::style::Style;
    use std::collections::HashMap;
    let panel_titles = [
        Line::from("Metrics"),
        Line::from("Feeds"),
        Line::from("Network"),
        Line::from("Alerts"),
    ];
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(200);
    loop {
        let mut dash = dashboard.write().await;
        // Animate alerts
        dash.group_state.animate_tick = dash.group_state.animate_tick.wrapping_add(1);
        // --- FIX: Clamp log_scroll to available lines after new logs arrive ---
        // Build flat_lines for clamping
        let mut group_map: HashMap<String, Vec<&LogEntry>> = HashMap::new();
        for entry in dash.logs.iter() {
            group_map.entry(entry.target.clone()).or_default().push(entry);
        }
        let mut flat_lines = Vec::new();
        let mut groups: Vec<_> = group_map.into_iter().collect();
        groups.sort_by(|a, b| b.1.last().unwrap().timestamp.cmp(&a.1.last().unwrap().timestamp));
        for (target, entries) in &groups {
            flat_lines.push(());
            if !dash.group_state.collapsed.contains(target) {
                for _ in entries.iter().rev() {
                    flat_lines.push(());
                }
            }
        }
        let total = flat_lines.len();
        let max_visible = 30;
        // If user is at bottom, keep at bottom as new logs arrive
        if dash.log_scroll == 0 {
            dash.log_scroll = 0;
        } else {
            dash.log_scroll = dash.log_scroll.min(total.saturating_sub(max_visible));
        }
        drop(dash); // Release lock for draw
        let dash = dashboard.read().await.clone();
        terminal.draw(|f| {
            let size = f.size();
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(2), // Tab bar
                    Constraint::Min(8),   // Tab content
                    Constraint::Length((size.height / 3).max(5)), // Logs bottom third
                ])
                .split(size);
            // Tab bar
            let tabs = Tabs::new(panel_titles.to_vec())
                .select(dash.selected_panel)
                .block(Block::default().borders(Borders::ALL).title("Omikuji Dashboard"))
                .highlight_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD));
            f.render_widget(tabs, layout[0]);
            // Tab content
            match dash.selected_panel {
                0 => render_metrics(f, layout[1], &dash),
                1 => render_feeds(f, layout[1], &dash),
                2 => render_network(f, layout[1], &dash),
                3 => render_alerts(f, layout[1], &dash),
                _ => {},
            }
            // Logs always at bottom
            render_logs(f, layout[2], &dash);
        })?;
        // Handle input
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                let mut dash = dashboard.write().await;
                // --- FIX: Use total lines for scroll clamping ---
                // Build flat_lines for clamping
                let mut group_map: HashMap<String, Vec<&LogEntry>> = HashMap::new();
                for entry in dash.logs.iter() {
                    group_map.entry(entry.target.clone()).or_default().push(entry);
                }
                let mut flat_lines = Vec::new();
                let mut groups: Vec<_> = group_map.into_iter().collect();
                groups.sort_by(|a, b| b.1.last().unwrap().timestamp.cmp(&a.1.last().unwrap().timestamp));
                for (target, entries) in &groups {
                    flat_lines.push(());
                    if !dash.group_state.collapsed.contains(target) {
                        for _ in entries.iter().rev() {
                            flat_lines.push(());
                        }
                    }
                }
                let total = flat_lines.len();
                let max_visible = 30;
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Tab => { dash.selected_panel = (dash.selected_panel + 1) % panel_titles.len(); },
                    KeyCode::Up => {
                        if dash.log_scroll < total.saturating_sub(1) {
                            dash.log_scroll += 1;
                        }
                        dash.log_scroll = dash.log_scroll.min(total.saturating_sub(max_visible));
                    },
                    KeyCode::Down => {
                        if dash.log_scroll > 0 {
                            dash.log_scroll -= 1;
                        }
                        // Clamp to zero
                        if dash.log_scroll > total.saturating_sub(max_visible) {
                            dash.log_scroll = 0;
                        }
                    },
                    KeyCode::Char('f') => { dash.filter = Some(String::new()); },
                    KeyCode::Char('c') => { dash.compact_logs = !dash.compact_logs; },
                    KeyCode::Char('/') => { dash.group_state.search_active = true; dash.group_state.search = Some(String::new()); },
                    KeyCode::Char('e') => { dash.group_state.collapsed.clear(); }, // Expand all
                    KeyCode::Char('x') => { // Collapse all
                        dash.group_state.collapsed = dash.logs.iter().map(|l| l.target.clone()).collect();
                    },
                    KeyCode::Enter => { // Toggle collapse for group under cursor
                        let group_target = dash.logs.iter().rev().skip(dash.log_scroll).next().map(|g| g.target.clone());
                        if let Some(target) = group_target {
                            if dash.group_state.collapsed.contains(&target) {
                                dash.group_state.collapsed.remove(&target);
                            } else {
                                dash.group_state.collapsed.insert(target);
                            }
                        }
                    },
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_rate { last_tick = Instant::now(); }
    }
    Ok(())
}

// --- Panel Renderers ---
fn render_metrics(f: &mut Frame, area: Rect, dash: &DashboardState) {
    let m = &dash.metrics;
    let rows = vec![
        Row::new(vec![Cell::from("Feeds"), Cell::from(m.feed_count.to_string())]),
        Row::new(vec![Cell::from("Errors"), Cell::from(m.error_count.to_string())]),
        Row::new(vec![Cell::from("Txs"), Cell::from(m.tx_count.to_string())]),
        Row::new(vec![Cell::from("Last Tx Cost"), Cell::from(m.last_tx_cost.map(|c| format!("{c:.4} ETH")).unwrap_or("-".to_string()))]),
    ];
    let table = Table::new(rows, [Constraint::Length(16), Constraint::Min(8)])
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Metrics", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))));
    f.render_widget(table, area);
}

fn render_feeds(f: &mut Frame, area: Rect, dash: &DashboardState) {
    let rows: Vec<Row> = dash.feeds.iter().map(|feed| {
        let countdown = feed.next_update.saturating_duration_since(Instant::now()).as_secs();
        Row::new(vec![
            Cell::from(feed.name.clone()),
            Cell::from(feed.last_value.clone()),
            Cell::from(format!("{}s", countdown)),
            Cell::from(feed.error.clone().unwrap_or_default()),
        ])
    }).collect();
    let table = Table::new(rows, [Constraint::Length(16), Constraint::Length(16), Constraint::Length(8), Constraint::Min(8)])
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Feeds", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))));
    f.render_widget(table, area);
}

fn render_network(f: &mut Frame, area: Rect, dash: &DashboardState) {
    let rows: Vec<Row> = dash.networks.iter().map(|net| {
        Row::new(vec![
            Cell::from(net.name.clone()),
            Cell::from(net.chain_id.map(|id| id.to_string()).unwrap_or("-".to_string())),
            Cell::from(net.block_number.map(|b| b.to_string()).unwrap_or("-".to_string())),
            Cell::from(net.wallet_status.clone()),
            Cell::from(if net.rpc_ok { "OK" } else { "ERR" }),
        ])
    }).collect();
    let table = Table::new(rows, [Constraint::Length(16), Constraint::Length(8), Constraint::Length(12), Constraint::Length(12), Constraint::Length(6)])
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Network", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))));
    f.render_widget(table, area);
}

// --- Panel Renderers ---
fn render_logs(f: &mut Frame, area: Rect, dash: &DashboardState) {
    use std::collections::HashMap;
    let filter = dash.filter.as_ref().map(|s| s.to_lowercase());
    let highlight_words = ["failed", "success", "timeout", "panic", "error", "warn", "critical"];
    let mut log_items: Vec<ListItem> = Vec::new();
    let mut group_map: HashMap<String, Vec<&LogEntry>> = HashMap::new();
    let mut error_count = 0;
    let mut warn_count = 0;
    // Group logs by target/module
    for entry in dash.logs.iter() {
        if entry.level == LogLevel::Error { error_count += 1; }
        if entry.level == LogLevel::Warn { warn_count += 1; }
        group_map.entry(entry.target.clone()).or_default().push(entry);
    }
    // Build a flat list of all visible log lines (group headers + entries)
    let mut flat_lines = Vec::new();
    let mut groups: Vec<_> = group_map.into_iter().collect();
    groups.sort_by(|a, b| b.1.last().unwrap().timestamp.cmp(&a.1.last().unwrap().timestamp));
    for (target, entries) in &groups {
        // Group header
        let collapsed = dash.group_state.collapsed.contains(target);
        let group_label = format!("{} [{}]", target, entries.len());
        let group_color = if entries.iter().any(|e| e.level == LogLevel::Error) {
            Color::Red
        } else if entries.iter().any(|e| e.level == LogLevel::Warn) {
            Color::Yellow
        } else {
            Color::Blue
        };
        let mut group_title = vec![Span::styled(group_label, Style::default().fg(group_color).add_modifier(Modifier::BOLD))];
        if collapsed {
            group_title.push(Span::raw(" [collapsed]"));
        }
        flat_lines.push(ListItem::new(Line::from(group_title)));
        // Entries
        if !collapsed {
            for entry in entries.iter().rev() {
                if let Some(ref filter) = filter {
                    if !entry.message.to_lowercase().contains(filter) && !entry.target.to_lowercase().contains(filter) {
                        continue;
                    }
                }
                let mut spans = vec![
                    Span::styled(
                        // Improved timestamp format: 'YYYY-MM-DD HH:MM:SS.mmm'
                        format!("{}", entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f")),
                        Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                    ),
                    Span::raw(" "),
                    Span::styled(entry.level.icon(), Style::default().fg(entry.level.color())),
                    Span::raw(" "),
                    Span::styled(
                        format!("{}", entry.target),
                        Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(": "),
                ];
                let mut last = 0;
                let msg = &entry.message;
                let mut found = false;
                for word in highlight_words.iter() {
                    if let Some(idx) = msg.to_lowercase().find(word) {
                        if idx > last {
                            spans.push(Span::raw(&msg[last..idx]));
                        }
                        let color = match *word {
                            "failed" | "panic" | "error" | "critical" => Color::Red,
                            "warn" | "timeout" => Color::Yellow,
                            "success" => Color::Green,
                            _ => Color::White,
                        };
                        spans.push(Span::styled(&msg[idx..idx+word.len()], Style::default().fg(color).add_modifier(Modifier::BOLD)));
                        last = idx + word.len();
                        found = true;
                        break;
                    }
                }
                if !found {
                    spans.push(Span::raw(&msg[last..]));
                } else if last < msg.len() {
                    spans.push(Span::raw(&msg[last..]));
                }
                let line = Line::from(spans);
                flat_lines.push(ListItem::new(line));
                if msg.contains('\n') {
                    for sub in msg.split('\n').skip(1) {
                        flat_lines.push(ListItem::new(Line::from(vec![Span::raw(format!("    {sub}"))])));
                    }
                }
            }
        }
    }
    // --- FIX: Clamp scroll and always show latest logs in real time ---
    let max_visible = 30;
    let total = flat_lines.len();
    // Clamp log_scroll to available lines
    let log_scroll = dash.log_scroll.min(total.saturating_sub(max_visible));
    let start = total.saturating_sub(max_visible + log_scroll);
    let end = total.saturating_sub(log_scroll);
    let visible = if start < end && end <= total { &flat_lines[start..end] } else { &[] };
    let logs = List::new(visible.to_vec())
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            format!("Logs [{} errors, {} warns, {} total]{}", error_count, warn_count, dash.logs.len(), if dash.compact_logs {" (compact)"} else {""}),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
    f.render_widget(logs, area);
}

// --- Alerts Panel: Animated effect for critical alerts ---
fn render_alerts(f: &mut Frame, area: Rect, dash: &DashboardState) {
    let animate = dash.group_state.animate_tick % 8 < 4;
    let items: Vec<ListItem> = dash.alerts.iter().rev().take(10)
        .map(|msg| {
            let is_critical = msg.to_lowercase().contains("critical") || msg.to_lowercase().contains("error");
            let style = if is_critical && animate {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD | Modifier::RAPID_BLINK)
            } else {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            };
            ListItem::new(Line::from(vec![Span::styled(msg, style)]))
        })
        .collect();
    let alerts = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Alerts", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))));
    f.render_widget(alerts, area);
}
