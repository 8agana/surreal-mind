use std::collections::VecDeque;
use std::fs;
use std::io;
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ratatui::crossterm::{event, execute, terminal};
use ratatui::prelude::*;
use ratatui::widgets::*;
use reqwest::blocking::Client;
use serde_json::{Value, from_str};

#[derive(Default, Clone)]
enum LogFilter {
    #[default]
    All,
    Stdout,
    Stderr,
    Cloudflared,
}

#[derive(Clone)]
enum OpsAction {
    KgPopulate,
    KgEmbed,
    ReembedKg,
    HealthCheck,
    BuildRestart,
    Fmt,
    Clippy,
}

#[derive(Clone)]
enum OpsEvent {
    Line(String),
    Done { exit: i32, duration_ms: u128 },
}

#[derive(Default)]
struct Status {
    service_running: bool,
    cloudflared_running: bool,
    health_local: bool,
    health_remote: bool,
    latency_local_ms: Option<u128>,
    latency_remote_ms: Option<u128>,
    total_requests: Option<u64>,
    rps: Option<f64>,
    url: String,
    token: String,
    log_scroll: u16,
    lat_local_hist: Vec<f64>,
    lat_remote_hist: Vec<f64>,
    rps_history: Vec<f64>,
    combined_log_tail: Vec<String>,
    use_header_auth: bool,
    http_active_sessions: Option<usize>,
    http_total_sessions: Option<u64>,
    db_connected: Option<bool>,
    db_ping_ms: Option<u64>,
    db_ns: Option<String>,
    db_db: Option<String>,
    resource_cpu: Option<f64>,
    resource_rss_mb: Option<f64>,
    resource_uptime: Option<String>,
    log_filter: LogFilter,
    log_tail_limit: usize,
    info_cache: Option<(Instant, Value)>,
    stdio_sessions: Option<usize>,
    tunnel_url: Option<String>,
    show_detail: bool,
    ops_last_cmd: Option<String>,
    ops_last_status: Option<i32>,
    ops_last_duration_ms: Option<u128>,
    ops_last_started_at: Option<Instant>,
    ops_output_tail: VecDeque<String>,
    ops_output_limit: usize,
    ops_running: bool,
    ops_auto_restart: bool,
    ops_use_release_bins: bool,
    ops_dry_run: bool,
    ops_batch_size: usize,
    ops_limit: Option<usize>,
    ops_spinner_frame: usize,
}

