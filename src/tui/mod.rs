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
pub mod update;
pub mod metrics;
pub mod db_metrics;
use ratatui::symbols::bar;

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
    // --- Added for live metrics ---
    pub update_success_count: usize, // total successful updates
    pub update_total_count: usize,   // total attempted updates
    pub avg_staleness_secs: f64,     // average staleness in seconds
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
    // --- New fields for redesign ---
    pub autoscroll: bool,
    pub input_mode: bool,
    pub input_buffer: String,
    pub show_help: bool,
    pub gas_price_gwei_hist: Ring<120, f64>,
    pub response_time_ms_hist: Ring<120, u64>,
    // pub theme: Theme, // Uncomment if theming is implemented
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

// --- Ring Buffer for History ---
#[derive(Clone, Debug)]
pub struct Ring<const N: usize, T: Copy + Default>(pub [T; N], pub usize);

impl<const N: usize, T: Copy + Default> Default for Ring<N, T> {
    fn default() -> Self {
        Self([T::default(); N], 0)
    }
}

impl<const N: usize, T: Copy + Default> Ring<N, T> {
    pub fn push(&mut self, v: T) {
        self.0[self.1 % N] = v;
        self.1 += 1;
    }
    pub fn as_vec(&self) -> Vec<T> {
        let filled = self.1.min(N);
        let mut out = Vec::with_capacity(filled);
        for i in 0..filled {
            out.push(self.0[(self.1 - filled + i) % N]);
        }
        out
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
        Line::from("Live"),
        Line::from("Feeds"),
    ];
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(200);
    loop {
        let mut dash = dashboard.write().await;
        dash.group_state.animate_tick = dash.group_state.animate_tick.wrapping_add(1);
        // Clamp log_scroll to available lines after new logs arrive
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
            // Layout: Overview (top), Main panel (tabs), Network bar, Command input, Logs
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(9), // Overview panel (fixed height)
                    Constraint::Min(8),    // Main panel (tabs)
                    Constraint::Length(1), // Network bar
                    Constraint::Length(if dash.input_mode { 3 } else { 0 }), // Command input
                    Constraint::Length((size.height / 3).max(5)), // Logs
                ])
                .split(size);
            // Overview panel
            render_overview(f, layout[0], &dash);
            // Main panel (tabs)
            let tabs = Tabs::new(panel_titles.to_vec())
                .select(dash.selected_panel)
                .block(Block::default().borders(Borders::ALL).title("Omikuji Dashboard"))
                .highlight_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD));
            f.render_widget(tabs, layout[1]);
            match dash.selected_panel {
                0 => render_panel_live(f, layout[1], &dash),
                1 => render_panel_feeds(f, layout[1], &dash),
                _ => {},
            }
            // Network bar
            render_network_bar(f, layout[2], &dash);
            // Command input (if active)
            if dash.input_mode {
                render_command_input(f, layout[3], &dash);
            }
            // Logs
            render_logs(f, layout[4], &dash);
            // Help overlay (if active)
            if dash.show_help {
                render_help_overlay(f, layout[5], &dash);
            }
        })?;
        // Handle input
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        // --- Updated Key Handling and Command Parsing ---
        // In run_app event loop, after event::poll:
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                let mut dash = dashboard.write().await;
                if dash.input_mode {
                    match key.code {
                        KeyCode::Esc => { dash.input_mode = false; dash.input_buffer.clear(); },
                        KeyCode::Enter => {
                            let cmd = dash.input_buffer.trim().to_string();
                            dash.input_mode = false;
                            dash.input_buffer.clear();
                            // Handle commands
                            match cmd.split_whitespace().next() {
                                Some("ping") => {
                                    let net = cmd.split_whitespace().nth(1).unwrap_or("");
                                    dash.logs.push_back(LogEntry {
                                        timestamp: Local::now(),
                                        level: LogLevel::Info,
                                        target: "cmd".to_string(),
                                        message: format!("Pinging network: {net}"),
                                });
                                },
                                Some("txcost") => {
                                    let feed = cmd.split_whitespace().nth(1).unwrap_or("");
                                    dash.logs.push_back(LogEntry {
                                        timestamp: Local::now(),
                                        level: LogLevel::Info,
                                        target: "cmd".to_string(),
                                        message: format!("Tx cost for feed: {feed}"),
                                    });
                                },
                                Some("clear") => {
                                    dash.logs.clear();
                                },
                                Some("help") => {
                                    dash.logs.push_back(LogEntry {
                                        timestamp: Local::now(),
                                        level: LogLevel::Info,
                                        target: "cmd".to_string(),
                                        message: "Available: ping <network>, txcost <feed>, clear, help".to_string(),
                                    });
                                },
                                _ => {
                                    dash.logs.push_back(LogEntry {
                                        timestamp: Local::now(),
                                        level: LogLevel::Warn,
                                        target: "cmd".to_string(),
                                        message: format!("Unknown command: {cmd}"),
                                    });
                                }
                            }
                        },
                        KeyCode::Char(c) => { dash.input_buffer.push(c); },
                        KeyCode::Backspace => { dash.input_buffer.pop(); },
                        _ => {}
                    }
                } else if dash.show_help {
                    if let KeyCode::Char('?') | KeyCode::Esc = key.code {
                        dash.show_help = false;
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Tab => { dash.selected_panel = (dash.selected_panel + 1) % 2; },
                        KeyCode::Char(':') => { dash.input_mode = true; dash.input_buffer.clear(); },
                        KeyCode::Char('?') => { dash.show_help = !dash.show_help; },
                        KeyCode::Up => {
                            dash.autoscroll = false;
                            dash.log_scroll = dash.log_scroll.saturating_add(1);
                        },
                        KeyCode::Down => {
                            dash.log_scroll = dash.log_scroll.saturating_sub(1);
                            if dash.log_scroll == 0 { dash.autoscroll = true; }
                        },
                        KeyCode::Char('b') => { dash.log_scroll = 0; dash.autoscroll = true; },
                        KeyCode::Char('t') => { dash.log_scroll = 9999; dash.autoscroll = false; },
                        KeyCode::Esc => { dash.input_mode = false; dash.input_buffer.clear(); },
                        _ => {}
                    }
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
    for entry in dash.logs.iter() {
        if entry.level == LogLevel::Error { error_count += 1; }
        if entry.level == LogLevel::Warn { warn_count += 1; }
        group_map.entry(entry.target.clone()).or_default().push(entry);
    }
    let mut flat_lines = Vec::new();
    let mut groups: Vec<_> = group_map.into_iter().collect();
    groups.sort_by(|a, b| b.1.last().unwrap().timestamp.cmp(&a.1.last().unwrap().timestamp));
    for (target, entries) in &groups {
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
        if !collapsed {
            for entry in entries.iter().rev() {
                if let Some(ref filter) = filter {
                    if !entry.message.to_lowercase().contains(filter) && !entry.target.to_lowercase().contains(filter) {
                        continue;
                    }
                }
                let mut spans = vec![
                    Span::styled(
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
    let max_visible = (area.height as usize).saturating_sub(2);
    let total = flat_lines.len();
    let log_scroll = if dash.autoscroll { 0 } else { dash.log_scroll.min(total.saturating_sub(max_visible)) };
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

// --- Overview Panel Renderer ---
fn render_overview(f: &mut Frame, area: Rect, dash: &DashboardState) {
    use ratatui::widgets::{Table, Row, Cell, Block, Borders, Sparkline};
    use ratatui::layout::{Layout, Constraint, Direction};
    use ratatui::style::{Style, Color, Modifier};
    use ratatui::text::Span;
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);
    // Left: Metrics Table
    let m = &dash.metrics;
    let avg_response = dash.response_time_ms_hist.as_vec();
    let avg_response = if avg_response.is_empty() { 0 } else { avg_response.iter().sum::<u64>() / avg_response.len() as u64 };
    let avg_tx_cost = dash.metrics.last_tx_cost.map(|c| format!("{c:.4} ETH")).unwrap_or("-".to_string());
    let rows = vec![
        Row::new(vec![Cell::from("Total Feeds"), Cell::from(m.feed_count.to_string())]),
        Row::new(vec![Cell::from("Active Errors"), Cell::from(m.error_count.to_string())]),
        Row::new(vec![Cell::from("Total Txs Today"), Cell::from(m.tx_count.to_string())]),
        Row::new(vec![Cell::from("Total Updates"), Cell::from(m.update_total_count.to_string())]),
        Row::new(vec![Cell::from("Avg Tx Cost"), Cell::from(avg_tx_cost)]),
        Row::new(vec![Cell::from("Avg Response Time"), Cell::from(format!("{avg_response} ms"))]),
        Row::new(vec![Cell::from("Max Data Staleness"), Cell::from(Span::styled(
            format!("{:.1} s", dash.metrics.avg_staleness_secs),
            if dash.metrics.avg_staleness_secs > 300.0 { Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) } else { Style::default() }
        ))]),
    ];
    let table = Table::new(rows, [Constraint::Length(20), Constraint::Min(10)])
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Overview", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))).style(Style::default().bg(Color::Black)))
        .style(Style::default().bg(Color::Black));
    f.render_widget(table, chunks[0]);
    // Right: Sparklines
    let gas_hist = dash.gas_price_gwei_hist.as_vec().iter().map(|v| *v as u64).collect::<Vec<u64>>();
    let resp_hist = dash.response_time_ms_hist.as_vec();
    let spark_gas = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Gas Price (Gwei)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))).style(Style::default().bg(Color::Black)))
        .data(&gas_hist)
        .style(Style::default().fg(Color::Green).bg(Color::Black))
        .bar_set(bar::NINE_LEVELS);
    let spark_resp = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Response Time (ms)", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))).style(Style::default().bg(Color::Black)))
        .data(&resp_hist)
        .style(Style::default().fg(Color::Blue).bg(Color::Black))
        .bar_set(bar::NINE_LEVELS);
    let spark_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);
    f.render_widget(spark_gas, spark_chunks[0]);
    f.render_widget(spark_resp, spark_chunks[1]);
}

