use std::{
    io,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Chart, Dataset, Paragraph, Row, Sparkline, Table},
    Frame, Terminal,
};

use crate::{
    redis_client::{RedisAddress, RedisClient},
    redis_type::RespType,
};

pub struct MonitorApp {
    memory_usage: Vec<u64>,
    connected_clients: Vec<u64>,
    cpu_usage: Vec<f64>,
    max_data_points: usize,
    last_update: Instant,
    update_interval: Duration,
    redis_info: RedisInfo,
}

#[derive(Default)]
struct RedisInfo {
    used_memory: u64,
    used_memory_human: String,
    connected_clients: u64,
    total_connections_received: u64,
    used_cpu_sys: f64,
    used_cpu_user: f64,
    keyspace_hits: u64,
    keyspace_misses: u64,
    uptime_in_seconds: u64,
    redis_version: String,
}

impl MonitorApp {
    pub fn new() -> Self {
        Self {
            memory_usage: Vec::with_capacity(60),
            connected_clients: Vec::with_capacity(60),
            cpu_usage: Vec::with_capacity(60),
            max_data_points: 60,
            last_update: Instant::now(),
            update_interval: Duration::from_secs(1),
            redis_info: RedisInfo::default(),
        }
    }

    pub fn run(&mut self, redis_address: &RedisAddress) -> anyhow::Result<()> {
        let mut client = RedisClient::connect(redis_address.clone())?;

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let res = self.run_loop(&mut terminal, &mut client);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        res
    }