fn main() -> anyhow::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut status = gather_status(None);
    let mut last_refresh = Instant::now();
    let mut ops_rx: Option<mpsc::Receiver<OpsEvent>> = None;

    loop {
        terminal.draw(|f| ui(f, &status))?;

        if last_refresh.elapsed() >= Duration::from_secs(2) {
            status = gather_status(Some(&status));
            last_refresh = Instant::now();
        }

        // Poll for ops events
        if let Some(rx) = &ops_rx {
            let mut done = false;
            while let Ok(event) = rx.try_recv() {
                match event {
                    OpsEvent::Line(line) => {
                        if status.ops_output_tail.len() >= status.ops_output_limit {
                            status.ops_output_tail.pop_front();
                        }
                        status.ops_output_tail.push_back(line);
                    }
                    OpsEvent::Done { exit, duration_ms } => {
                        status.ops_running = false;
                        status.ops_last_status = Some(exit);
                        status.ops_last_duration_ms = Some(duration_ms);
                        status.ops_last_started_at = None;
                        done = true;
                        let summary = format!(
                            "[ops] {}: {} in {:.1}s",
                            status.ops_last_cmd.as_deref().unwrap_or("?"),
                            if exit == 0 { "success" } else { "fail" },
                            duration_ms as f64 / 1000.0
                        );
                        status.combined_log_tail.push(summary);
                        if status.combined_log_tail.len() > status.log_tail_limit {
                            let _ = status.combined_log_tail.drain(0..1);
                        }
                    }
                }
            }
            if done {
                ops_rx = None;
            }
            if status.ops_running {
                status.ops_spinner_frame = (status.ops_spinner_frame + 1) % 4;
            }
        }

        if event::poll(Duration::from_millis(200))?
            && let event::Event::Key(k) = event::read()?
        {
            use ratatui::crossterm::event::{KeyCode, KeyModifiers};
            match k.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('r') => {
                    let _ = restart_sm();
                    status = gather_status(Some(&status));
                }
                KeyCode::Char('f') => {
                    let _ = start_cloudflared();
                    status = gather_status(Some(&status));
                }
                KeyCode::Char('g') => {
                    let _ = stop_cloudflared();
                    status = gather_status(Some(&status));
                }
                KeyCode::Char('y') => {
                    let _ = copy_to_clipboard(&status.url);
                }
                KeyCode::Char('Y') => {
                    let _ = copy_to_clipboard(&status.token);
                }
                KeyCode::Char('u') => {
                    let _ = copy_to_clipboard(status.tunnel_url.as_deref().unwrap_or(""));
                }
                KeyCode::Char('a') => {
                    status.use_header_auth = !status.use_header_auth;
                }
                KeyCode::PageUp => status.log_scroll = status.log_scroll.saturating_add(10),
                KeyCode::PageDown => status.log_scroll = status.log_scroll.saturating_sub(10),
                KeyCode::Home | KeyCode::Char('b') => {
                    status.log_scroll = status.combined_log_tail.len() as u16
                }
                KeyCode::End | KeyCode::Char('e') => status.log_scroll = 0,
                KeyCode::Char('s') => {
                    status.log_filter = match status.log_filter {
                        LogFilter::All => LogFilter::Stdout,
                        LogFilter::Stdout => LogFilter::Stderr,
                        LogFilter::Stderr => LogFilter::Cloudflared,
                        LogFilter::Cloudflared => LogFilter::All,
                    };
                }
                KeyCode::Char('t') => {
                    status.show_detail = !status.show_detail;
                }
                KeyCode::Char('k') => trigger_ops(&mut status, OpsAction::KgPopulate, &mut ops_rx),
                KeyCode::Char('G') => trigger_ops(&mut status, OpsAction::KgEmbed, &mut ops_rx),
                KeyCode::Char('i') => trigger_ops(&mut status, OpsAction::ReembedKg, &mut ops_rx),
                KeyCode::Char('h') => trigger_ops(&mut status, OpsAction::HealthCheck, &mut ops_rx),
                KeyCode::Char('j') => {
                    trigger_ops(&mut status, OpsAction::BuildRestart, &mut ops_rx)
                }
                KeyCode::Char('m') => trigger_ops(&mut status, OpsAction::Fmt, &mut ops_rx),
                KeyCode::Char('n') => trigger_ops(&mut status, OpsAction::Clippy, &mut ops_rx),
                KeyCode::Char('A') => status.ops_auto_restart = !status.ops_auto_restart,
                KeyCode::Char('B') => status.ops_use_release_bins = !status.ops_use_release_bins,
                KeyCode::Char('D') => status.ops_dry_run = !status.ops_dry_run,
                KeyCode::Char('x') => {
                    status.ops_output_tail.clear();
                    status.ops_last_cmd = None;
                    status.ops_last_status = None;
                    status.ops_last_duration_ms = None;
                    status.ops_running = false;
                }
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => break,
                _ => {}
            }
        }
    }

    terminal::disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::LeaveAlternateScreen)?;
    Ok(())
}