// --- Live Panel Renderer ---
fn render_panel_live(f: &mut Frame, area: Rect, dash: &DashboardState) {
    use ratatui::widgets::{Sparkline, Gauge, Block, Borders};
    use ratatui::layout::{Layout, Constraint, Direction};
    use ratatui::style::{Style, Color, Modifier};
    use ratatui::text::Span;
    // New layout: vertical split, top 50% for charts, bottom 50% for gauges
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50), // Top: charts
            Constraint::Percentage(50), // Bottom: gauges
        ])
        .split(area);
    // Top: horizontal split for charts
    let chart_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);
    // Gas price sparkline
    let gas_hist = dash.gas_price_gwei_hist.as_vec().iter().map(|v| *v as u64).collect::<Vec<u64>>();
    let spark_gas = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Gas Price (Gwei)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))).style(Style::default().bg(Color::Black)))
        .data(&gas_hist)
        .style(Style::default().fg(Color::Green).bg(Color::Black))
        .bar_set(bar::NINE_LEVELS);
    f.render_widget(spark_gas, chart_chunks[0]);
    // Response time sparkline
    let resp_hist = dash.response_time_ms_hist.as_vec();
    let spark_resp = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Response Time (ms)", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))).style(Style::default().bg(Color::Black)))
        .data(&resp_hist)
        .style(Style::default().fg(Color::Blue).bg(Color::Black))
        .bar_set(bar::NINE_LEVELS);
    f.render_widget(spark_resp, chart_chunks[1]);
    // Bottom: vertical split for gauges (each gets 50% of bottom area)
    let gauge_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);
    // Use real metrics from dash.metrics
    let success = dash.metrics.update_success_count as u16;
    let total = dash.metrics.update_total_count as u16;
    let staleness = dash.metrics.avg_staleness_secs;
    let gauge_success = Gauge::default()
        .block(Block::default().title(Span::styled("Update Success Ratio", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))).borders(Borders::ALL).style(Style::default().bg(Color::Black)))
        .gauge_style(Style::default().fg(Color::Magenta).bg(Color::Black).add_modifier(Modifier::BOLD))
        .ratio(if total > 0 { success as f64 / total as f64 } else { 0.0 })
        .label(format!("{:.1}% ({}/{})", if total > 0 { 100.0 * (success as f64 / total as f64) } else { 0.0 }, success, total));
    let gauge_stale = Gauge::default()
        .block(Block::default().title(Span::styled("Avg Staleness", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))).borders(Borders::ALL).style(Style::default().bg(Color::Black)))
        .gauge_style(Style::default().fg(if staleness > 300.0 { Color::Red } else { Color::Cyan }).bg(Color::Black))
        .ratio((staleness.min(600.0)) / 600.0)
        .label(format!("{:.1} s", staleness));
    f.render_widget(gauge_success, gauge_chunks[0]);
    f.render_widget(gauge_stale, gauge_chunks[1]);
}

