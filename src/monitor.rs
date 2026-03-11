use std::time::{Duration, Instant};
use std::io;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, LineGauge, List, ListItem, Paragraph, Sparkline},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use crate::redis_client::RedisClient;
use crate::redis_type::RespType;

#[derive(Debug, Default, Clone)]
pub struct RedisMetrics {
    pub used_memory: u64,
    pub used_memory_peak: u64,
    pub used_memory_rss: u64,
    pub connected_clients: u32,
    pub total_connections_received: u64,
    pub total_commands_processed: u64,
    pub instantaneous_ops_per_sec: u32,
    pub keyspace_hits: u64,
    pub keyspace_misses: u64,
    pub used_cpu_sys: f64,
    pub used_cpu_user: f64,
    pub used_cpu_sys_children: f64,
    pub used_cpu_user_children: f64,
    pub total_system_memory: u64,
    pub db_keys: Vec<(String, u32)>,
}

#[derive(Debug, Clone)]
pub struct MetricsHistory {
    pub cpu_history: Vec<f64>,
    pub memory_history: Vec<u64>,
    pub ops_history: Vec<u64>,
    pub max_history_len: usize,
}

impl Default for MetricsHistory {
    fn default() -> Self {
        MetricsHistory {
            cpu_history: Vec::with_capacity(100),
            memory_history: Vec::with_capacity(100),
            ops_history: Vec::with_capacity(100),
            max_history_len: 100,
        }
    }
}

impl MetricsHistory {
    pub fn push(&mut self, metrics: &RedisMetrics) {
        let total_cpu = metrics.used_cpu_sys + metrics.used_cpu_user;
        
        self.cpu_history.push(total_cpu);
        if self.cpu_history.len() > self.max_history_len {
            self.cpu_history.remove(0);
        }

        self.memory_history.push(metrics.used_memory);
        if self.memory_history.len() > self.max_history_len {
            self.memory_history.remove(0);
        }

        self.ops_history.push(metrics.total_commands_processed);
        if self.ops_history.len() > self.max_history_len {
            self.ops_history.remove(0);
        }
    }
}

pub struct Monitor {
    client: Option<RedisClient>,
    metrics: RedisMetrics,
    history: MetricsHistory,
    last_update: Instant,
    update_interval: Duration,
}

impl Monitor {
    pub fn new(client: RedisClient) -> Self {
        Monitor {
            client: Some(client),
            metrics: RedisMetrics::default(),
            history: MetricsHistory::default(),
            last_update: Instant::now(),
            update_interval: Duration::from_millis(500),
        }
    }

    pub fn fetch_metrics(&mut self) -> io::Result<()> {
        let client = self.client.as_mut().ok_or_else(|| 
            io::Error::other("Client not available")
        )?;
        
        let command = RespType::create_from_command_line("INFO ALL");
        client.write_command(command)
            .map_err(io::Error::other)?;
        
        let response = client.read_resp()
            .map_err(io::Error::other)?;
        
        self.parse_info_response(&response.to_string());
        self.history.push(&self.metrics);
        
        Ok(())
    }

    fn parse_info_response(&mut self, info: &str) {
        let mut metrics = RedisMetrics::default();
        
        for line in info.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            if let Some((key, value)) = line.split_once(':') {
                match key {
                    "used_memory" => metrics.used_memory = value.parse().unwrap_or(0),
                    "used_memory_peak" => metrics.used_memory_peak = value.parse().unwrap_or(0),
                    "used_memory_rss" => metrics.used_memory_rss = value.parse().unwrap_or(0),
                    "connected_clients" => metrics.connected_clients = value.parse().unwrap_or(0),
                    "total_connections_received" => metrics.total_connections_received = value.parse().unwrap_or(0),
                    "total_commands_processed" => metrics.total_commands_processed = value.parse().unwrap_or(0),
                    "instantaneous_ops_per_sec" => metrics.instantaneous_ops_per_sec = value.parse().unwrap_or(0),
                    "keyspace_hits" => metrics.keyspace_hits = value.parse().unwrap_or(0),
                    "keyspace_misses" => metrics.keyspace_misses = value.parse().unwrap_or(0),
                    "used_cpu_sys" => metrics.used_cpu_sys = value.parse().unwrap_or(0.0),
                    "used_cpu_user" => metrics.used_cpu_user = value.parse().unwrap_or(0.0),
                    "used_cpu_sys_children" => metrics.used_cpu_sys_children = value.parse().unwrap_or(0.0),
                    "used_cpu_user_children" => metrics.used_cpu_user_children = value.parse().unwrap_or(0.0),
                    "total_system_memory" => metrics.total_system_memory = value.parse().unwrap_or(0),
                    _ => {
                        if key.starts_with("db")
                            && let Some(keys_part) = value.split(',').next()
                            && let Some((_, keys)) = keys_part.split_once('=')
                            && let Ok(count) = keys.parse::<u32>()
                        {
                            metrics.db_keys.push((key.to_string(), count));
                        }
                    }
                }
            }
        }
        
