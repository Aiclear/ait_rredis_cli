use std::io;
use std::time::Duration;

use crate::redis_client::RedisClient;
use crate::redis_type::RespType;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame, Terminal,
};

pub struct MonitorData {
    used_memory: u64,
    total_memory: u64,
    connected_clients: u64,
    total_connections_received: u64,
    instantaneous_ops_per_sec: u64,
    used_cpu_sys: f64,
    used_cpu_user: f64,
}

impl MonitorData {
    pub fn new() -> Self {
        Self {
            used_memory: 0,
            total_memory: 0,
            connected_clients: 0,
            total_connections_received: 0,
            instantaneous_ops_per_sec: 0,
            used_cpu_sys: 0.0,
            used_cpu_user: 0.0,
        }
    }

    pub fn fetch(redis_client: &mut RedisClient) -> anyhow::Result<Self> {
        let resp = RespType::create_from_command_line("INFO");
        redis_client.write_command(resp)?;
        let response = redis_client.read_resp()?;

        let info_str = Self::extract_info_string(&response);
        Self::parse_info(&info_str)
    }

    fn extract_info_string(resp: &RespType) -> String {
        match resp {
            RespType::BulkStrings(bs) => bs.value().to_string(),
            RespType::SimpleStrings(ss) => ss.value().to_string(),
            _ => String::new(),
        }
    }

    fn parse_info(info: &str) -> anyhow::Result<Self> {
        let mut data = Self::new();

        for line in info.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once(':') {
                match key.trim() {
                    "used_memory" => {
                        data.used_memory = value.trim().parse().unwrap_or(0);
                    }
                    "used_memory_peak" => {
                        data.total_memory = data.total_memory.max(value.trim().parse().unwrap_or(0));
                    }
                    "connected_clients" => {
                        data.connected_clients = value.trim().parse().unwrap_or(0);
                    }
                    "total_connections_received" => {
                        data.total_connections_received = value.trim().parse().unwrap_or(0);
                    }
                    "instantaneous_ops_per_sec" => {
                        data.instantaneous_ops_per_sec = value.trim().parse().unwrap_or(0);
                    }
                    "used_cpu_sys" => {
                        data.used_cpu_sys = value.trim().parse().unwrap_or(0.0);
                    }
                    "used_cpu_user" => {
                        data.used_cpu_user = value.trim().parse().unwrap_or(0.0);
                    }
                    "maxmemory" => {
                        let max: u64 = value.trim().parse().unwrap_or(0);
                        if max > 0 {
                            data.total_memory = max;
                        }
                    }
                    _ => {}
                }
            }
        }

        if data.total_memory == 0 {
            data.total_memory = data.used_memory.max(1024 * 1024 * 1024);
        }

        Ok(data)
    }

    fn memory_usage_percent(&self) -> f64 {
        if self.total_memory == 0 {
            0.0
        } else {
            (self.used_memory as f64 / self.total_memory as f64) * 100.0
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
}

pub struct MonitorApp {
    data: MonitorData,
    should_quit: bool,
}

impl MonitorApp {
    pub fn new() -> Self {
        Self {
            data: MonitorData::new(),
            should_quit: false,
        }
    }

    pub fn update(&mut self, redis_client: &mut RedisClient) {
        if let Ok(data) = MonitorData::fetch(redis_client) {
            self.data = data;
        }
    }

    pub fn handle_event(&mut self) -> anyhow::Result<bool> {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    self.should_quit = true;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

pub fn run_monitor(redis_client: &mut RedisClient) -> anyhow::Result<()> {
    let original_stdout = io::stdout();
    
    enable_raw_mode()?;
    
    let mut stdout = original_stdout;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = MonitorApp::new();

    let res = run_monitor_loop(&mut terminal, &mut app, redis_client);

    disable_raw_mode()?;
    
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    
    terminal.show_cursor()?;

    drop(terminal);

    let mut stdout = io::stdout();
    execute!(stdout, crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;
    execute!(stdout, crossterm::cursor::MoveTo(0, 0))?;

    res
}

fn run_monitor_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut MonitorApp,
    redis_client: &mut RedisClient,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if app.handle_event()? || app.should_quit {
            break;
        }

        app.update(redis_client);
    }
    Ok(())
}

fn ui(f: &mut Frame, app: &MonitorApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Min(0),
        ])
        .split(f.area());

    let title = Paragraph::new("Redis Monitor - Press 'q' or 'Esc' to exit")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(title, chunks[0]);

    render_memory_gauge(f, app, chunks[1]);
    render_connections_gauge(f, app, chunks[2]);
    render_ops_gauge(f, app, chunks[3]);
    render_cpu_gauge(f, app, chunks[4]);
}

fn render_memory_gauge(f: &mut Frame, app: &MonitorApp, area: Rect) {
    let memory_percent = app.data.memory_usage_percent();
    let used = MonitorData::format_bytes(app.data.used_memory);
    let total = MonitorData::format_bytes(app.data.total_memory);

    let block = Block::default()
        .title("Memory Usage")
        .borders(Borders::ALL)
        .title_style(Style::default().fg(Color::Yellow));

    let gauge = Gauge::default()
        .block(block)
        .gauge_style(
            Style::default()
                .fg(Color::Green)
                .bg(Color::Black)
                .add_modifier(Modifier::ITALIC),
        )
        .percent(memory_percent as u16)
        .label(format!("{} / {}", used, total));

    f.render_widget(gauge, area);
}

fn render_connections_gauge(f: &mut Frame, app: &MonitorApp, area: Rect) {
    let clients = app.data.connected_clients;
    let total = app.data.total_connections_received;

    let block = Block::default()
        .title("Connections")
        .borders(Borders::ALL)
        .title_style(Style::default().fg(Color::Yellow));

    let text = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Active Clients: ",
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("{}", clients),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "Total Connections: ",
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("{}", total),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ])
    .block(block);

    f.render_widget(text, area);
}

fn render_ops_gauge(f: &mut Frame, app: &MonitorApp, area: Rect) {
    let ops = app.data.instantaneous_ops_per_sec;

    let block = Block::default()
        .title("Operations per Second")
        .borders(Borders::ALL)
        .title_style(Style::default().fg(Color::Yellow));

    let text = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Ops/sec: ",
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("{}", ops),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ]),
    ])
    .block(block);

    f.render_widget(text, area);
}

fn render_cpu_gauge(f: &mut Frame, app: &MonitorApp, area: Rect) {
    let cpu_sys = app.data.used_cpu_sys;
    let cpu_user = app.data.used_cpu_user;

    let block = Block::default()
        .title("CPU Usage")
        .borders(Borders::ALL)
        .title_style(Style::default().fg(Color::Yellow));

    let text = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "System CPU: ",
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("{:.2}%", cpu_sys),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "User CPU: ",
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("{:.2}%", cpu_user),
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            ),
        ]),
    ])
    .block(block);

    f.render_widget(text, area);
}
