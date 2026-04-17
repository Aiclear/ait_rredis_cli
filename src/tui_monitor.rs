use std::sync::{Arc, Mutex};
use std::time::Duration;
use chrono::Local;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Line, Text},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame, Terminal,
};

use crate::redis_type::RespType;

#[derive(Debug, Clone, Default)]
pub struct RedisMetrics {
    pub used_memory: u64,
    pub used_memory_human: String,
    pub used_memory_peak: u64,
    pub used_memory_peak_human: String,
    pub used_memory_rss: u64,
    pub used_memory_rss_human: String,
    pub total_system_memory: u64,
    pub total_system_memory_human: String,
    pub connected_clients: u64,
    pub client_longest_output_list: u64,
    pub client_biggest_input_buf: u64,
    pub blocked_clients: u64,
    pub tracking_clients: u64,
    pub total_commands_processed: u64,
    pub instantaneous_ops_per_sec: u64,
    pub total_net_input_bytes: u64,
    pub total_net_output_bytes: u64,
    pub instantaneous_input_kbps: f64,
    pub instantaneous_output_kbps: f64,
    pub keyspace_hits: u64,
    pub keyspace_misses: u64,
    pub keyspace_hit_rate: f64,
    pub uptime_in_seconds: u64,
    pub uptime_in_days: u64,
    pub connected_slaves: u64,
    pub role: String,
    pub latest_time: String,
}

impl RedisMetrics {
    pub fn parse_info_response(resp: &RespType) -> Option<Self> {
        let mut metrics = RedisMetrics::default();
        metrics.latest_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        if let Some(bs) = resp.as_bulk_string() {
            let info_str = bs.value();
            for line in info_str.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    match key {
                        "used_memory" => metrics.used_memory = value.parse().unwrap_or(0),
                        "used_memory_human" => metrics.used_memory_human = value.to_string(),
                        "used_memory_peak" => metrics.used_memory_peak = value.parse().unwrap_or(0),
                        "used_memory_peak_human" => metrics.used_memory_peak_human = value.to_string(),
                        "used_memory_rss" => metrics.used_memory_rss = value.parse().unwrap_or(0),
                        "used_memory_rss_human" => metrics.used_memory_rss_human = value.to_string(),
                        "total_system_memory" => metrics.total_system_memory = value.parse().unwrap_or(0),
                        "total_system_memory_human" => metrics.total_system_memory_human = value.to_string(),
                        "connected_clients" => metrics.connected_clients = value.parse().unwrap_or(0),
                        "client_longest_output_list" => metrics.client_longest_output_list = value.parse().unwrap_or(0),
                        "client_biggest_input_buf" => metrics.client_biggest_input_buf = value.parse().unwrap_or(0),
                        "blocked_clients" => metrics.blocked_clients = value.parse().unwrap_or(0),
                        "tracking_clients" => metrics.tracking_clients = value.parse().unwrap_or(0),
                        "total_commands_processed" => metrics.total_commands_processed = value.parse().unwrap_or(0),
                        "instantaneous_ops_per_sec" => metrics.instantaneous_ops_per_sec = value.parse().unwrap_or(0),
                        "total_net_input_bytes" => metrics.total_net_input_bytes = value.parse().unwrap_or(0),
                        "total_net_output_bytes" => metrics.total_net_output_bytes = value.parse().unwrap_or(0),
                        "instantaneous_input_kbps" => metrics.instantaneous_input_kbps = value.parse().unwrap_or(0.0),
                        "instantaneous_output_kbps" => metrics.instantaneous_output_kbps = value.parse().unwrap_or(0.0),
                        "keyspace_hits" => metrics.keyspace_hits = value.parse().unwrap_or(0),
                        "keyspace_misses" => metrics.keyspace_misses = value.parse().unwrap_or(0),
                        "uptime_in_seconds" => metrics.uptime_in_seconds = value.parse().unwrap_or(0),
                        "uptime_in_days" => metrics.uptime_in_days = value.parse().unwrap_or(0),
                        "connected_slaves" => metrics.connected_slaves = value.parse().unwrap_or(0),
                        "role" => metrics.role = value.to_string(),
                        _ => {}
                    }
                }
            }

            // Calculate keyspace hit rate
            let total_lookups = metrics.keyspace_hits + metrics.keyspace_misses;
            if total_lookups > 0 {
                metrics.keyspace_hit_rate = (metrics.keyspace_hits as f64 / total_lookups as f64) * 100.0;
            } else {
                metrics.keyspace_hit_rate = 100.0;
            }

            Some(metrics)
        } else {
            None
        }
    }

    pub fn memory_usage_percent(&self) -> f64 {
        if self.total_system_memory > 0 {
            (self.used_memory as f64 / self.total_system_memory as f64) * 100.0
        } else {
            0.0
        }
    }

    pub fn format_uptime(&self) -> String {
        let days = self.uptime_in_days;
        let hours = (self.uptime_in_seconds % 86400) / 3600;
        let minutes = (self.uptime_in_seconds % 3600) / 60;
        let seconds = self.uptime_in_seconds % 60;

        if days > 0 {
            format!("{} days, {}:{:02}:{:02}", days, hours, minutes, seconds)
        } else {
            format!("{}:{:02}:{:02}", hours, minutes, seconds)
        }
    }
}