    fn run_loop<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        client: &mut RedisClient,
    ) -> anyhow::Result<()> {
        let mut last_draw = Instant::now();
        let draw_interval = Duration::from_millis(100);

        loop {
            if self.last_update.elapsed() >= self.update_interval {
                self.update_metrics(client)?;
                self.last_update = Instant::now();
            }

            if last_draw.elapsed() >= draw_interval {
                terminal.draw(|f| self.ui(f))?;
                last_draw = Instant::now();
            }

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        _ => {}
                    }
                }
            }
        }
    }

    fn update_metrics(&mut self, client: &mut RedisClient) -> anyhow::Result<()> {
        let info = self.fetch_info(client)?;
        self.parse_info(&info);

        self.memory_usage.push(self.redis_info.used_memory);
        self.connected_clients.push(self.redis_info.connected_clients);
        
        let total_cpu = self.redis_info.used_cpu_sys + self.redis_info.used_cpu_user;
        self.cpu_usage.push(total_cpu);

        if self.memory_usage.len() > self.max_data_points {
            self.memory_usage.remove(0);
        }
        if self.connected_clients.len() > self.max_data_points {
            self.connected_clients.remove(0);
        }
        if self.cpu_usage.len() > self.max_data_points {
            self.cpu_usage.remove(0);
        }

        Ok(())
    }

    fn fetch_info(&mut self, client: &mut RedisClient) -> anyhow::Result<String> {
        let cmd = "INFO";
        let resp_type = RespType::create_from_command_line(cmd);
        client.write_command(resp_type)?;
        let response = client.read_resp()?;

        match response {
            RespType::BulkStrings(bs) => Ok(bs.value),
            _ => Ok(String::new()),
        }
    }

    fn parse_info(&mut self, info: &str) {
        for line in info.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once(':') {
                match key {
                    "used_memory" => {
                        self.redis_info.used_memory = value.parse().unwrap_or(0)
                    }
                    "used_memory_human" => {
                        self.redis_info.used_memory_human = value.to_string()
                    }
                    "connected_clients" => {
                        self.redis_info.connected_clients = value.parse().unwrap_or(0)
                    }
                    "total_connections_received" => {
                        self.redis_info.total_connections_received = value.parse().unwrap_or(0)
                    }
                    "used_cpu_sys" => {
                        self.redis_info.used_cpu_sys = value.parse().unwrap_or(0.0)
                    }
                    "used_cpu_user" => {
                        self.redis_info.used_cpu_user = value.parse().unwrap_or(0.0)
                    }
                    "keyspace_hits" => {
                        self.redis_info.keyspace_hits = value.parse().unwrap_or(0)
                    }
                    "keyspace_misses" => {
                        self.redis_info.keyspace_misses = value.parse().unwrap_or(0)
                    }
                    "uptime_in_seconds" => {
                        self.redis_info.uptime_in_seconds = value.parse().unwrap_or(0)
                    }
                    "redis_version" => {
                        self.redis_info.redis_version = value.to_string()
                    }
                    _ => {}
                }
            }
        }
    }

    fn ui(&self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Min(5),
            ])
            .split(frame.area());

        self.render_header(frame, chunks[0]);
        self.render_memory_chart(frame, chunks[1]);
        self.render_clients_chart(frame, chunks[2]);
        self.render_cpu_chart(frame, chunks[3]);
        self.render_stats_table(frame, chunks[4]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let header = Paragraph::new(
            Text::from(vec![
                Line::from(vec![
                    Span::styled(
                        "Redis Monitor",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" - Press 'q' or 'ESC' to exit"),
                ]),
                Line::from(format!(
                    "Redis Version: {} | Uptime: {}s",
                    self.redis_info.redis_version, self.redis_info.uptime_in_seconds
                )),
            ])
        )
        .block(Block::default().borders(Borders::BOTTOM));

        frame.render_widget(header, area);
    }

    fn render_memory_chart(&self, frame: &mut Frame, area: Rect) {
        let data: Vec<(f64, f64)> = self
            .memory_usage
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as f64, v as f64 / (1024.0 * 1024.0)))
            .collect();

        let datasets = vec![Dataset::default()
            .name("Memory Usage (MB)")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Yellow))
            .graph_type(ratatui::widgets::GraphType::Line)
            .data(&data)];

        let max_memory = self
            .memory_usage
            .iter()
            .max()
            .copied()
            .unwrap_or(1) as f64
            / (1024.0 * 1024.0);

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .title(format!(
                        "Memory Usage - {}",
                        self.redis_info.used_memory_human
                    ))
                    .borders(Borders::ALL),
            )
            .x_axis(
                ratatui::widgets::Axis::default()
                    .bounds([0.0, self.max_data_points as f64]),
            )
            .y_axis(
                ratatui::widgets::Axis::default()
                    .bounds([0.0, max_memory.max(1.0)])
                    .labels(vec![
                        Span::raw("0"),
                        Span::raw(format!("{:.1}", max_memory / 2.0)),
                        Span::raw(format!("{:.1}", max_memory)),
                    ]),
            );

        frame.render_widget(chart, area);
    }

    fn render_clients_chart(&self, frame: &mut Frame, area: Rect) {
        let sparkline = Sparkline::default()
            .block(
                Block::default()
                    .title(format!(
                        "Connected Clients - {}",
                        self.redis_info.connected_clients
                    ))
                    .borders(Borders::ALL),
            )
            .data(&self.connected_clients)
            .style(Style::default().fg(Color::Green));

        frame.render_widget(sparkline, area);
    }

    fn render_cpu_chart(&self, frame: &mut Frame, area: Rect) {
        let data: Vec<(f64, f64)> = self
            .cpu_usage
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as f64, v))
            .collect();

        let datasets = vec![Dataset::default()
            .name("CPU Usage")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Red))
            .graph_type(ratatui::widgets::GraphType::Line)
            .data(&data)];

        let max_cpu: f64 = self.cpu_usage.iter().fold(0.0_f64, |a, &b| a.max(b));

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .title(format!(
                        "CPU Usage - Sys: {:.2}s, User: {:.2}s",
                        self.redis_info.used_cpu_sys, self.redis_info.used_cpu_user
                    ))
                    .borders(Borders::ALL),
            )
            .x_axis(
                ratatui::widgets::Axis::default()
                    .bounds([0.0, self.max_data_points as f64]),
            )
            .y_axis(
                ratatui::widgets::Axis::default()
                    .bounds([0.0, max_cpu.max(1.0)])
                    .labels(vec![
                        Span::raw("0"),
                        Span::raw(format!("{:.1}", max_cpu / 2.0)),
                        Span::raw(format!("{:.1}", max_cpu)),
                    ]),
            );

        frame.render_widget(chart, area);
    }

    fn render_stats_table(&self, frame: &mut Frame, area: Rect) {
        let hit_rate = if self.redis_info.keyspace_hits + self.redis_info.keyspace_misses > 0 {
            (self.redis_info.keyspace_hits as f64 * 100.0)
                / (self.redis_info.keyspace_hits + self.redis_info.keyspace_misses) as f64
        } else {
            0.0
        };

        let rows = vec![
            Row::new(vec![
                Cell::from("Keyspace Hits"),
                Cell::from(self.redis_info.keyspace_hits.to_string()),
            ]),
            Row::new(vec![
                Cell::from("Keyspace Misses"),
                Cell::from(self.redis_info.keyspace_misses.to_string()),
            ]),
            Row::new(vec![
                Cell::from("Hit Rate"),
                Cell::from(format!("{:.2}%", hit_rate)),
            ]),
            Row::new(vec![
                Cell::from("Total Connections"),
                Cell::from(self.redis_info.total_connections_received.to_string()),
            ]),
        ];

        let table = Table::new(
            rows,
            &[Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .header(
            Row::new(vec!["Metric", "Value"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().title("Statistics").borders(Borders::ALL));

        frame.render_widget(table, area);
    }
}
