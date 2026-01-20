use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(author, version, about = "REMini wrapper orchestrating maintenance tasks", long_about = None)]
struct Args {
    /// Run all tasks (populate, embed, rethink, wander, health)
    #[arg(long)]
    all: bool,

    /// Comma-separated tasks to run (populate,embed,rethink,wander,health)
    #[arg(long)]
    tasks: Option<String>,

    /// Mark-type filter to pass to rethink (gem_rethink) task (e.g., correction,research)
    #[arg(long)]
    rethink_types: Option<String>,

    /// Dry run (propagated to child tasks via DRY_RUN=1)
    #[arg(long)]
    dry_run: bool,

    /// Show last report and exit
    #[arg(long)]
    report: bool,

    /// Timeout per task in seconds (default: 3600 = 1 hour)
    #[arg(long, default_value = "3600")]
    timeout: u64,
}

#[derive(Serialize, Debug)]
struct TaskResult {
    name: String,
    success: bool,
    duration_ms: u128,
    stdout: String,
    stderr: String,
}

#[derive(Serialize, Debug)]
struct SleepReport {
    run_timestamp: String,
    tasks_run: Vec<String>,
    summary: Summary,
    task_details: Vec<TaskResult>,
    duration_seconds: f64,
}

#[derive(Serialize, Debug, Default)]
struct Summary {
    tasks_succeeded: usize,
    tasks_failed: usize,
}

const REPORT_PATH: &str = "logs/remini_report.json";
const BIN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/target/release");

fn main() -> Result<()> {
    let args = Args::parse();

    if args.report {
        show_report()?;
        return Ok(());
    }

    let tasks = resolve_tasks(&args);
    let start = Instant::now();

    let mut results = Vec::new();
    let mut summary = Summary::default();

    for task in tasks.iter() {
        let (ok, dur, out, err) = run_task(
            task,
            args.dry_run,
            args.rethink_types.as_deref(),
            args.timeout,
        )?;
        if ok {
            summary.tasks_succeeded += 1;
        } else {
            summary.tasks_failed += 1;
        }
        results.push(TaskResult {
            name: task.clone(),
            success: ok,
            duration_ms: dur,
            stdout: out,
            stderr: err,
        });
    }

    let report = SleepReport {
        run_timestamp: chrono::Utc::now().to_rfc3339(),
        tasks_run: tasks,
        summary,
        task_details: results,
        duration_seconds: start.elapsed().as_secs_f64(),
    };

    persist_report(&report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);

    Ok(())
}

fn resolve_tasks(args: &Args) -> Vec<String> {
    if let Some(list) = &args.tasks {
        list.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else if args.all {
        vec![
            "populate".into(),
            "embed".into(),
            "rethink".into(),
            "wander".into(),
            "health".into(),
        ]
    } else {
        // default set
        vec![
            "populate".into(),
            "embed".into(),
            "rethink".into(),
            "wander".into(),
            "health".into(),
        ]
    }
}

fn run_task(
    task: &str,
    dry_run: bool,
    rethink_types: Option<&str>,
    timeout_secs: u64,
) -> Result<(bool, u128, String, String)> {
    let mut cmd_path = PathBuf::from(BIN_DIR);
    let mut envs = vec![];

    match task {
        "populate" => cmd_path.push("kg_populate"),
        "embed" => cmd_path.push("kg_embed"),
        "rethink" => {
            cmd_path.push("gem_rethink");
            if let Some(rt) = rethink_types {
                envs.push(("RETHINK_TYPES", rt));
            }
        }
        "wander" => {
            cmd_path.push("kg_wander");
        }
        "report" => {
            let start = Instant::now();
            let res = show_report();
            let dur = start.elapsed().as_millis();
            match res {
                Ok(()) => {
                    return Ok((true, dur, String::new(), String::new()));
                }
                Err(e) => {
                    return Ok((false, dur, String::new(), e.to_string()));
                }
            }
        }
        "health" => {
            let script = PathBuf::from("scripts/sm_health.sh");
            if !script.exists() {
                return Ok((
                    true,
                    0,
                    "health: scripts/sm_health.sh not found (skipped)".into(),
                    String::new(),
                ));
            }
            let start = Instant::now();
            let output = Command::new("bash")
                .arg(script)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .context("failed to start health script")?;
            let dur = start.elapsed().as_millis();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Ok((output.status.success(), dur, stdout, stderr));
        }
        other => {
            return Ok((false, 0, String::new(), format!("unknown task: {}", other)));
        }
    }

    let start = Instant::now();
    let mut command = Command::new(&cmd_path);
    if dry_run {
        envs.push(("DRY_RUN", "1"));
    }
    for (k, v) in envs.iter() {
        command.env(k, v);
    }

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start task {}", task))?;

    let timeout = Duration::from_secs(timeout_secs);
    let poll_interval = Duration::from_millis(500);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process finished
                let dur = start.elapsed().as_millis();
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        use std::io::Read;
                        let _ = s.read_to_string(&mut buf);
                        buf
                    })
                    .unwrap_or_default();
                let stderr = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        let mut buf = String::new();
                        use std::io::Read;
                        let _ = s.read_to_string(&mut buf);
                        buf
                    })
                    .unwrap_or_default();
                return Ok((status.success(), dur, stdout, stderr));
            }
            Ok(None) => {
                // Still running, check timeout
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let dur = start.elapsed().as_millis();
                    return Ok((
                        false,
                        dur,
                        String::new(),
                        format!("TIMEOUT: {} exceeded {}s limit", task, timeout_secs),
                    ));
                }
                thread::sleep(poll_interval);
            }
            Err(e) => {
                let dur = start.elapsed().as_millis();
                return Ok((false, dur, String::new(), format!("wait error: {}", e)));
            }
        }
    }
}

fn persist_report(report: &SleepReport) -> Result<()> {
    let path = PathBuf::from(REPORT_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(report)?;
    fs::write(&path, data)?;
    Ok(())
}

fn show_report() -> Result<()> {
    let path = PathBuf::from(REPORT_PATH);
    if !path.exists() {
        println!("No report found at {}", REPORT_PATH);
        return Ok(());
    }
    let data = fs::read_to_string(path)?;
    println!("{}", data);
    Ok(())
}