pub struct MonitorApp {
    metrics: Arc<Mutex<RedisMetrics>>,
    should_quit: bool,
}

impl MonitorApp {
    pub fn new(metrics: Arc<Mutex<RedisMetrics>>) -> Self {
        Self {
            metrics,
            should_quit: false,
        }
    }

    pub fn on_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }
}

pub async fn run_monitor(metrics: Arc<Mutex<RedisMetrics>>) -> anyhow::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = MonitorApp::new(metrics.clone());

    // Main loop
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = std::time::Instant::now();
    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key.code);
            }
        }
        
        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
        }

        if app.should_quit() {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut Frame, app: &MonitorApp) {
    let metrics = app.metrics.lock().unwrap();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Percentage(40),
                Constraint::Percentage(40),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(f.size());

    // Title
    let title = Paragraph::new(Text::from(Line::from(vec![
        Span::styled("Redis Monitor - ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(&metrics.latest_time, Style::default().fg(Color::Cyan)),
    ])))
    .block(Block::default().borders(Borders::ALL).title("Status"))
    .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(title, chunks[0]);

    // Memory section
    let memory_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
        .split(chunks[1]);

    // Memory usage gauge and details
    let memory_usage_percent = metrics.memory_usage_percent();
    let memory_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Memory Usage"))
        .gauge_style(Style::default().fg(if memory_usage_percent > 80.0 { Color::Red } else { Color::Green }))
        .percent(memory_usage_percent as u16)
        .label(Span::styled(
            format!("{} / {} ({:.1}%)", 
                metrics.used_memory_human, 
                metrics.total_system_memory_human,
                memory_usage_percent),
            Style::default().add_modifier(Modifier::BOLD),
        ));
    f.render_widget(memory_gauge, memory_chunks[0]);

    // Memory details
    let memory_details = vec![
        ListItem::new(Line::from(vec![
            Span::styled("Used Memory: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(&metrics.used_memory_human, Style::default().fg(Color::Yellow)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Peak Memory: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(&metrics.used_memory_peak_human, Style::default().fg(Color::Magenta)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("RSS Memory: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(&metrics.used_memory_rss_human, Style::default().fg(Color::Cyan)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("System Memory: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(&metrics.total_system_memory_human, Style::default().fg(Color::White)),
        ])),
    ];
    let memory_list = List::new(memory_details)
        .block(Block::default().borders(Borders::ALL).title("Memory Details"));
    f.render_widget(memory_list, memory_chunks[1]);

    // Connections and stats section
    let stats_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[2]);

    // Connections
    let connections = vec![
        ListItem::new(Line::from(vec![
            Span::styled("Connected Clients: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(metrics.connected_clients.to_string(), Style::default().fg(Color::Green)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Blocked Clients: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(metrics.blocked_clients.to_string(), Style::default().fg(Color::Yellow)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Tracking Clients: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(metrics.tracking_clients.to_string(), Style::default().fg(Color::Cyan)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Longest Output List: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(metrics.client_longest_output_list.to_string(), Style::default().fg(Color::White)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Biggest Input Buf: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(metrics.client_biggest_input_buf.to_string(), Style::default().fg(Color::White)),
        ])),
    ];
    let connections_list = List::new(connections)
        .block(Block::default().borders(Borders::ALL).title("Connections"));
    f.render_widget(connections_list, stats_chunks[0]);

    // Performance stats
    let stats = vec![
        ListItem::new(Line::from(vec![
            Span::styled("Ops/sec: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(metrics.instantaneous_ops_per_sec.to_string(), Style::default().fg(Color::Green)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Total Commands: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(metrics.total_commands_processed.to_string(), Style::default().fg(Color::Yellow)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Keyspace Hit Rate: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:.2}%", metrics.keyspace_hit_rate), 
                Style::default().fg(if metrics.keyspace_hit_rate > 90.0 { Color::Green } else { Color::Yellow })),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Input KB/s: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:.2}", metrics.instantaneous_input_kbps), Style::default().fg(Color::Cyan)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Output KB/s: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:.2}", metrics.instantaneous_output_kbps), Style::default().fg(Color::Magenta)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Uptime: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(metrics.format_uptime(), Style::default().fg(Color::White)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Role: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(&metrics.role, Style::default().fg(Color::Green)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Connected Slaves: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(metrics.connected_slaves.to_string(), Style::default().fg(Color::White)),
        ])),
    ];
    let stats_list = List::new(stats)
        .block(Block::default().borders(Borders::ALL).title("Performance Stats"));
    f.render_widget(stats_list, stats_chunks[1]);

    // Footer
    let footer = Paragraph::new(Text::from(Line::from(vec![
        Span::styled("Press ", Style::default().fg(Color::Gray)),
        Span::styled("q", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(" or ", Style::default().fg(Color::Gray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(" to quit monitor", Style::default().fg(Color::Gray)),
    ])))
    .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(footer, chunks[3]);
}
