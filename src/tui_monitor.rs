use std::time::{Duration, Instant};
use std::io;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use crate::redis_client::RedisClient;
use crate::redis_type::RespType;

#[derive(Debug, Default)]
pub struct MonitorData {
    pub used_memory: u64,
    pub used_memory_peak: u64,
    pub connected_clients: u64,
    pub total_connections_received: u64,
    pub instantaneous_ops_per_sec: u64,
    pub cpu_used_sys: f64,
    pub cpu_used_user: f64,
    pub keyspace_hits: u64,
    pub keyspace_misses: u64,
    pub memory_history: Vec<u64>,
    pub cpu_history: Vec<f64>,
    pub ops_history: Vec<u64>,
}

impl MonitorData {
    pub fn new() -> Self {
        Self {
            memory_history: Vec::with_capacity(60),
            cpu_history: Vec::with_capacity(60),
            ops_history: Vec::with_capacity(60),
            ..Default::default()
        }
    }

    pub fn update(&mut self, info: &str) {
        for line in info.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() == 2 {
                match parts[0] {
                    "used_memory" => self.used_memory = parts[1].parse().unwrap_or(0),
                    "used_memory_peak" => self.used_memory_peak = parts[1].parse().unwrap_or(0),
                    "connected_clients" => self.connected_clients = parts[1].parse().unwrap_or(0),
                    "total_connections_received" => self.total_connections_received = parts[1].parse().unwrap_or(0),
                    "instantaneous_ops_per_sec" => self.instantaneous_ops_per_sec = parts[1].parse().unwrap_or(0),
                    "used_cpu_sys" => self.cpu_used_sys = parts[1].parse().unwrap_or(0.0),
                    "used_cpu_user" => self.cpu_used_user = parts[1].parse().unwrap_or(0.0),
                    "keyspace_hits" => self.keyspace_hits = parts[1].parse().unwrap_or(0),
                    "keyspace_misses" => self.keyspace_misses = parts[1].parse().unwrap_or(0),
                    _ => {}
                }
            }
        }

        if self.memory_history.len() >= 60 {
            self.memory_history.remove(0);
        }
        self.memory_history.push(self.used_memory);

        if self.cpu_history.len() >= 60 {
            self.cpu_history.remove(0);
        }
        self.cpu_history.push(self.cpu_used_sys + self.cpu_used_user);

        if self.ops_history.len() >= 60 {
            self.ops_history.remove(0);
        }
        self.ops_history.push(self.instantaneous_ops_per_sec);
    }
}

pub fn run_monitor(redis_client: &mut RedisClient) -> anyhow::Result<()> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut monitor_data = MonitorData::new();
    let mut last_update = Instant::now();
    let update_interval = Duration::from_secs(1);

    loop {
        if last_update.elapsed() >= update_interval {
            let info_cmd = RespType::create_from_command_line("info all");
            redis_client.write_command(info_cmd)?;
            let response = redis_client.read_resp()?;
            monitor_data.update(&response.to_string());
            last_update = Instant::now();
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(0),
                ].as_ref())
                .split(f.size());

            let memory_percent = if monitor_data.used_memory_peak > 0 {
                (monitor_data.used_memory as f64 / monitor_data.used_memory_peak as f64) * 100.0
            } else {
                0.0
            };

            let memory_gauge = Gauge::default()
                .block(Block::default().title("Memory Usage").borders(Borders::ALL))
                .gauge_style(Style::default().fg(Color::Blue))
                .percent(memory_percent as u16)
                .label(format!("{} / {} bytes", monitor_data.used_memory, monitor_data.used_memory_peak));
            f.render_widget(memory_gauge, chunks[0]);

            let cpu_percent = ((monitor_data.cpu_used_sys + monitor_data.cpu_used_user) * 10.0).min(100.0);
            let cpu_gauge = Gauge::default()
                .block(Block::default().title("CPU Usage").borders(Borders::ALL))
                .gauge_style(Style::default().fg(Color::Green))
                .percent(cpu_percent as u16)
                .label(format!("sys: {:.2}%, user: {:.2}%", monitor_data.cpu_used_sys, monitor_data.cpu_used_user));
            f.render_widget(cpu_gauge, chunks[1]);

            let connections_text = Line::from(vec![
                Span::raw("Connected Clients: "),
                Span::styled(monitor_data.connected_clients.to_string(), Style::default().fg(Color::Yellow)),
                Span::raw(" | Total Connections: "),
                Span::styled(monitor_data.total_connections_received.to_string(), Style::default().fg(Color::Yellow)),
            ]);
            let connections = Paragraph::new(connections_text)
                .block(Block::default().title("Connections").borders(Borders::ALL));
            f.render_widget(connections, chunks[2]);

            let ops_text = Line::from(vec![
                Span::raw("Ops/sec: "),
                Span::styled(monitor_data.instantaneous_ops_per_sec.to_string(), Style::default().fg(Color::Cyan)),
                Span::raw(" | Hits: "),
                Span::styled(monitor_data.keyspace_hits.to_string(), Style::default().fg(Color::Green)),
                Span::raw(" | Misses: "),
                Span::styled(monitor_data.keyspace_misses.to_string(), Style::default().fg(Color::Red)),
            ]);
            let ops = Paragraph::new(ops_text)
                .block(Block::default().title("Performance").borders(Borders::ALL));
            f.render_widget(ops, chunks[3]);

            let bottom_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(33),
                    Constraint::Percentage(33),
                    Constraint::Percentage(34),
                ].as_ref())
                .split(chunks[4]);

            let memory_sparkline = Sparkline::default()
                .block(Block::default().title("Memory History").borders(Borders::ALL))
                .data(&monitor_data.memory_history)
                .style(Style::default().fg(Color::Blue));
            f.render_widget(memory_sparkline, bottom_chunks[0]);

            let cpu_data: Vec<u64> = monitor_data.cpu_history.iter().map(|&x| (x * 10.0) as u64).collect();
            let cpu_sparkline = Sparkline::default()
                .block(Block::default().title("CPU History").borders(Borders::ALL))
                .data(&cpu_data)
                .style(Style::default().fg(Color::Green));
            f.render_widget(cpu_sparkline, bottom_chunks[1]);

            let ops_sparkline = Sparkline::default()
                .block(Block::default().title("Ops History").borders(Borders::ALL))
                .data(&monitor_data.ops_history)
                .style(Style::default().fg(Color::Cyan));
            f.render_widget(ops_sparkline, bottom_chunks[2]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break;
                }
            }
        }
    }

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
