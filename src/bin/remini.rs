use std::fs;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(author, version, about = "REMini wrapper orchestrating maintenance tasks", long_about = None)]
struct Args {
    /// Run all tasks (populate, embed, rethink, consolidate, wander, health)
    #[arg(long)]
    all: bool,

    /// Comma-separated tasks to run (populate,embed,rethink,consolidate,wander,health)
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

    /// Path to write/read the run report (default: logs/remini_report.json,
    /// overridable via REMINI_REPORT_PATH; this flag takes precedence over
    /// the env var)
    #[arg(long)]
    report_path: Option<PathBuf>,
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
const TASK_OUTPUT_LIMIT: usize = 64 * 1024;
const TASK_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[cfg(test)]
static FORCE_TRY_WAIT_ERROR: AtomicBool = AtomicBool::new(false);

/// Resolve the effective report path: --report-path flag, then
/// REMINI_REPORT_PATH env var, then the hardcoded default -- so the
/// nightly scheduled run (which sets neither) is byte-for-byte unchanged.
fn resolve_report_path(cli_path: &Option<PathBuf>) -> PathBuf {
    if let Some(p) = cli_path {
        return p.clone();
    }
    if let Ok(env_path) = std::env::var("REMINI_REPORT_PATH") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    PathBuf::from(REPORT_PATH)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let report_path = resolve_report_path(&args.report_path);

    if args.report {
        show_report(&report_path)?;
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
            &report_path,
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

    persist_report(&report, &report_path)?;
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
            "consolidate".into(),
            "wander".into(),
            "health".into(),
        ]
    } else {
        // default set
        vec![
            "populate".into(),
            "embed".into(),
            "rethink".into(),
            "consolidate".into(),
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
    report_path: &Path,
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
        "consolidate" => {
            cmd_path.push("kg_consolidate");
            // Nightly REMini should perform full cleanup, including safe loser deletion.
            envs.push(("CONSOLIDATE_DELETE", "1"));
        }
        "wander" => {
            cmd_path.push("kg_wander");
        }
        "report" => {
            let start = Instant::now();
            let res = show_report(report_path);
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
            if dry_run {
                return Ok((
                    true,
                    0,
                    "[DRY_RUN] health check skipped, DB read-only mode requires live opt-in".into(),
                    String::new(),
                ));
            }
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

    command.process_group(0);
    let output = run_owned_task(command, task, Duration::from_secs(timeout_secs))?;
    let dur = start.elapsed().as_millis();
    let mut stderr = output.stderr;
    if let Some(error) = output.supervisor_error.as_deref() {
        append_diagnostic(&mut stderr, error);
    }
    Ok((
        output.status.as_ref().is_some_and(ExitStatus::success)
            && !output.timed_out
            && output.supervisor_error.is_none(),
        dur,
        output.stdout,
        stderr,
    ))
}

#[derive(Default)]
struct CapturedStream {
    retained: Vec<u8>,
    discarded: u64,
}

struct OwnedTaskOutput {
    status: Option<ExitStatus>,
    stdout: String,
    stderr: String,
    timed_out: bool,
    supervisor_error: Option<String>,
}

fn drain_stream<R: Read>(mut reader: R) -> std::io::Result<CapturedStream> {
    let mut retained = Vec::with_capacity(TASK_OUTPUT_LIMIT.min(8192));
    let mut discarded = 0u64;
    let mut buffer = [0u8; 8192];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }

        let remaining = TASK_OUTPUT_LIMIT.saturating_sub(retained.len());
        let keep = remaining.min(read);
        retained.extend_from_slice(&buffer[..keep]);
        discarded += (read - keep) as u64;
    }

    Ok(CapturedStream {
        retained,
        discarded,
    })
}

fn render_stream(captured: CapturedStream) -> String {
    let mut output = String::from_utf8_lossy(&captured.retained).into_owned();
    if captured.discarded > 0 {
        append_diagnostic(
            &mut output,
            &format!(
                "[OUTPUT_TRUNCATED: {} bytes discarded after {} byte retention limit]",
                captured.discarded, TASK_OUTPUT_LIMIT
            ),
        );
    }
    output
}

fn append_diagnostic(output: &mut String, diagnostic: &str) {
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str(diagnostic);
}

fn run_owned_task(mut command: Command, task: &str, timeout: Duration) -> Result<OwnedTaskOutput> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start task {}", task))?;

    let stdout_reader = child
        .stdout
        .take()
        .context("task stdout pipe unavailable after spawn")?;
    let stderr_reader = child
        .stderr
        .take()
        .context("task stderr pipe unavailable after spawn")?;
    let stdout_thread = thread::spawn(move || drain_stream(stdout_reader));
    let stderr_thread = thread::spawn(move || drain_stream(stderr_reader));
    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let mut supervisor_error = None;
    let mut status = None;
    loop {
        match poll_child(&mut child) {
            Ok(Some(exit_status)) => {
                status = Some(exit_status);
                break;
            }
            Ok(None) if Instant::now() >= deadline => {
                timed_out = true;
                break;
            }
            Ok(None) => thread::sleep(TASK_POLL_INTERVAL),
            Err(error) => {
                supervisor_error = Some(format!("SUPERVISOR: wait error: {}", error));
                break;
            }
        }
    }

