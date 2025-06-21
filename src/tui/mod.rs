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

#[derive(Clone, Debug, Default)]
pub struct DashboardState {
    pub logs: VecDeque<(String, LogLevel)>,
    pub feeds: Vec<FeedStatus>,
    pub networks: Vec<NetworkStatus>,
    pub metrics: MetricsState,
    pub alerts: Vec<String>,
    pub selected_panel: usize,
    pub log_scroll: usize,
    pub filter: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl LogLevel {
    pub fn color(self) -> Color {
        match self {
            LogLevel::Info => Color::Cyan,
            LogLevel::Warn => Color::Yellow,
            LogLevel::Error => Color::Red,
            LogLevel::Debug => Color::Green,
        }
    }
}

// --- Log Channel and Tracing Layer ---
use ratatui::terminal::Frame;
use tracing::{Event as TracingEvent, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::fmt::Layer as FmtLayer;
use tracing_subscriber::prelude::*;

pub fn setup_log_channel_layer(_tx: mpsc::Sender<(String, LogLevel)>) -> FmtLayer<tracing_subscriber::Registry> {
    tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(std::io::stdout)
}

struct LogWriter(mpsc::Sender<(String, LogLevel)>);

impl std::io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let msg = String::from_utf8_lossy(buf).to_string();
        let level = if msg.contains("ERROR") {
            LogLevel::Error
        } else if msg.contains("WARN") {
            LogLevel::Warn
        } else if msg.contains("DEBUG") {
            LogLevel::Debug
        } else {
            LogLevel::Info
        };
        let _ = self.0.try_send((msg, level));
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
    mut log_rx: mpsc::Receiver<(String, LogLevel)>,
) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Spawn a task to receive logs and update dashboard state
    let dashboard_clone = dashboard.clone();
    tokio::spawn(async move {
        while let Some((msg, level)) = log_rx.recv().await {
            let mut dash = dashboard_clone.write().await;
            if dash.logs.len() > 1000 { dash.logs.pop_front(); }
            dash.logs.push_back((msg, level));
        }
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
    use ratatui::widgets::{Tabs};
    use ratatui::style::Style;
    let panel_titles = [
        Line::from("Logs"),
        Line::from("Metrics"),
        Line::from("Feeds"),
        Line::from("Network"),
        Line::from("Alerts"),
    ];
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(200);
    loop {
        let dash = dashboard.read().await.clone();
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .split(size);
            // Top bar
            let tabs = Tabs::new(panel_titles.to_vec())
                .select(dash.selected_panel)
                .block(Block::default().borders(Borders::ALL).title("Omikuji Dashboard"))
                .highlight_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD));
            f.render_widget(tabs, chunks[0]);
            // Main panel
            match dash.selected_panel {
                0 => render_logs(f, chunks[1], &dash),
                1 => render_metrics(f, chunks[1], &dash),
                2 => render_feeds(f, chunks[1], &dash),
                3 => render_network(f, chunks[1], &dash),
                4 => render_alerts(f, chunks[1], &dash),
                _ => {},
            }
            // Footer
            let help = Paragraph::new("[Tab] Switch Panel  [↑/↓] Scroll  [F]ilter  [R]efresh  [Q]uit")
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(help, chunks[2]);
        })?;
        // Handle input
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                let mut dash = dashboard.write().await;
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Tab => { dash.selected_panel = (dash.selected_panel + 1) % panel_titles.len(); },
                    KeyCode::Up => { if dash.log_scroll > 0 { dash.log_scroll -= 1; } },
                    KeyCode::Down => { dash.log_scroll += 1; },
                    KeyCode::Char('f') => { dash.filter = Some(String::new()); },
                    KeyCode::Char('r') => {}, // Manual refresh placeholder
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_rate { last_tick = Instant::now(); }
    }
    Ok(())
}

// --- Panel Renderers ---
fn render_logs(f: &mut Frame, area: Rect, dash: &DashboardState) {
    let log_items: Vec<ListItem> = dash.logs.iter().rev().skip(dash.log_scroll).take(30)
        .map(|(msg, level)| ListItem::new(Line::from(vec![Span::styled(msg, Style::default().fg(level.color()))])))
        .collect();
    let logs = List::new(log_items)
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Logs", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
    f.render_widget(logs, area);
}

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

fn render_alerts(f: &mut Frame, area: Rect, dash: &DashboardState) {
    let items: Vec<ListItem> = dash.alerts.iter().rev().take(10)
        .map(|msg| ListItem::new(Line::from(vec![Span::styled(msg, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))])))
        .collect();
    let alerts = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(Span::styled("Alerts", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))));
    f.render_widget(alerts, area);
}
