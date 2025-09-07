use std::io;
use std::process::Command;
use std::time::{Duration, Instant};

use crossterm::{event, execute, terminal};
use ratatui::prelude::*;
use ratatui::widgets::*;
use reqwest::blocking::Client;

#[derive(Default, Clone)]
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
}

fn main() -> anyhow::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut status = gather_status(None);
    let mut last_refresh = Instant::now();

    loop {
        terminal.draw(|f| ui(f, &status))?;

        if last_refresh.elapsed() >= Duration::from_secs(2) {
            status = gather_status(Some(&status));
            last_refresh = Instant::now();
        }

        if event::poll(Duration::from_millis(200))? {
            if let event::Event::Key(k) = event::read()? {
                use crossterm::event::{KeyCode, KeyModifiers};
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
                    KeyCode::Char('a') => {
                        status.use_header_auth = !status.use_header_auth;
                    }
                    KeyCode::PageUp => status.log_scroll = status.log_scroll.saturating_add(10),
                    KeyCode::PageDown => status.log_scroll = status.log_scroll.saturating_sub(10),
                    KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => break,
                    _ => {}
                }
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
            Constraint::Length(9),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(f.size());

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

    // Logs pane
    let logs_lines: Vec<Line> = if s.combined_log_tail.is_empty() {
        vec![Line::raw("(no logs yet)")]
    } else {
        let h = chunks[2].height as usize;
        s.combined_log_tail
            .iter()
            .rev()
            .skip(s.log_scroll as usize)
            .take(h.saturating_sub(2))
            .rev()
            .map(|l| Line::raw(l.clone()))
            .collect()
    };
    let logs_p = Paragraph::new(logs_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Recent Logs (PgUp/PgDn)"),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(logs_p, chunks[2]);

    let help = Paragraph::new(vec![
        Line::raw("Keys: q/Esc quit • r restart server • f/g start/stop cloudflared • y/Y copy URL/token • a toggle auth header"),
        Line::raw(format!("Auth mode: {}", if s.use_header_auth { "Authorization header" } else { "query token" })),
    ])
    .block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(help, chunks[3]);
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

    // /metrics for total_requests
    if !token.is_empty() {
        if let Some(m) =
            http_json_auth_mode("http://127.0.0.1:8787/metrics", &token, st.use_header_auth)
        {
            if let Some(t) = m.get("total_requests").and_then(|v| v.as_u64()) {
                st.total_requests = Some(t);
            }
            if let Some(prev_t) = prev.and_then(|p| p.total_requests) {
                if let Some(cur) = st.total_requests {
                    let dt = 2.0_f64;
                    st.rps = Some((cur.saturating_sub(prev_t) as f64) / dt);
                }
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

    // Merge logs
    let logs_dir = format!("{}/Library/Logs", std::env::var("HOME").unwrap_or_default());
    st.combined_log_tail = merge_logs_chrono(
        &format!("{}/surreal-mind.out.log", logs_dir),
        &format!("{}/surreal-mind.err.log", logs_dir),
        &format!("{}/cloudflared-tunnel.out.log", logs_dir),
        &format!("{}/cloudflared-tunnel.err.log", logs_dir),
        400,
    );
    st
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

fn http_json_auth_mode(url: &str, tok: &str, header: bool) -> Option<serde_json::Value> {
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
    resp.json::<serde_json::Value>().ok()
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

fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    let mut child = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        write!(stdin, "{text}")?;
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
    if let Some(first) = line.split_whitespace().next() {
        if let Ok(dt) =
            time::OffsetDateTime::parse(first, &time::format_description::well_known::Rfc3339)
        {
            return Some(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(dt.unix_timestamp() as u64),
            );
        }
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