// --- Feeds Panel Renderer ---
fn render_panel_feeds(f: &mut Frame, area: Rect, dash: &DashboardState) {
    use ratatui::widgets::{Table, Row, Cell, Block, Borders, Gauge, List, ListItem};
    use ratatui::layout::{Layout, Constraint, Direction};
    use ratatui::style::{Style, Color, Modifier};
    use ratatui::text::{Span, Line};
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6), // Feeds table
            Constraint::Length(7), // Alerts
        ])
        .split(area);
    // Feeds Table
    let feed_rows: Vec<Row> = dash.feeds.iter().map(|feed| {
        let countdown = feed.next_update.saturating_duration_since(Instant::now()).as_secs();
        let error_style = if feed.error.is_some() { Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) } else { Style::default() };
        Row::new(vec![
            Cell::from(feed.name.clone()),
            Cell::from(feed.last_value.clone()),
            Cell::from(format!("{}s", countdown)),
            Cell::from(Span::styled(feed.error.clone().unwrap_or_default(), error_style)),
        ])
    }).collect();
    let table = Table::new(feed_rows, [Constraint::Length(16), Constraint::Length(16), Constraint::Length(8), Constraint::Min(8)])
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Feeds", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))));
    f.render_widget(table, chunks[0]);
    // Alerts List
    let animate = dash.group_state.animate_tick % 8 < 4;
    let items: Vec<ListItem> = dash.alerts.iter().rev().take(5)
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
    f.render_widget(alerts, chunks[1]);
}