fn ui(f: &mut Frame, s: &Status) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(12),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Min(4),
            Constraint::Length(4),
        ])
        .split(f.area());

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "Surreal‑Mind — Remote MCP Dashboard",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  |  "),
        Span::raw(format!("URL: {}", s.url)),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Overview"));
    f.render_widget(header, chunks[0]);

    let row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let svc = Paragraph::new(vec![
        Line::raw(format!("Service: {}", on_off(s.service_running))),
        Line::raw(format!(
            "Local health: {}{}",
            ok_fail(s.health_local),
            s.latency_local_ms
                .map(|d| format!(" ({d} ms)"))
                .unwrap_or_default()
        )),
        Line::raw("Bind: 127.0.0.1:8787"),
        Line::raw("Path: /mcp"),
        Line::raw(format!(
            "CPU: {}%",
            s.resource_cpu
                .map(|c| format!("{:.1}", c))
                .unwrap_or("–".to_string())
        )),
        Line::raw(format!(
            "RSS: {} MB",
            s.resource_rss_mb
                .map(|r| format!("{:.1}", r))
                .unwrap_or("–".to_string())
        )),
        Line::raw(format!(
            "Uptime: {}",
            s.resource_uptime.as_deref().unwrap_or("–")
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title("MCP Service"));
    f.render_widget(svc, row[0]);

    let cf_block = Block::default().borders(Borders::ALL).title("Cloudflared");
    let cf_area = row[1];
    f.render_widget(cf_block, cf_area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(1)])
        .margin(1)
        .split(cf_area);

    let cf_text = Paragraph::new(vec![
        Line::raw(format!("Cloudflared: {}", on_off(s.cloudflared_running))),
        Line::raw(format!(
            "Remote health: {}{}",
            ok_fail(s.health_remote),
            s.latency_remote_ms
                .map(|d| format!(" ({d} ms)"))
                .unwrap_or_default()
        )),
        Line::raw(match (s.total_requests, s.rps) {
            (Some(t), Some(r)) => format!("MCP requests: {}  (~{:.1}/s)", t, r),
            (Some(t), None) => format!("MCP requests: {}", t),
            _ => String::from("MCP requests: –"),
        }),
        Line::raw(if let Some(url) = &s.tunnel_url {
            format!("Tunnel: {}", url)
        } else {
            "Tunnel: – (if available)".to_string()
        }),
    ]);
    f.render_widget(cf_text, inner[0]);

    let data: Vec<u64> = s
        .rps_history
        .iter()
        .map(|v| (*v * 10.0).max(0.0) as u64)
        .collect();
    let spark = Sparkline::default()
        .block(Block::default().borders(Borders::NONE).title("RPS"))
        .data(&data)
        .style(Style::default().fg(Color::Green));
    f.render_widget(spark, inner[1]);

    // Row 2: Sessions, DB, and Ops
    let row2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
        ])
        .split(chunks[2]);

    let sessions_text = Paragraph::new(vec![
        Line::raw(format!(
            "HTTP Sessions: {}/{}",
            s.http_active_sessions.unwrap_or(0),
            s.http_total_sessions.unwrap_or(0)
        )),
        Line::raw(if let Some(stdio) = s.stdio_sessions {
            format!("Stdio Sessions: {}", stdio)
        } else {
            "Stdio Sessions: –".to_string()
        }),
    ])
    .block(Block::default().borders(Borders::ALL).title("Sessions"));
    f.render_widget(sessions_text, row2[0]);

    let db_text = Paragraph::new(vec![
        Line::raw(format!(
            "Connected: {}",
            s.db_connected
                .map(|c| if c { "yes" } else { "no" })
                .unwrap_or("–")
        )),
        Line::raw(format!(
            "Ping: {} ms",
            s.db_ping_ms
                .map(|p| p.to_string())
                .unwrap_or("–".to_string())
        )),
        Line::raw(format!(
            "NS/DB: {}/{}",
            s.db_ns.as_deref().unwrap_or("–"),
            s.db_db.as_deref().unwrap_or("–")
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title("SurrealDB"));
    f.render_widget(db_text, row2[1]);

    let spinner = if s.ops_running {
        ["|", "/", "-", "\\"][s.ops_spinner_frame % 4]
    } else {
        " "
    };
    let ops_lines = vec![
        Line::raw(format!(
            "k: kg_populate{} (batch={}, dry={})",
            spinner,
            s.ops_batch_size,
            on_off(s.ops_dry_run)
        )),
        Line::raw(format!(
            "G: kg_embed (limit={:?}, dry={})",
            s.ops_limit,
            on_off(s.ops_dry_run)
        )),
        Line::raw(format!(
            "i: reembed_kg (limit={:?}, dry={})",
            s.ops_limit,
            on_off(s.ops_dry_run)
        )),
        Line::raw("h: health check".to_string()),
        Line::raw(format!(
            "j: build+restart (auto={})",
            on_off(s.ops_auto_restart)
        )),
        Line::raw("m: fmt".to_string()),
        Line::raw("n: clippy".to_string()),
    ];
    let ops_p =
        Paragraph::new(ops_lines).block(Block::default().borders(Borders::ALL).title("Ops"));
    f.render_widget(ops_p, row2[2]);

    // Command Runner pane
    let last_cmd = s.ops_last_cmd.as_deref().unwrap_or("none");
    let status_str = if s.ops_running {
        "running"
    } else if let Some(st) = s.ops_last_status {
        if st == 0 { "success" } else { "fail" }
    } else {
        "none"
    };
    let duration_str = s
        .ops_last_duration_ms
        .map(|d| format!("{:.1}s", d as f64 / 1000.0))
        .unwrap_or_default();
    let output_lines: Vec<Line> = s
        .ops_output_tail
        .iter()
        .rev()
        .take(10)
        .rev()
        .map(Line::raw)
        .collect();
    let status_color = if s.ops_running {
        Color::Yellow
    } else if let Some(st) = s.ops_last_status {
        if st == 0 { Color::Green } else { Color::Red }
    } else {
        Color::White
    };
    let runner_lines = vec![
        Line::raw(format!("Last cmd: {}", last_cmd)),
        Line::from(vec![
            Span::raw("Status: "),
            Span::styled(status_str, Style::default().fg(status_color)),
        ]),
        Line::raw(format!("Duration: {}", duration_str)),
    ];
    let mut all_lines = runner_lines;
    all_lines.push(Line::raw("Output:"));
    all_lines.extend(output_lines);
    let runner_p = Paragraph::new(all_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Command Runner"),
    );
    f.render_widget(runner_p, chunks[3]);

    // Logs pane
    let logs_lines: Vec<Line> = if s.combined_log_tail.is_empty() {
        vec![Line::raw("(no logs yet)")]
    } else {
        let h = chunks[4].height as usize;
        let filtered_logs: Vec<_> = s
            .combined_log_tail
            .iter()
            .filter(|line| match s.log_filter {
                LogFilter::All => true,
                LogFilter::Stdout => line.contains("[sm.out]"),
                LogFilter::Stderr => line.contains("[sm.err]"),
                LogFilter::Cloudflared => line.contains("[cf.out]") || line.contains("[cf.err]"),
            })
            .collect();
        filtered_logs
            .iter()
            .rev()
            .skip(s.log_scroll as usize)
            .take(h.saturating_sub(2))
            .rev()
            .map(|l| Line::raw(l.as_str()))
            .collect()
    };
    let logs_p = Paragraph::new(logs_lines)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Recent Logs ({}) (PgUp/PgDn)",
            match s.log_filter {
                LogFilter::All => "all",
                LogFilter::Stdout => "stdout",
                LogFilter::Stderr => "stderr",
                LogFilter::Cloudflared => "cloudflared",
            }
        )))
        .wrap(Wrap { trim: true });
    f.render_widget(logs_p, chunks[4]);

    let help_text = format!(
        "Keys: q/Esc quit • r restart • f/g cloudflared • y/Y/u copy • a auth • s log filter • e end • b beginning • t detail\n • ops: k/G/i/h/j/m/n • toggles: A/B/D • x clear ops\nAuth: {} • Detail: {}",
        if s.use_header_auth { "header" } else { "query" },
        if s.show_detail { "ON" } else { "OFF" }
    );
    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: true });
    f.render_widget(help, chunks[5]);
}