        self.metrics = metrics;
    }

    pub fn run(&mut self) -> io::Result<RedisClient> {
        enable_raw_mode()?;
        io::stdout().execute(EnterAlternateScreen)?;
        
        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let result = self.run_event_loop(&mut terminal);
        
        disable_raw_mode()?;
        io::stdout().execute(LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result?;
        
        Ok(self.client.take().unwrap())
    }

    fn run_event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            if self.last_update.elapsed() >= self.update_interval {
                self.fetch_metrics()?;
                self.last_update = Instant::now();
            }

            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Length(8),
                        Constraint::Length(8),
                        Constraint::Min(8),
                    ].as_ref())
                    .split(f.size());

                let title = Paragraph::new(Line::from(vec![
                    Span::styled(
                        "Redis Monitor - Press 'q' or 'ESC' to exit",
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    ),
                ]))
                .block(Block::default().borders(Borders::ALL))
                .alignment(ratatui::layout::Alignment::Center);
                f.render_widget(title, chunks[0]);

                let top_row = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(33),
                        Constraint::Percentage(33),
                        Constraint::Percentage(34),
                    ].as_ref())
                    .split(chunks[1]);

                let memory_percent = if self.metrics.total_system_memory > 0 {
                    (self.metrics.used_memory as f64 / self.metrics.total_system_memory as f64) * 100.0
                } else {
                    0.0
                };

                let memory_gauge = Gauge::default()
                    .block(Block::default().title("Memory Usage").borders(Borders::ALL))
                    .gauge_style(Style::default().fg(Color::Cyan))
                    .percent(memory_percent as u16)
                    .label(format!(
                        "Used: {} / {} ({:.1}%)",
                        format_bytes(self.metrics.used_memory),
                        format_bytes(self.metrics.total_system_memory),
                        memory_percent
                    ));
                f.render_widget(memory_gauge, top_row[0]);

                let connections_gauge = LineGauge::default()
                    .block(Block::default().title("Connections").borders(Borders::ALL))
                    .gauge_style(Style::default().fg(Color::Magenta))
                    .ratio(self.metrics.connected_clients as f64 / 10000.0)
                    .label(format!("Active: {}", self.metrics.connected_clients));
                f.render_widget(connections_gauge, top_row[1]);

                let cpu_percent = (self.metrics.used_cpu_sys + self.metrics.used_cpu_user).min(100.0);
                let cpu_gauge = Gauge::default()
                    .block(Block::default().title("CPU Usage").borders(Borders::ALL))
                    .gauge_style(Style::default().fg(Color::Yellow))
                    .percent(cpu_percent as u16)
                    .label(format!(
                        "Sys: {:.2}% | User: {:.2}%",
                        self.metrics.used_cpu_sys,
                        self.metrics.used_cpu_user
                    ));
                f.render_widget(cpu_gauge, top_row[2]);

                let charts_row = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(50),
                        Constraint::Percentage(50),
                    ].as_ref())
                    .split(chunks[2]);

                let memory_data: Vec<u64> = self.history.memory_history
                    .iter()
                    .map(|&m| m / 1024 / 1024)
                    .collect();
                let memory_spark = Sparkline::default()
                    .block(Block::default().title("Memory Trend (MB)").borders(Borders::ALL))
                    .data(&memory_data)
                    .style(Style::default().fg(Color::Cyan));
                f.render_widget(memory_spark, charts_row[0]);

                let ops_data: Vec<u64> = self.history.ops_history
                    .windows(2)
                    .map(|w| w[1].saturating_sub(w[0]))
                    .collect();
                let ops_spark = Sparkline::default()
                    .block(Block::default().title("Operations/sec Trend").borders(Borders::ALL))
                    .data(&ops_data)
                    .style(Style::default().fg(Color::Green));
                f.render_widget(ops_spark, charts_row[1]);

                let bottom_row = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(40),
                        Constraint::Percentage(30),
                        Constraint::Percentage(30),
                    ].as_ref())
                    .split(chunks[3]);

                let mut info_items = vec![
                    ListItem::new(format!("Peak Memory: {}", format_bytes(self.metrics.used_memory_peak))),
                    ListItem::new(format!("RSS Memory: {}", format_bytes(self.metrics.used_memory_rss))),
                    ListItem::new(format!("Total Connections: {}", self.metrics.total_connections_received)),
                    ListItem::new(format!("Total Commands: {}", self.metrics.total_commands_processed)),
                    ListItem::new(format!("Ops/sec: {}", self.metrics.instantaneous_ops_per_sec)),
                ];
                
                let hit_rate = if self.metrics.keyspace_hits + self.metrics.keyspace_misses > 0 {
                    (self.metrics.keyspace_hits as f64 / (self.metrics.keyspace_hits + self.metrics.keyspace_misses) as f64) * 100.0
                } else {
                    0.0
                };
                info_items.push(ListItem::new(format!("Cache Hit Rate: {:.1}%", hit_rate)));

                let info_list = List::new(info_items)
                    .block(Block::default().title("Server Info").borders(Borders::ALL))
                    .style(Style::default().fg(Color::White));
                f.render_widget(info_list, bottom_row[0]);

                let mut db_items = Vec::new();
                for (db, keys) in &self.metrics.db_keys {
                    db_items.push(ListItem::new(format!("{}: {} keys", db, keys)));
                }
                let db_list = List::new(db_items)
                    .block(Block::default().title("Keyspace").borders(Borders::ALL))
                    .style(Style::default().fg(Color::White));
                f.render_widget(db_list, bottom_row[1]);

                let cpu_items = vec![
                    ListItem::new(format!("Sys CPU: {:.2}%", self.metrics.used_cpu_sys)),
                    ListItem::new(format!("User CPU: {:.2}%", self.metrics.used_cpu_user)),
                    ListItem::new(format!("Sys Children: {:.2}%", self.metrics.used_cpu_sys_children)),
                    ListItem::new(format!("User Children: {:.2}%", self.metrics.used_cpu_user_children)),
                ];
                let cpu_list = List::new(cpu_items)
                    .block(Block::default().title("CPU Breakdown").borders(Borders::ALL))
                    .style(Style::default().fg(Color::White));
                f.render_widget(cpu_list, bottom_row[2]);
            })?;

            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
