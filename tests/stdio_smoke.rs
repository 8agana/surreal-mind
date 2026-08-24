//! STDIO-01: minimal stdio `initialize`/`tools/list` smoke test.
//!
//! Stdio is SurrealMind's default transport (`SURR_TRANSPORT` defaults to
//! `"stdio"` in `config.rs`; `main.rs` wires `rmcp::transport::stdio`
//! whenever no other transport is configured), so the rmcp 3.1.4 upgrade
//! gets a runtime witness here rather than leaving stdio at compile-only
//! coverage (upgrade doc decision D10). This spawns the actual compiled
//! binary as a subprocess over piped stdio — never the live process — sends
//! a newline-delimited JSON-RPC `initialize` then `tools/list` (the framing
//! `tests/test_stdio_persistence.sh` already exercises for this server),
//! and asserts a clean, protocol-only response.
//!
//! Gated the same way as the other `SurrealMindServer`-backed integration
//! tests: `SurrealMindServer::new` needs a live database even for stdio, so
//! this requires `RUN_DB_TESTS=1` against a verified disposable
//! namespace/database (never the production namespace — see the testing
//! doc's evidence rules) and is skipped otherwise.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Spawn a background thread that forwards each line of the child's stdout
/// to a channel, so the test thread can enforce a read timeout without
/// fighting `BufReader`'s blocking API.
fn spawn_line_reader(stdout: std::process::ChildStdout) -> mpsc::Receiver<std::io::Result<String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    if tx.send(Ok(line)).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    break;
                }
            }
        }
    });
    rx
}

fn recv_line_with_timeout(
    rx: &mpsc::Receiver<std::io::Result<String>>,
    timeout: Duration,
) -> anyhow::Result<String> {
    match rx.recv_timeout(timeout) {
        Ok(Ok(line)) => Ok(line),
        Ok(Err(e)) => Err(anyhow::anyhow!("failed to read child stdout: {e}")),
        Err(_) => Err(anyhow::anyhow!(
            "timed out after {:?} waiting for a line on child stdout",
            timeout
        )),
    }
}

#[test]
fn stdio01_initialize_and_list_tools_smoke() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping STDIO-01 smoke test - set RUN_DB_TESTS=1 to run");
        return;
    }

    let bin = env!("CARGO_BIN_EXE_surreal-mind");

    // Disposable config: stdio transport explicitly, no state file, quiet
    // stdout so protocol bytes are never mixed with log lines. Database
    // connectivity comes from the ambient environment, matching the other
    // RUN_DB_TESTS-gated tests in this suite (never the live/production
    // namespace — see the testing doc's evidence rules).
    let mut child = Command::new(bin)
        .env("SURR_TRANSPORT", "stdio")
        .env("MCP_NO_LOG", "1")
        .env_remove("SURR_WRITE_STATE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn surreal-mind binary for stdio smoke test");

    let mut stdin = child.stdin.take().expect("child stdin was not piped");
    let stdout = child.stdout.take().expect("child stdout was not piped");
    let lines = spawn_line_reader(stdout);

    let timeout = Duration::from_secs(15);

    // 1. initialize
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "stdio-smoke-test", "version": "0.0.0"}
        }
    });
    writeln!(stdin, "{}", init_request).expect("failed to write initialize request");
    stdin.flush().ok();

    let init_line = recv_line_with_timeout(&lines, timeout).expect("no initialize response line");
    let init_response: serde_json::Value =
        serde_json::from_str(init_line.trim()).unwrap_or_else(|e| {
            panic!(
                "initialize response was not valid JSON (non-protocol bytes on stdout?): {e}\nline: {init_line:?}"
            )
        });
    assert_eq!(
        init_response.get("id"),
        Some(&serde_json::json!(1)),
        "initialize response id must echo the request id"
    );
    let negotiated = init_response
        .get("result")
        .and_then(|r| r.get("protocolVersion"))
        .and_then(|v| v.as_str())
        .expect("initialize response must carry a negotiated protocolVersion");
    assert!(
        !negotiated.is_empty(),
        "negotiated protocol version must not be empty"
    );
    assert_eq!(
        init_response
            .get("result")
            .and_then(|r| r.get("capabilities"))
            .and_then(|c| c.get("tools"))
            .and_then(|t| t.get("listChanged")),
        Some(&serde_json::json!(false)),
        "tools.listChanged must serialize as explicit false over stdio too (PROTO-05)"
    );

    // 2. notifications/initialized (required by the MCP lifecycle before
    // further requests; sent as a notification, no response expected)
    let initialized_notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    writeln!(stdin, "{}", initialized_notification)
        .expect("failed to write initialized notification");
    stdin.flush().ok();

    // 3. tools/list
    let list_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });
    writeln!(stdin, "{}", list_request).expect("failed to write tools/list request");
    stdin.flush().ok();

    let list_line = recv_line_with_timeout(&lines, timeout).expect("no tools/list response line");
    let list_response: serde_json::Value =
        serde_json::from_str(list_line.trim()).unwrap_or_else(|e| {
            panic!(
                "tools/list response was not valid JSON (non-protocol bytes on stdout?): {e}\nline: {list_line:?}"
            )
        });
    assert_eq!(
        list_response.get("id"),
        Some(&serde_json::json!(2)),
        "tools/list response id must echo the request id"
    );
    let tools = list_response
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .expect("tools/list response must carry a tools array");
    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();
    assert_eq!(
        tool_names.len(),
        16,
        "stdio tools/list must return exactly 16 tools, got: {tool_names:?}"
    );
    assert!(
        tool_names.contains(&"journal"),
        "stdio tools/list must include journal, got: {tool_names:?}"
    );

    // Tear down the child cleanly; drop stdin to signal EOF, then kill if
    // it does not exit promptly.
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}