// --- Network Bar Renderer ---
fn render_network_bar(f: &mut Frame, area: Rect, dash: &DashboardState) {
    use ratatui::widgets::{Paragraph, Block, Borders};
    use ratatui::style::{Style, Color, Modifier};
    use ratatui::text::Span;
    let mut spans = Vec::new();
    for (i, net) in dash.networks.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  |  "));
        }
        let color = if net.rpc_ok { Color::Green } else { Color::Red };
        spans.push(Span::styled(format!("[{}]", net.name), Style::default().fg(color).add_modifier(Modifier::BOLD)));
        spans.push(Span::raw(format!(" ✓ | chain_id: {} | block: {}",
            net.chain_id.map(|id| id.to_string()).unwrap_or("-".to_string()),
            net.block_number.map(|b| b.to_string()).unwrap_or("-".to_string())
        )));
    }
    let bar = Paragraph::new(ratatui::text::Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title("Network"));
    f.render_widget(bar, area);
}

// --- Command Input Renderer ---
fn render_command_input(f: &mut Frame, area: Rect, dash: &DashboardState) {
    use ratatui::widgets::{Paragraph, Block, Borders};
    use ratatui::style::{Style, Color};
    let input = Paragraph::new(dash.input_buffer.clone())
        .block(Block::default().title("Command").borders(Borders::ALL).border_style(Style::default().fg(Color::Magenta)));
    f.render_widget(input, area);
}

// --- Help Overlay Renderer ---
fn render_help_overlay(f: &mut Frame, area: Rect, _dash: &DashboardState) {
    use ratatui::widgets::{Paragraph, Block, Borders, Clear};
    use ratatui::style::{Style, Color, Modifier};
    use ratatui::text::Span;
    let help = "q Quit | Tab Switch Panel | : Command | ↑/↓ Scroll Logs  \nb Bottom | t Top | Esc Exit Input | ? Toggle Help";
    let para = Paragraph::new(ratatui::text::Line::from(vec![Span::raw(help)]))
        .block(Block::default().title("Help").borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let popup = ratatui::layout::Rect {
        x: area.x + area.width / 6,
        y: area.y + area.height / 3,
        width: area.width * 2 / 3,
        height: 7,
    };
    f.render_widget(Clear, popup);
    f.render_widget(para, popup);
}

// --- Utility function to create centered rectangle ---
fn centered_rect(width: u16, height: u16, size: ratatui::layout::Size) -> Rect {
    let x = (size.width - width) / 2;
    let y = (size.height - height) / 2;
    Rect::new(x, y, width, height)
}