    let pgid = child.id() as i32;
    let kill_result = unsafe { libc::kill(-pgid, libc::SIGKILL) };
    if kill_result != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            append_supervisor_error(
                &mut supervisor_error,
                format!(
                    "SUPERVISOR: failed to signal process group {}: {}",
                    pgid, error
                ),
            );
        }
    }
    if status.is_none() {
        match child.wait() {
            Ok(exit_status) => status = Some(exit_status),
            Err(error) => append_supervisor_error(
                &mut supervisor_error,
                format!("SUPERVISOR: failed to reap task: {}", error),
            ),
        }
    }

    let stdout = match stdout_thread.join() {
        Ok(Ok(captured)) => captured,
        Ok(Err(error)) => {
            append_supervisor_error(
                &mut supervisor_error,
                format!("SUPERVISOR: stdout drain failed: {}", error),
            );
            CapturedStream::default()
        }
        Err(_) => {
            append_supervisor_error(
                &mut supervisor_error,
                "SUPERVISOR: stdout drain thread panicked".to_string(),
            );
            CapturedStream::default()
        }
    };
    let stderr = match stderr_thread.join() {
        Ok(Ok(captured)) => captured,
        Ok(Err(error)) => {
            append_supervisor_error(
                &mut supervisor_error,
                format!("SUPERVISOR: stderr drain failed: {}", error),
            );
            CapturedStream::default()
        }
        Err(_) => {
            append_supervisor_error(
                &mut supervisor_error,
                "SUPERVISOR: stderr drain thread panicked".to_string(),
            );
            CapturedStream::default()
        }
    };
    let mut stdout = render_stream(stdout);
    let mut stderr = render_stream(stderr);

    if timed_out {
        append_diagnostic(
            &mut stderr,
            &format!("TIMEOUT: {} exceeded {}s limit", task, timeout.as_secs()),
        );
    }

    Ok(OwnedTaskOutput {
        status,
        stdout: std::mem::take(&mut stdout),
        stderr: std::mem::take(&mut stderr),
        timed_out,
        supervisor_error,
    })
}

fn append_supervisor_error(target: &mut Option<String>, message: String) {
    match target {
        Some(existing) => {
            existing.push_str("; ");
            existing.push_str(&message);
        }
        None => *target = Some(message),
    }
}

fn poll_child(child: &mut Child) -> std::io::Result<Option<ExitStatus>> {
    #[cfg(test)]
    if FORCE_TRY_WAIT_ERROR.swap(false, Ordering::SeqCst) {
        return Err(std::io::Error::other("forced try_wait failure"));
    }
    child.try_wait()
}

fn persist_report(report: &SleepReport, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(report)?;
    fs::write(path, data)?;
    Ok(())
}

fn show_report(path: &Path) -> Result<()> {
    if !path.exists() {
        println!("No report found at {}", path.display());
        return Ok(());
    }
    let data = fs::read_to_string(path)?;
    println!("{}", data);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn try_wait_error_preserves_task_result_and_report() {
        let dir = tempfile::tempdir().unwrap();
        let child_script = dir.path().join("child.sh");
        let ready_path = dir.path().join("ready");
        std::fs::write(
            &child_script,
            format!(
                "#!/bin/sh\nprintf ready > {}\nprintf 'before-supervisor-error stdout\\n'\nprintf 'before-supervisor-error stderr\\n' >&2\nsleep 60\\n",
                ready_path.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&child_script).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&child_script, permissions).unwrap();

        let mut command = Command::new(&child_script);
        command.process_group(0);
        let trigger_path = ready_path.clone();
        let trigger = std::thread::spawn(move || {
            for _ in 0..100 {
                if trigger_path.exists() {
                    FORCE_TRY_WAIT_ERROR.store(true, Ordering::SeqCst);
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("child did not publish try_wait trigger");
        });
        let output = run_owned_task(command, "fixture", Duration::from_secs(5)).unwrap();
        trigger.join().unwrap();
        FORCE_TRY_WAIT_ERROR.store(false, Ordering::SeqCst);

        assert!(!output.status.as_ref().is_some_and(ExitStatus::success));
        assert!(!output.timed_out);
        assert!(output.stdout.contains("before-supervisor-error stdout"));
        assert!(output.stderr.contains("before-supervisor-error stderr"));
        assert!(
            output
                .supervisor_error
                .as_deref()
                .is_some_and(|error| error.contains("forced try_wait failure"))
        );

        let report_path = dir.path().join("report.json");
        let report = SleepReport {
            run_timestamp: "synthetic".to_string(),
            tasks_run: vec!["populate".to_string()],
            summary: Summary {
                tasks_succeeded: 0,
                tasks_failed: 1,
            },
            task_details: vec![TaskResult {
                name: "populate".to_string(),
                success: false,
                duration_ms: 1,
                stdout: output.stdout,
                stderr: format!("{}; {}", output.stderr, output.supervisor_error.unwrap()),
            }],
            duration_seconds: 0.001,
        };
        persist_report(&report, &report_path).unwrap();
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(report_path).unwrap()).unwrap();
        assert_eq!(saved["summary"]["tasks_failed"], 1);
        assert_eq!(saved["task_details"][0]["success"], false);
        assert!(
            saved["task_details"][0]["stdout"]
                .as_str()
                .unwrap()
                .contains("before-supervisor-error stdout")
        );
        assert!(
            saved["task_details"][0]["stderr"]
                .as_str()
                .unwrap()
                .contains("SUPERVISOR: wait error")
        );
    }
}
