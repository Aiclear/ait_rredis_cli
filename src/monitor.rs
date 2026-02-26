use std::{
    io,
    time::Duration,
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Gauge, Paragraph},
    Terminal,
};

use crate::redis_client::RedisClient;

#[derive(Default)]
pub struct RedisStats {
    pub used_memory_human: String,
    pub used_memory: u64,
    pub maxmemory: u64,
    pub connected_clients: u32,
    pub total_connections_received: u32,
    pub connected_slaves: u32,
    pub used_cpu_sys: f64,
    pub used_cpu_user: f64,
    pub instantaneous_ops_per_sec: u32,
    pub keys_count: u64,
    pub db_info: Vec<(String, u64)>,
}

fn parse_simple_value(info_str: &str, key: &str) -> Option<String> {
    for line in info_str.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() == 2 && parts[0].trim() == key {
            return Some(parts[1].trim().to_string());
        }
    }
    None
}

fn fetch_info_section(client: &mut RedisClient, section: &str) -> anyhow::Result<String> {
    let cmd = format!("INFO {}", section);
    let resp_type = crate::redis_type::RespType::create_from_command_line(&cmd);
    client.write_command(resp_type)?;
    let response = client.read_resp()?;
    
    let info_str = match &response {
        crate::redis_type::RespType::BulkStrings(bs) => bs.value.clone(),
        crate::redis_type::RespType::SimpleStrings(ss) => ss.value.clone(),
        _ => String::new(),
    };
    Ok(info_str)
}

fn parse_keyspace_info(info_str: &str) -> Vec<(String, u64)> {
    let mut result = Vec::new();
    for line in info_str.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() == 2 {
            let db_name = parts[0].trim().to_string();
            let keys_part: Vec<&str> = parts[1].split(',').collect();
            if let Some(keys_str) = keys_part.first() {
                let kv: Vec<&str> = keys_str.split('=').collect();
                if kv.len() == 2 {
                    if let Ok(count) = kv[1].parse::<u64>() {
                        result.push((db_name, count));
                    }
                }
            }
        }
    }
    result
}

fn fetch_stats(client: &mut RedisClient) -> anyhow::Result<RedisStats> {
    let mut stats = RedisStats::default();
    
    let memory_info = fetch_info_section(client, "MEMORY")?;
    stats.used_memory_human = parse_simple_value(&memory_info, "used_memory_human").unwrap_or_default();
    stats.used_memory = parse_simple_value(&memory_info, "used_memory").and_then(|v| v.parse().ok()).unwrap_or(0);
    stats.maxmemory = parse_simple_value(&memory_info, "maxmemory").and_then(|v| v.parse().ok()).unwrap_or(0);
    
    let stats_info = fetch_info_section(client, "STATS")?;
    stats.connected_clients = parse_simple_value(&stats_info, "connected_clients").and_then(|v| v.parse().ok()).unwrap_or(0);
    stats.total_connections_received = parse_simple_value(&stats_info, "total_connections_received").and_then(|v| v.parse().ok()).unwrap_or(0);
    stats.instantaneous_ops_per_sec = parse_simple_value(&stats_info, "instantaneous_ops_per_sec").and_then(|v| v.parse().ok()).unwrap_or(0);
    
    let cpu_info = fetch_info_section(client, "CPU")?;
    stats.used_cpu_sys = parse_simple_value(&cpu_info, "used_cpu_sys").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    stats.used_cpu_user = parse_simple_value(&cpu_info, "used_cpu_user").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    
    let replication_info = fetch_info_section(client, "REPLICATION")?;
    stats.connected_slaves = parse_simple_value(&replication_info, "connected_slaves").and_then(|v| v.parse().ok()).unwrap_or(0);
    
    let keyspace_info = fetch_info_section(client, "KEYSPACE")?;
    stats.db_info = parse_keyspace_info(&keyspace_info);
    
    Ok(stats)
}

pub fn run_monitor(client: &mut RedisClient) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    let mut stats = fetch_stats(client)?;
    
    loop {
        terminal.draw(|f| {
            let size = f.area();
            
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(6),
                    Constraint::Length(6),
                    Constraint::Min(2),
                ])
                .split(size);
            
            let title = Paragraph::new(Span::styled(
                "Redis Monitor - Press 'q' or 'ESC' to exit",
                Style::default().fg(Color::Cyan),
            ))
            .block(Block::default().borders(Borders::ALL).title("Status"));
            f.render_widget(title, main_chunks[0]);
            
            let memory_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(main_chunks[1]);
            
            let memory_percent = if stats.maxmemory > 0 {
                (stats.used_memory as f64 / stats.maxmemory as f64).min(1.0)
            } else {
                0.0
            };
            
            let memory_gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title("Memory Usage"))
                .gauge_style(Style::default().fg(Color::Magenta))
                .percent((memory_percent * 100.0) as u16)
                .label(Span::raw(stats.used_memory_human.clone()));
            f.render_widget(memory_gauge, memory_chunks[0]);
            
            let cpu_percent = ((stats.used_cpu_sys + stats.used_cpu_user) * 10.0).min(100.0).max(0.0);
            let cpu_gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title("CPU Usage (est)"))
                .gauge_style(Style::default().fg(Color::Yellow))
                .percent(cpu_percent as u16)
                .label(Span::raw(format!("sys: {:.2} user: {:.2}", stats.used_cpu_sys, stats.used_cpu_user)));
            f.render_widget(cpu_gauge, memory_chunks[1]);
            
            let conn_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(33),
                    Constraint::Percentage(33),
                    Constraint::Percentage(34),
                ])
                .split(main_chunks[2]);
            
            let conn_info = format!(
                "Connected Clients: {}\nTotal Received: {}\nConnected Slaves: {}",
                stats.connected_clients, stats.total_connections_received, stats.connected_slaves
            );
            let conn_widget = Paragraph::new(conn_info)
                .block(Block::default().borders(Borders::ALL).title("Connections"));
            f.render_widget(conn_widget, conn_chunks[0]);
            
            let ops_info = format!(
                "Ops/sec: {}",
                stats.instantaneous_ops_per_sec
            );
            let ops_widget = Paragraph::new(ops_info)
                .block(Block::default().borders(Borders::ALL).title("Performance"));
            f.render_widget(ops_widget, conn_chunks[1]);
            
            let db_text: String = stats.db_info
                .iter()
                .map(|(name, keys)| format!("{}: {} keys\n", name, keys))
                .collect();
            let db_widget = Paragraph::new(db_text)
                .block(Block::default().borders(Borders::ALL).title("Databases"));
            f.render_widget(db_widget, conn_chunks[2]);
            
            let help = Paragraph::new(Span::styled(
                "Data refreshes every 2 seconds...",
                Style::default().fg(Color::Gray),
            ));
            f.render_widget(help, main_chunks[3]);
        })?;
        
        if event::poll(Duration::from_secs(2))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    _ => {}
                }
            }
        }
        
        stats = fetch_stats(client)?;
    }
    
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    
    Ok(())
}