fn on_off(b: bool) -> String {
    if b { "on".into() } else { "off".into() }
}
fn ok_fail(b: bool) -> String {
    if b { "ok".into() } else { "fail".into() }
}

fn gather_status(prev: Option<&Status>) -> Status {
    let token = std::fs::read_to_string(format!(
        "{}/.surr_token",
        std::env::var("HOME").unwrap_or_default()
    ))
    .unwrap_or_default()
    .trim()
    .to_string();
    let url = if token.is_empty() {
        "https://mcp.samataganaphotography.com/mcp".to_string()
    } else {
        format!(
            "https://mcp.samataganaphotography.com/mcp?access_token={}",
            token
        )
    };
    let mut st = Status {
        url: url.clone(),
        token: token.clone(),
        use_header_auth: prev.map(|p| p.use_header_auth).unwrap_or(false),
        log_filter: prev.map(|p| p.log_filter.clone()).unwrap_or_default(),
        log_tail_limit: prev.map(|p| p.log_tail_limit).unwrap_or(400),
        ops_last_cmd: prev.and_then(|p| p.ops_last_cmd.clone()),
        ops_last_status: prev.and_then(|p| p.ops_last_status),
        ops_last_duration_ms: prev.and_then(|p| p.ops_last_duration_ms),
        ops_last_started_at: prev.and_then(|p| p.ops_last_started_at),
        ops_output_tail: prev
            .map(|p| p.ops_output_tail.iter().cloned().collect())
            .unwrap_or_default(),
        ops_output_limit: std::env::var("SMTOP_OPS_TAIL")
            .unwrap_or_else(|_| "200".to_string())
            .parse()
            .unwrap_or(200),
        ops_running: prev.map(|p| p.ops_running).unwrap_or(false),
        ops_auto_restart: prev.map(|p| p.ops_auto_restart).unwrap_or(true),
        ops_use_release_bins: prev.map(|p| p.ops_use_release_bins).unwrap_or(true),
        ops_dry_run: prev.map(|p| p.ops_dry_run).unwrap_or(false),
        ops_batch_size: std::env::var("KG_POPULATE_BATCH_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5),
        ops_limit: std::env::var("LIMIT").ok().and_then(|s| s.parse().ok()),
        ops_spinner_frame: prev.map(|p| p.ops_spinner_frame).unwrap_or(0),
        ..Default::default()
    };

    // Service running?
    st.service_running = port_listening(8787);
    st.cloudflared_running = is_process_running("cloudflared");

    // Local /health
    let (hl, ll) = http_health_latency("http://127.0.0.1:8787/health");
    st.health_local = hl;
    st.latency_local_ms = ll;
    // Remote /health
    let (hr, lr) = http_health_latency("https://mcp.samataganaphotography.com/health");
    st.health_remote = hr;
    st.latency_remote_ms = lr;

    // /metrics for total_requests and new fields
    if !token.is_empty() {
        if let Some(m) =
            http_json_auth_mode("http://127.0.0.1:8787/metrics", &token, st.use_header_auth)
        {
            if let Some(t) = m.get("total_requests").and_then(|v| v.as_u64()) {
                st.total_requests = Some(t);
            }
            st.http_active_sessions = m
                .get("http_active_sessions")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            st.http_total_sessions = m.get("http_total_sessions").and_then(|v| v.as_u64());
            if let Some(prev_t) = prev.and_then(|p| p.total_requests)
                && let Some(cur) = st.total_requests
            {
                let dt = 2.0_f64;
                st.rps = Some((cur.saturating_sub(prev_t) as f64) / dt);
            }
            let mut rps_hist = prev.map(|p| p.rps_history.clone()).unwrap_or_default();
            if let Some(rps) = st.rps {
                rps_hist.push(rps);
            }
            if rps_hist.len() > 60 {
                let _ = rps_hist.drain(0..(rps_hist.len() - 60));
            }
            st.rps_history = rps_hist;
        }

        // /info with caching
        let info_ttl_ms = std::env::var("SMTOP_INFO_TTL_MS")
            .unwrap_or_else(|_| "1000".to_string())
            .parse::<u64>()
            .unwrap_or(1000);
        let now = Instant::now();
        let should_fetch_info = st
            .info_cache
            .as_ref()
            .map(|(ts, _)| now.duration_since(*ts).as_millis() as u64 > info_ttl_ms)
            .unwrap_or(true);
        if should_fetch_info {
            if let Some(info) =
                http_json_auth_mode("http://127.0.0.1:8787/info", &token, st.use_header_auth)
            {
                st.info_cache = Some((now, info.clone()));
                if let Some(db) = info.get("db").and_then(|v| v.as_object()) {
                    st.db_connected = db.get("connected").and_then(|v| v.as_bool());
                    st.db_ping_ms = db.get("ping_ms").and_then(|v| v.as_u64());
                    st.db_ns = db.get("ns").and_then(|v| v.as_str()).map(|s| s.to_string());
                    st.db_db = db.get("db").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
            }
        } else if let Some((_, info)) = &st.info_cache
            && let Some(db) = info.get("db").and_then(|v| v.as_object())
        {
            st.db_connected = db.get("connected").and_then(|v| v.as_bool());
            st.db_ping_ms = db.get("ping_ms").and_then(|v| v.as_u64());
            st.db_ns = db.get("ns").and_then(|v| v.as_str()).map(|s| s.to_string());
            st.db_db = db.get("db").and_then(|v| v.as_str()).map(|s| s.to_string());
        }
    }

    // Track latency histories
    if let Some(ms) = st.latency_local_ms {
        let mut v = prev.map(|p| p.lat_local_hist.clone()).unwrap_or_default();
        v.push(ms as f64);
        if v.len() > 60 {
            let _ = v.drain(0..(v.len() - 60));
        }
        st.lat_local_hist = v;
    }
    if let Some(ms) = st.latency_remote_ms {
        let mut v = prev.map(|p| p.lat_remote_hist.clone()).unwrap_or_default();
        v.push(ms as f64);
        if v.len() > 60 {
            let _ = v.drain(0..(v.len() - 60));
        }
        st.lat_remote_hist = v;
    }

    // Set log tail limit from env
    st.log_tail_limit = std::env::var("SMTOP_LOG_TAIL")
        .unwrap_or_else(|_| "400".to_string())
        .parse::<usize>()
        .unwrap_or(400);

    // Merge logs
    let logs_dir = format!("{}/Library/Logs", std::env::var("HOME").unwrap_or_default());
    let combined = merge_logs_chrono(
        &format!("{}/surreal-mind.out.log", logs_dir),
        &format!("{}/surreal-mind.err.log", logs_dir),
        &format!("{}/cloudflared-tunnel.out.log", logs_dir),
        &format!("{}/cloudflared-tunnel.err.log", logs_dir),
        st.log_tail_limit,
    );

    // Parse tunnel URL from cloudflared logs (prefer latest, fallback sources)
    st.tunnel_url = combined
        .iter()
        .rev()
        .find(|line| {
            (line.contains("cloudflared-tunnel.out") || line.contains("cloudflared.out"))
                && line.contains("https://")
        })
        .and_then(|line| {
            if let Some(start) = line.find("https://") {
                let url_part = &line[start..];
                url_part.split_whitespace().next().map(|s| s.to_string())
            } else {
                None
            }
        });

    st.combined_log_tail = combined;

    // Read stdio state.json
    if let Some(data_dir) = dirs::data_dir() {
        let state_file = data_dir.join("surreal-mind").join("state.json");
        if let Ok(content) = fs::read_to_string(state_file)
            && let Ok(state) = from_str::<Value>(&content)
        {
            st.stdio_sessions = state
                .get("sessions")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
        }
    }

    // Gather resource usage for surreal-mind process
    if let Some(pid) = get_surreal_mind_pid()
        && let Some((cpu, rss_kb, uptime)) = get_process_stats(pid)
    {
        st.resource_cpu = Some(cpu);
        st.resource_rss_mb = Some(rss_kb as f64 / 1024.0);
        st.resource_uptime = Some(uptime);
    }

    st
}

fn get_surreal_mind_pid() -> Option<u32> {
    let output = Command::new("pgrep")
        .arg("-x")
        .arg("surreal-mind")
        .output()
        .ok()?;
    let stdout = String::from_utf8(output.stdout).ok()?;
    let pid_str = stdout.lines().next()?;
    pid_str.parse::<u32>().ok()
}

fn get_process_stats(pid: u32) -> Option<(f64, u64, String)> {
    let output = Command::new("ps")
        .args(["-o", "%cpu=,rss=,etime=", "-p"])
        .arg(pid.to_string())
        .output()
        .ok()?;
    let stdout = String::from_utf8(output.stdout).ok()?;
    let parts: Vec<&str> = stdout.split_whitespace().collect();
    if parts.len() >= 3 {
        let cpu = parts[0].parse::<f64>().ok()?;
        let rss = parts[1].parse::<u64>().ok()?;
        let etime = parts[2].to_string();
        Some((cpu, rss, etime))
    } else {
        None
    }
}

fn port_listening(port: u16) -> bool {
    // lsof -iTCP:port is heavy; instead try connecting quickly
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

fn is_process_running(name: &str) -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}

fn http_health_latency(url: &str) -> (bool, Option<u128>) {
    let client = Client::builder()
        .timeout(Duration::from_millis(2000))
        .danger_accept_invalid_certs(true)
        .build();
    if let Ok(c) = client {
        let t0 = Instant::now();
        if let Ok(resp) = c.get(url).send() {
            return (resp.status().is_success(), Some(t0.elapsed().as_millis()));
        }
    }
    (false, None)
}

fn http_json_auth_mode(url: &str, tok: &str, header: bool) -> Option<Value> {
    let client = Client::builder()
        .timeout(Duration::from_millis(1500))
        .danger_accept_invalid_certs(true)
        .build()
        .ok()?;
    let req = if header {
        client
            .get(url)
            .header("Authorization", format!("Bearer {}", tok))
    } else {
        let sep = if url.contains('?') { '&' } else { '?' };
        let full = format!("{url}{sep}access_token={tok}");
        client.get(&full)
    };
    let resp = req.send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<Value>().ok()
}

fn restart_sm() -> anyhow::Result<()> {
    Command::new("launchctl")
        .args([
            "kickstart",
            "-k",
            &format!("gui/{}/dev.legacymind.surreal-mind", uid()),
        ])
        .status()?;
    Ok(())
}

fn start_cloudflared() -> anyhow::Result<()> {
    Command::new("launchctl")
        .args([
            "kickstart",
            "-k",
            &format!("gui/{}/com.legacymind.cloudflared-tunnel", uid()),
        ])
        .status()?;
    Ok(())
}

fn stop_cloudflared() -> anyhow::Result<()> {
    Command::new("launchctl")
        .args([
            "bootout",
            &format!("gui/{}/com.legacymind.cloudflared-tunnel", uid()),
        ])
        .status()?;
    Ok(())
}

fn uid() -> String {
    // Portable-ish: id -u
    let out = Command::new("id").arg("-u").output().ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "501".to_string())
        .trim()
        .to_string()
}

fn run_command_async(
    cmd: String,
    args: Vec<String>,
    cwd: String,
    env_vars: Vec<(String, String)>,
    tx: mpsc::Sender<OpsEvent>,
) {
    std::thread::spawn(move || {
        let mut child = match Command::new(&cmd)
            .args(&args)
            .current_dir(&cwd)
            .envs(env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(OpsEvent::Line(format!("[err] failed to spawn: {}", e)));
                let _ = tx.send(OpsEvent::Done {
                    exit: -1,
                    duration_ms: 0,
                });
                return;
            }
        };

        let start = Instant::now();

        let tx_stdout = tx.clone();
        let tx_stderr = tx.clone();

        if let Some(stdout) = child.stdout.take() {
            std::thread::spawn(move || {
                use std::io::BufRead;
                let reader = std::io::BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            let _ = tx_stdout.send(OpsEvent::Line(format!("[out] {}", l)));
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                use std::io::BufRead;
                let reader = std::io::BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            let _ = tx_stderr.send(OpsEvent::Line(format!("[err] {}", l)));
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        let status = child.wait();
        let duration_ms = start.elapsed().as_millis();
        let exit = match status {
            Ok(s) => s.code().unwrap_or(-1),
            Err(_) => -1,
        };
        let _ = tx.send(OpsEvent::Done { exit, duration_ms });
    });
}

fn trigger_ops(
    status: &mut Status,
    action: OpsAction,
    ops_rx: &mut Option<mpsc::Receiver<OpsEvent>>,
) {
    if status.ops_running {
        return;
    }
    let cwd = "/Users/samuelatagana/Projects/LegacyMind/surreal-mind".to_string();
    let mut env_vars = Vec::new();
    if status.ops_dry_run {
        env_vars.push(("DRY_RUN".to_string(), "1".to_string()));
    }
    let cmd_str = match &action {
        OpsAction::KgPopulate => format!(
            "kg_populate batch={} dry_run={}",
            status.ops_batch_size,
            if status.ops_dry_run { "on" } else { "off" }
        ),
        OpsAction::KgEmbed => format!(
            "kg_embed limit={:?} dry_run={}",
            status.ops_limit,
            if status.ops_dry_run { "on" } else { "off" }
        ),
        OpsAction::ReembedKg => format!(
            "reembed_kg limit={:?} dry_run={}",
            status.ops_limit,
            if status.ops_dry_run { "on" } else { "off" }
        ),
        OpsAction::HealthCheck => "health check".to_string(),
        OpsAction::BuildRestart => format!(
            "build+restart auto={}",
            if status.ops_auto_restart { "on" } else { "off" }
        ),
        OpsAction::Fmt => "fmt".to_string(),
        OpsAction::Clippy => "clippy".to_string(),
    };
    status.ops_last_cmd = Some(cmd_str);
    status.ops_last_started_at = Some(Instant::now());
    status.ops_running = true;
    status.ops_spinner_frame = 0; // reset spinner
    let (tx, rx) = mpsc::channel();
    *ops_rx = Some(rx);

    let mut env_vars_clone = env_vars.clone();
    env_vars_clone.push(("RUST_BACKTRACE".to_string(), "1".to_string())); // inherit

    let bin = match action {
        OpsAction::KgPopulate => "kg_populate",
        OpsAction::KgEmbed => "kg_embed",
        OpsAction::ReembedKg => "reembed_kg",
        OpsAction::HealthCheck => {
            // special case
            run_command_async(
                "sh".to_string(),
                vec!["scripts/sm_health.sh".to_string()],
                cwd,
                env_vars_clone,
                tx,
            );
            return;
        }
        OpsAction::BuildRestart => {
            // special case
            let cmd_str = if status.ops_auto_restart {
                "cargo build --release --bin surreal-mind && launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind".to_string()
            } else {
                "cargo build --release --bin surreal-mind".to_string()
            };
            run_command_async(
                "sh".to_string(),
                vec!["-c".to_string(), cmd_str],
                cwd,
                env_vars_clone,
                tx,
            );
            return;
        }
        OpsAction::Fmt => {
            run_command_async(
                "cargo".to_string(),
                vec!["fmt".to_string(), "--all".to_string()],
                cwd,
                env_vars_clone,
                tx,
            );
            return;
        }
        OpsAction::Clippy => {
            run_command_async(
                "cargo".to_string(),
                vec![
                    "clippy".to_string(),
                    "--workspace".to_string(),
                    "--all-targets".to_string(),
                    "--".to_string(),
                    "-D".to_string(),
                    "warnings".to_string(),
                ],
                cwd,
                env_vars_clone,
                tx,
            );
            return;
        }
    };

    let release_path = format!("target/release/{}", bin);
    let (cmd, args) = if std::fs::metadata(&release_path).is_ok() {
        (release_path, vec![])
    } else {
        (
            "cargo".to_string(),
            vec!["run".to_string(), "--bin".to_string(), bin.to_string()],
        )
    };

    if matches!(action, OpsAction::KgPopulate) {
        env_vars_clone.push((
            "KG_POPULATE_BATCH_SIZE".to_string(),
            status.ops_batch_size.to_string(),
        ));
    }
    if matches!(action, OpsAction::KgEmbed | OpsAction::ReembedKg)
        && let Some(limit) = status.ops_limit
    {
        env_vars_clone.push(("LIMIT".to_string(), limit.to_string()));
    }

    run_command_async(cmd, args, cwd, env_vars_clone, tx);
}

fn copy_to_clipboard(clip: &str) -> anyhow::Result<()> {
    let mut child = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        write!(stdin, "{}", clip)?;
    }
    let _ = child.wait()?;
    Ok(())
}

fn tail_lines(path: &str, max: usize) -> Vec<String> {
    if let Ok(data) = std::fs::read_to_string(path) {
        let mut lines: Vec<String> = data.lines().map(|s| s.to_string()).collect();
        if lines.len() > max {
            lines = lines.split_off(lines.len() - max);
        }
        return lines;
    }
    vec![]
}

fn parse_ts(line: &str) -> Option<std::time::SystemTime> {
    if let Some(first) = line.split_whitespace().next()
        && let Ok(dt) =
            time::OffsetDateTime::parse(first, &time::format_description::well_known::Rfc3339)
    {
        return Some(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(dt.unix_timestamp() as u64),
        );
    }
    None
}

fn merge_logs_chrono(out1: &str, err1: &str, out2: &str, err2: &str, max: usize) -> Vec<String> {
    let mut merged: Vec<(Option<std::time::SystemTime>, usize, String)> = Vec::new();
    for (i, l) in tail_lines(out1, max).into_iter().enumerate() {
        merged.push((parse_ts(&l), i, format!("[sm.out] {}", l)));
    }
    for (i, l) in tail_lines(err1, max).into_iter().enumerate() {
        merged.push((parse_ts(&l), i, format!("[sm.err] {}", l)));
    }
    for (i, l) in tail_lines(out2, max).into_iter().enumerate() {
        merged.push((parse_ts(&l), i, format!("[cf.out] {}", l)));
    }
    for (i, l) in tail_lines(err2, max).into_iter().enumerate() {
        merged.push((parse_ts(&l), i, format!("[cf.err] {}", l)));
    }
    merged.sort_by(|a, b| match (a.0, b.0) {
        (Some(ta), Some(tb)) => ta.cmp(&tb),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.1.cmp(&b.1),
    });
    let mut out: Vec<String> = merged.into_iter().map(|(_, _, s)| s).collect();
    if out.len() > max {
        out = out.split_off(out.len() - max);
    }
    out
}
