//! maintenance_ops tool handler for archival and cleanup operations

use crate::error::{Result, SurrealMindError};
use crate::indexes::{IndexHealth, TableInfo, get_expected_indexes};
use crate::server::SurrealMindServer;
// corrections tool handler is in scope via SurrealMindServer impl; no direct import needed
use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde_json::json;
use std::fs;
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_MAINTENANCE_TIMEOUT_MS: u64 = 1_800_000;
const MIN_MAINTENANCE_TIMEOUT_MS: u64 = 100;
const MAX_MAINTENANCE_TIMEOUT_MS: u64 = 3_600_000;
const MAINTENANCE_OUTPUT_LIMIT: usize = 64 * 1024;
const READER_POLL_MS: i32 = 25;
const CLEANUP_GRACE: Duration = Duration::from_secs(1);
const CANCEL_PENDING: u8 = 0;
const CANCEL_SPAWNING: u8 = 1;
const CANCEL_RUNNING: u8 = 2;
const CANCELLED: u8 = 3;
const KG_WANDER_PARENT_SUPERVISED: &str = "KG_WANDER_PARENT_SUPERVISED";

/// Parameters for the maintenance_ops tool
#[derive(Debug, serde::Deserialize)]
pub struct MaintenanceParams {
    pub subcommand: String,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::deserializers::de_option_u64_forgiving"
    )]
    pub limit: Option<u64>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default)]
    pub tasks: Option<String>,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub rethink_types: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

fn normalize_thought_record_key(raw_id: &str) -> String {
    let trimmed = raw_id.trim();
    let without_table = trimmed.strip_prefix("thoughts:").unwrap_or(trimmed);
    without_table.trim_matches('`').to_string()
}

fn reembed_stats_json(stats: &crate::ReembedStats, dry_run: bool) -> serde_json::Value {
    json!({
        "expected_dim": stats.expected_dim,
        "batch_size": stats.batch_size,
        "processed": stats.processed,
        "updated": stats.updated,
        "skipped": stats.skipped,
        "missing": stats.missing,
        "mismatched": stats.mismatched,
        "no_match": stats.no_match,
        "failed": stats.failed,
        "dry_run": dry_run
    })
}

fn reembed_kg_stats_json(stats: &crate::ReembedKgStats, dry_run: bool) -> serde_json::Value {
    json!({
        "message": "KG reembed completed",
        "expected_dim": stats.expected_dim,
        "provider": stats.provider,
        "model": stats.model,
        "entities": {
            "updated": stats.entities_updated,
            "skipped": stats.entities_skipped,
            "missing": stats.entities_missing,
            "mismatched": stats.entities_mismatched,
            "no_match": stats.entities_no_match,
            "failed": stats.entities_failed
        },
        "observations": {
            "updated": stats.observations_updated,
            "skipped": stats.observations_skipped,
            "missing": stats.observations_missing,
            "mismatched": stats.observations_mismatched,
            "no_match": stats.observations_no_match,
            "failed": stats.observations_failed
        },
        "edges": {
            "updated": stats.edges_updated,
            "skipped": stats.edges_skipped,
            "missing": stats.edges_missing,
            "mismatched": stats.edges_mismatched,
            "no_match": stats.edges_no_match,
            "failed": stats.edges_failed
        },
        "dry_run": dry_run
    })
}

fn maintenance_timeout(timeout_ms: Option<u64>) -> Result<Duration> {
    let timeout_ms = timeout_ms.unwrap_or(DEFAULT_MAINTENANCE_TIMEOUT_MS);
    if !(MIN_MAINTENANCE_TIMEOUT_MS..=MAX_MAINTENANCE_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(SurrealMindError::InvalidParams {
            message: format!(
                "timeout_ms must be between {} and {} milliseconds",
                MIN_MAINTENANCE_TIMEOUT_MS, MAX_MAINTENANCE_TIMEOUT_MS
            ),
        });
    }
    Ok(Duration::from_millis(timeout_ms))
}

#[derive(Clone, Debug, Default)]
struct MaintenancePaths {
    bin_dir: Option<PathBuf>,
    script_dir: Option<PathBuf>,
}

fn maintenance_binary_path(bin: &str, paths: &MaintenancePaths) -> PathBuf {
    if let Some(dir) = &paths.bin_dir {
        return dir.join(bin);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/release")
        .join(bin)
}

fn maintenance_script_path(script: &str, paths: &MaintenancePaths) -> PathBuf {
    if let Some(dir) = &paths.script_dir {
        let name = Path::new(script)
            .file_name()
            .unwrap_or_else(|| Path::new(script).as_os_str());
        return dir.join(name);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join(script)
}

struct CancelOnDrop {
    state: Arc<AtomicU8>,
    armed: bool,
}

impl CancelOnDrop {
    fn new(state: Arc<AtomicU8>) -> Self {
        Self { state, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.state.store(CANCELLED, Ordering::Release);
        }
    }
}

struct CapturedOutput {
    bytes: Vec<u8>,
    discarded: usize,
    incomplete: bool,
}

struct ReaderControl {
    stop: AtomicBool,
    stop_at: Mutex<Option<Instant>>,
}

impl ReaderControl {
    fn new() -> Self {
        Self {
            stop: AtomicBool::new(false),
            stop_at: Mutex::new(None),
        }
    }

    fn arm_cleanup_deadline(&self) {
        *self.stop_at.lock().expect("reader control mutex poisoned") =
            Some(Instant::now() + CLEANUP_GRACE);
    }

    fn should_stop(&self) -> bool {
        if self.stop.load(Ordering::Acquire) {
            return true;
        }
        self.stop_at
            .lock()
            .expect("reader control mutex poisoned")
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn deadline_expired(&self) -> bool {
        self.stop_at
            .lock()
            .expect("reader control mutex poisoned")
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn remaining(&self) -> Option<Duration> {
        self.stop_at
            .lock()
            .expect("reader control mutex poisoned")
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }
}

fn drain_output<R: Read + AsRawFd>(
    mut reader: R,
    control: Arc<ReaderControl>,
) -> io::Result<CapturedOutput> {
    let mut bytes = Vec::with_capacity(MAINTENANCE_OUTPUT_LIMIT);
    let mut discarded = 0;
    let mut incomplete = false;
    let mut buffer = [0u8; 8192];
    loop {
        if control.should_stop() {
            incomplete = control.deadline_expired();
            break;
        }
        let mut pollfd = libc::pollfd {
            fd: reader.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let poll_ms = control
            .remaining()
            .map(|remaining| READER_POLL_MS.min(remaining.as_millis() as i32))
            .unwrap_or(READER_POLL_MS)
            .max(1);
        let ready = unsafe { libc::poll(&mut pollfd, 1, poll_ms) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(error);
        }
        if ready == 0 {
            continue;
        }
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                let retained = (MAINTENANCE_OUTPUT_LIMIT - bytes.len()).min(read);
                bytes.extend_from_slice(&buffer[..retained]);
                discarded += read - retained;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(CapturedOutput {
        bytes,
        discarded,
        incomplete,
    })
}

fn render_output(mut captured: CapturedOutput, stream: &str) -> Vec<u8> {
    if captured.discarded > 0 || captured.incomplete {
        let marker = format!(
            "\n[OUTPUT_TRUNCATED stream={} discarded={} retained_limit={} incomplete={}]\n",
            stream, captured.discarded, MAINTENANCE_OUTPUT_LIMIT, captured.incomplete
        );
        captured.bytes.extend_from_slice(marker.as_bytes());
    }
    captured.bytes
}

fn append_diagnostic(bytes: &mut Vec<u8>, diagnostic: impl AsRef<str>) {
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(diagnostic.as_ref().as_bytes());
    bytes.push(b'\n');
}

fn process_group_has_live_member(pgid: i32, leader_pid: i32) -> io::Result<bool> {
    let mut scan = Command::new("ps")
        .args(["-axo", "pid=,pgid=,stat="])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = scan
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("process-group scan stdout pipe was not available"))?;
    let reader_control = Arc::new(ReaderControl::new());
    let reader_control_for_thread = Arc::clone(&reader_control);
    let reader = thread::spawn(move || drain_output(stdout, reader_control_for_thread));
    let deadline = Instant::now() + Duration::from_millis(100);
    let mut status = None;
    while Instant::now() < deadline {
        if let Some(exit_status) = scan.try_wait()? {
            status = Some(exit_status);
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    if status.is_none() {
        reader_control.arm_cleanup_deadline();
        scan.kill()?;
        status = Some(scan.wait()?);
    }
    reader_control.arm_cleanup_deadline();
    let captured = reader
        .join()
        .map_err(|_| io::Error::other("process-group scan reader panicked"))??;
    let status = status.expect("process-group scan status set before reader join");
    if !status.success() {
        return Err(io::Error::other(format!(
            "ps exited with status {}",
            status
        )));
    }
    if captured.incomplete || captured.discarded > 0 {
        return Err(io::Error::other("process-group scan output was incomplete"));
    }
    let output = String::from_utf8_lossy(&captured.bytes);
    let mut malformed = false;
    for line in output.lines() {
        let mut fields = line.split_whitespace();
        let Some(pid) = fields.next().and_then(|value| value.parse::<i32>().ok()) else {
            if !line.trim().is_empty() {
                malformed = true;
            }
            continue;
        };
        let Some(group) = fields.next().and_then(|value| value.parse::<i32>().ok()) else {
            malformed = true;
            continue;
        };
        let Some(state) = fields.next() else {
            malformed = true;
            continue;
        };
        if group == pgid && pid != leader_pid && !state.starts_with('Z') {
            return Ok(true);
        }
    }
    if malformed {
        return Err(io::Error::other("process-group scan output was malformed"));
    }
    Ok(false)
}

fn kill_process_group(pgid: i32, leader_pid: i32, leader_exited: bool) -> (bool, Option<String>) {
    let result = unsafe { libc::kill(-pgid, libc::SIGKILL) };
    if result == 0 {
        return (true, None);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        (false, None)
    } else if leader_exited && error.raw_os_error() == Some(libc::EPERM) {
        match process_group_has_live_member(pgid, leader_pid) {
            Ok(false) => (false, None),
            Ok(true) => (
                false,
                Some(format!("failed to kill process group {}: {}", pgid, error)),
            ),
            Err(scan_error) => (
                false,
                Some(format!(
                    "failed to kill process group {}: {}; process-group scan failed: {}",
                    pgid, error, scan_error
                )),
            ),
        }
    } else {
        (
            false,
            Some(format!("failed to kill process group {}: {}", pgid, error)),
        )
    }
}

fn leader_exited_without_reap(pid: i32) -> io::Result<bool> {
    let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info.si_pid == pid)
}

fn reap_after_termination(child: &mut Child) -> (Option<ExitStatus>, Option<String>) {
    let deadline = Instant::now() + CLEANUP_GRACE;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return (Some(status), None),
            Ok(None) => {}
            Err(error) => {
                return (
                    None,
                    Some(format!("try_wait during cleanup failed: {}", error)),
                );
            }
        }
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(READER_POLL_MS as u64).min(deadline - now));
    }

    let kill_error = child
        .kill()
        .err()
        .map(|error| format!("failed to kill child directly: {}", error));
    let force_deadline = Instant::now() + CLEANUP_GRACE;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return (Some(status), kill_error),
            Ok(None) => {}
            Err(error) => {
                return (
                    None,
                    Some(match kill_error {
                        Some(existing) => {
                            format!("{}; try_wait after direct kill failed: {}", existing, error)
                        }
                        None => format!("try_wait after direct kill failed: {}", error),
                    }),
                );
            }
        }
        let now = Instant::now();
        if now >= force_deadline {
            return (
                None,
                Some(match kill_error {
                    Some(existing) => {
                        format!("{}; child was not reaped before cleanup deadline", existing)
                    }
                    None => "child was not reaped before cleanup deadline".to_string(),
                }),
            );
        }
        thread::sleep(Duration::from_millis(READER_POLL_MS as u64).min(force_deadline - now));
    }
}

fn run_maintenance_command_blocking(
    mut cmd: Command,
    timeout: Duration,
    state: Arc<AtomicU8>,
) -> io::Result<Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.process_group(0);
    if state
        .compare_exchange(
            CANCEL_PENDING,
            CANCEL_SPAWNING,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        return Ok(Output {
            status: ExitStatus::from_raw(1),
            stdout: Vec::new(),
            stderr: b"SUPERVISOR: maintenance command cancelled before spawn\n".to_vec(),
        });
    }
    let mut child = cmd.spawn()?;
    let _ = state.compare_exchange(
        CANCEL_SPAWNING,
        CANCEL_RUNNING,
        Ordering::AcqRel,
        Ordering::Acquire,
    );
    let pgid = child.id() as i32;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("maintenance stdout pipe was not available"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("maintenance stderr pipe was not available"))?;
    let reader_control = Arc::new(ReaderControl::new());
    let stdout_reader = {
        let control = Arc::clone(&reader_control);
        thread::spawn(move || drain_output(stdout, control))
    };
    let stderr_reader = {
        let control = Arc::clone(&reader_control);
        thread::spawn(move || drain_output(stderr, control))
    };
    let deadline = Instant::now() + timeout;
    let mut status: Option<ExitStatus> = None;
    let mut timed_out = false;
    let mut supervisor_error = None;
    let mut leader_exited = false;

    loop {
        if state.load(Ordering::Acquire) == CANCELLED {
            supervisor_error = Some("maintenance command cancelled".to_string());
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            break;
        }
        match leader_exited_without_reap(pgid) {
            Ok(true) => {
                leader_exited = true;
                break;
            }
            Ok(false) => {}
            Err(error) => {
                supervisor_error = Some(format!("waitid failed: {}", error));
                break;
            }
        }
        let remaining = deadline - Instant::now();
        thread::sleep(Duration::from_millis(READER_POLL_MS as u64).min(remaining));
    }

    let (group_killed, kill_error) = {
        let (killed, kill_error) = kill_process_group(pgid, pgid, leader_exited);
        reader_control.arm_cleanup_deadline();
        let (reaped_status, reap_error) = reap_after_termination(&mut child);
        if status.is_none() {
            status = reaped_status;
        }
        if let Some(error) = reap_error {
            supervisor_error = Some(match supervisor_error {
                Some(existing) => format!("{}; {}", existing, error),
                None => error,
            });
        }
        (killed, kill_error)
    };

    let stdout = stdout_reader
        .join()
        .map_err(|_| io::Error::other("stdout reader panicked"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| io::Error::other("stderr reader panicked"))??;
    let incomplete_capture = stdout.incomplete || stderr.incomplete;
    let mut stdout = render_output(stdout, "stdout");
    let mut stderr = render_output(stderr, "stderr");
    if timed_out {
        append_diagnostic(
            &mut stderr,
            format!(
                "TIMEOUT: maintenance command exceeded {} ms",
                timeout.as_millis()
            ),
        );
    }
    let failed = timed_out
        || supervisor_error.is_some()
        || kill_error.is_some()
        || incomplete_capture
        || (leader_exited && group_killed);
    if let Some(ref error) = supervisor_error {
        append_diagnostic(&mut stderr, format!("SUPERVISOR: {}", error));
    }
    if let Some(error) = kill_error {
        append_diagnostic(&mut stderr, format!("SUPERVISOR: {}", error));
    }
    let status = if failed {
        ExitStatus::from_raw(1)
    } else {
        status.unwrap_or_else(|| ExitStatus::from_raw(1))
    };
    Ok(Output {
        status,
        stdout: std::mem::take(&mut stdout),
        stderr,
    })
}

async fn run_maintenance_command(cmd: Command, timeout: Duration) -> io::Result<Output> {
    let state = Arc::new(AtomicU8::new(CANCEL_PENDING));
    let worker_state = Arc::clone(&state);
    let mut cancel_on_drop = CancelOnDrop::new(state);
    let result = tokio::task::spawn_blocking(move || {
        run_maintenance_command_blocking(cmd, timeout, worker_state)
    })
    .await
    .map_err(|error| io::Error::other(format!("maintenance supervisor failed: {}", error)))?;
    cancel_on_drop.disarm();
    result
}

/// True only when `embedding`'s length exactly matches the configured embedding
/// dimension. Used to gate embed_pending writes so a wrong-dimension embedding
/// (e.g. a mid-migration provider mismatch) is never persisted as `complete`.
impl SurrealMindServer {
    /// Handle health check for database indexes
    async fn handle_health_check_indexes(&self, _dry_run: bool) -> Result<CallToolResult> {
        let mut results = vec![];

        for table_def in get_expected_indexes() {
            // Get current indexes for table
            // Use serde_json::Value since INFO FOR TABLE returns a complex object
            let info: Vec<serde_json::Value> = self
                .db
                .query(format!("INFO FOR TABLE {}", table_def.table))
                .await?
                .take(0)?;

            let raw_info = info.first().ok_or_else(|| SurrealMindError::Internal {
                message: format!("No info returned for table {}", table_def.table),
            })?;

            // Deserialize into TableInfo manually
            let table_info: TableInfo = serde_json::from_value(raw_info.clone()).map_err(|e| {
                SurrealMindError::Internal {
                    message: format!("Failed to parse table info for {}: {}", table_def.table, e),
                }
            })?;

            // Get expected index names (both required and optional)
            let mut expected = table_def
                .required
                .iter()
                .map(|idx| idx.to_definition().replace("{table}", &table_def.table))
                .collect::<Vec<_>>();
            let optional = table_def
                .optional
                .iter()
                .map(|idx| idx.to_definition().replace("{table}", &table_def.table))
                .collect::<Vec<_>>();
            expected.extend(optional);

            // Get present indexes
            let present = table_info.indexes.values().cloned().collect::<Vec<_>>();

            // Calculate missing (required only)
            let required_defs = table_def
                .required
                .iter()
                .map(|idx| idx.to_definition().replace("{table}", &table_def.table))
                .collect::<Vec<_>>();
            let missing = required_defs
                .iter()
                .filter(|req| !present.contains(req))
                .cloned()
                .collect::<Vec<_>>();

            results.push(IndexHealth {
                table: table_def.table.clone(),
                expected,
                present,
                missing,
            });
        }

        // Group by table for cleaner output
        let report = json!({
            "tables": results.iter().map(|r| json!({
                "table": r.table,
                "expected": r.expected,
                "present": r.present,
                "missing": r.missing,
                "status": if r.missing.is_empty() { "ok" } else { "missing_required" }
            })).collect::<Vec<_>>()
        });

        Ok(CallToolResult::structured(report))
    }
    /// Handle the maintenance_ops tool call
    pub async fn handle_maintenance_ops(
        &self,
        request: CallToolRequestParams,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: MaintenanceParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        let dry_run = params.dry_run.unwrap_or(false);
        let limit = params.limit.unwrap_or(100) as usize;
        let format = params.format.unwrap_or_else(|| "json".to_string());
        let output_dir = params.output_dir.unwrap_or_else(|| "./archive".to_string());
        let timeout = maintenance_timeout(params.timeout_ms)?;

        tracing::info!(
            "maintenance_ops called: subcommand={}, dry_run={}, limit={}, format={}, output_dir={}",
            params.subcommand,
            dry_run,
            limit,
            format,
            output_dir
        );

        match params.subcommand.as_str() {
            "list_removal_candidates" => self.handle_list_removal_candidates(limit, dry_run).await,
            "export_removals" => {
                self.handle_export_removals(limit, &format, &output_dir, dry_run)
                    .await
            }
            "finalize_removal" => self.handle_finalize_removal(limit, dry_run).await,
            "health_check_embeddings" => self.handle_health_check_embeddings(dry_run).await,
            "health_check_indexes" => self.handle_health_check_indexes(dry_run).await,
            "reembed" => self.handle_reembed(limit, dry_run).await,
            "reembed_kg" => self.handle_reembed_kg(limit, dry_run).await,
            "embed_pending" => self.handle_embed_pending(limit, dry_run).await,
            "ensure_continuity_fields" => self.handle_ensure_continuity_fields(dry_run).await,
            "echo_config" => self.handle_echo_config().await,
            "corrections" => {
                self.handle_corrections_bridge(limit, params.target_id.clone())
                    .await
            }
            "rethink" => {
                let mut envs: Vec<(String, String)> = vec![];
                if let Some(rt) = params.rethink_types.clone() {
                    envs.push(("RETHINK_TYPES".into(), rt));
                }
                Self::handle_spawn_binary("gem_rethink", dry_run, &envs, timeout).await
            }
            "consolidate" => {
                let envs: Vec<(String, String)> =
                    vec![("CONSOLIDATE_LIMIT".into(), limit.to_string())];
                Self::handle_spawn_binary("kg_consolidate", dry_run, &envs, timeout).await
            }
            "populate" => {
                Self::handle_spawn_binary("kg_populate", dry_run, &Vec::new(), timeout).await
            }
            "embed" => Self::handle_spawn_binary("kg_embed", dry_run, &Vec::new(), timeout).await,
            "wander" => Self::handle_spawn_binary("kg_wander", dry_run, &Vec::new(), timeout).await,
            "health" => Self::handle_spawn_script("scripts/sm_health.sh", dry_run, timeout).await,
            "report" => self.handle_report().await,
            "tasks" => {
                self.handle_tasks(params.tasks.clone(), dry_run, timeout)
                    .await
            }
            _ => Err(SurrealMindError::Validation {
                message: format!("Unknown subcommand: {}", params.subcommand),
            }),
        }
    }

    async fn handle_task_subprocess(
        task: &str,
        dry_run: bool,
        timeout: Duration,
    ) -> Result<CallToolResult> {
        Self::handle_task_subprocess_at(task, dry_run, timeout, &MaintenancePaths::default()).await
    }

    async fn handle_task_subprocess_at(
        task: &str,
        dry_run: bool,
        timeout: Duration,
        paths: &MaintenancePaths,
    ) -> Result<CallToolResult> {
        match task {
            "rethink" => {
                Self::handle_spawn_binary_at("gem_rethink", dry_run, &[], timeout, paths).await
            }
            "consolidate" => {
                let envs = [("CONSOLIDATE_LIMIT".to_string(), "100".to_string())];
                Self::handle_spawn_binary_at("kg_consolidate", dry_run, &envs, timeout, paths).await
            }
            "populate" => {
                Self::handle_spawn_binary_at("kg_populate", dry_run, &[], timeout, paths).await
            }
            "embed" => Self::handle_spawn_binary_at("kg_embed", dry_run, &[], timeout, paths).await,
            "wander" => {
                Self::handle_spawn_binary_at("kg_wander", dry_run, &[], timeout, paths).await
            }
            "health" => {
                Self::handle_spawn_script_at("scripts/sm_health.sh", dry_run, timeout, paths).await
            }
            other => Err(SurrealMindError::Validation {
                message: format!("Unknown task in list: {}", other),
            }),
        }
    }

    async fn try_run_subprocess_only_tasks_at(
        tasks: Option<&str>,
        dry_run: bool,
        timeout: Duration,
        paths: &MaintenancePaths,
    ) -> Result<Option<CallToolResult>> {
        let Some(task_list) = tasks else {
            return Ok(None);
        };
        let task_names: Vec<&str> = task_list
            .split(',')
            .map(str::trim)
            .filter(|task| !task.is_empty())
            .collect();
        if task_names.is_empty()
            || task_names.iter().any(|task| {
                *task == "all"
                    || !matches!(
                        *task,
                        "rethink" | "consolidate" | "populate" | "embed" | "wander" | "health"
                    )
            })
        {
            return Ok(None);
        }
        let mut results = Vec::new();
        for task in task_names {
            let result = Self::handle_task_subprocess_at(task, dry_run, timeout, paths).await;
            match result {
                Ok(result) => results.push(
                    result
                        .structured_content
                        .clone()
                        .unwrap_or_else(|| json!(result.content)),
                ),
                Err(error) => results.push(json!({"error": error.to_string(), "task": task})),
            }
        }
        Ok(Some(CallToolResult::structured(
            json!({"results": results}),
        )))
    }

    async fn handle_tasks(
        &self,
        tasks: Option<String>,
        dry_run: bool,
        timeout: Duration,
    ) -> Result<CallToolResult> {
        if let Some(result) = Self::try_run_subprocess_only_tasks_at(
            tasks.as_deref(),
            dry_run,
            timeout,
            &MaintenancePaths::default(),
        )
        .await?
        {
            return Ok(result);
        }
        let default_tasks: Vec<String> = vec![
            "populate".into(),
            "embed".into(),
            "rethink".into(),
            "consolidate".into(),
            "wander".into(),
            "health".into(),
            "report".into(),
            "corrections".into(),
        ];
        let list = tasks.unwrap_or_else(|| default_tasks.join(","));
        let mut tasks_vec: Vec<String> = list
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if tasks_vec.iter().any(|t| t == "all") {
            tasks_vec = default_tasks.clone();
        }
        if tasks_vec.is_empty() {
            tasks_vec = default_tasks;
        }
        let mut results: Vec<serde_json::Value> = Vec::new();
        for t in tasks_vec {
            let res = match t.as_str() {
                "corrections" => self.handle_corrections_bridge(100, None).await,
                "rethink" | "consolidate" | "populate" | "embed" | "wander" | "health" => {
                    Self::handle_task_subprocess(t.as_str(), dry_run, timeout).await
                }
                "report" => self.handle_report().await,
                other => Err(SurrealMindError::Validation {
                    message: format!("Unknown task in list: {}", other),
                }),
            };
            match res {
                Ok(r) => {
                    let payload = r
                        .structured_content
                        .clone()
                        .unwrap_or_else(|| json!(r.content));
                    results.push(payload);
                }
                Err(e) => results.push(json!({ "error": e.to_string(), "task": t })),
            }
        }
        Ok(CallToolResult::structured(json!({ "results": results })))
    }

    async fn handle_corrections_bridge(
        &self,
        limit: usize,
        target_id: Option<String>,
    ) -> Result<CallToolResult> {
        let mut map = serde_json::Map::new();
        map.insert("limit".into(), json!(limit as i64));
        if let Some(tid) = target_id {
            map.insert("target_id".into(), json!(tid));
        }
        let req = CallToolRequestParams::new("corrections").with_arguments(map);
        self.handle_corrections(req).await
    }

    async fn handle_spawn_binary(
        bin: &str,
        dry_run: bool,
        extra_env: &[(String, String)],
        timeout: Duration,
    ) -> Result<CallToolResult> {
        Self::handle_spawn_binary_at(
            bin,
            dry_run,
            extra_env,
            timeout,
            &MaintenancePaths::default(),
        )
        .await
    }

    async fn handle_spawn_binary_at(
        bin: &str,
        dry_run: bool,
        extra_env: &[(String, String)],
        timeout: Duration,
        paths: &MaintenancePaths,
    ) -> Result<CallToolResult> {
        let bin_path = maintenance_binary_path(bin, paths);
        let mut cmd = Command::new(bin_path);
        if dry_run {
            cmd.env("DRY_RUN", "1");
        }
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        if bin == "kg_wander" {
            cmd.env(KG_WANDER_PARENT_SUPERVISED, "1");
        }
        let output = run_maintenance_command(cmd, timeout).await.map_err(|e| {
            SurrealMindError::Internal {
                message: format!("failed to run {}: {}", bin, e),
            }
        })?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();
        let report = json!({
            "task": bin,
            "success": success,
            "stdout": stdout,
            "stderr": stderr
        });
        Ok(CallToolResult::structured(report))
    }

    async fn handle_spawn_script(
        script: &str,
        dry_run: bool,
        timeout: Duration,
    ) -> Result<CallToolResult> {
        Self::handle_spawn_script_at(script, dry_run, timeout, &MaintenancePaths::default()).await
    }

    async fn handle_spawn_script_at(
        script: &str,
        dry_run: bool,
        timeout: Duration,
        paths: &MaintenancePaths,
    ) -> Result<CallToolResult> {
        let script_path = maintenance_script_path(script, paths);
        let mut cmd = Command::new("bash");
        cmd.arg(script_path);
        if dry_run {
            cmd.env("DRY_RUN", "1");
        }
        let output = run_maintenance_command(cmd, timeout).await.map_err(|e| {
            SurrealMindError::Internal {
                message: format!("failed to run script {}: {}", script, e),
            }
        })?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();
        let report = json!({
            "task": script,
            "success": success,
            "stdout": stdout,
            "stderr": stderr
        });
        Ok(CallToolResult::structured(report))
    }

    async fn handle_report(&self) -> Result<CallToolResult> {
        let path = format!("{}/logs/remini_report.json", env!("CARGO_MANIFEST_DIR"));
        let contents = fs::read_to_string(&path).unwrap_or_else(|_| "{}".to_string());
        let json_val: serde_json::Value =
            serde_json::from_str(&contents).unwrap_or(json!({"warning": "report not found"}));
        Ok(CallToolResult::structured(json_val))
    }

    /// Return effective runtime configuration (safe subset) for debugging client/DB mismatch
    async fn handle_echo_config(&self) -> Result<CallToolResult> {
        let (prov, model, dim) = self.get_embedding_metadata();
        let rt = &self.config.runtime;
        let sys = &self.config.system;
        let out = json!({
            "db": {"url": sys.database_url, "ns": sys.database_ns, "db": sys.database_db},
            "embedding": {"provider": prov, "model": model, "dim": dim},
            "transport": rt.transport,
            "http": {"bind": rt.http_bind.to_string(), "path": rt.http_path},
            "mcp_no_log": rt.mcp_no_log,
        });
        Ok(CallToolResult::structured(out))
    }

    /// Ensure continuity fields and indexes exist on thoughts table
    async fn handle_ensure_continuity_fields(&self, dry_run: bool) -> Result<CallToolResult> {
        let mut created_fields = vec![];
        let mut created_indexes = vec![];
        let mut existing_fields = vec![];
        let mut existing_indexes = vec![];

        // Fields to ensure exist (SurrealDB 2.x type syntax)
        // Use option<...> instead of "NULL" suffix and record<thoughts> for record types.
        let fields: Vec<(&str, &str)> = vec![
            ("session_id", "option<string>"),
            ("chain_id", "option<string>"),
            ("previous_thought_id", "option<record<thoughts> | string>"),
            ("revises_thought", "option<record<thoughts> | string>"),
            ("branch_from", "option<record<thoughts> | string>"),
            ("confidence", "option<float>"),
        ];

        // Indexes to ensure exist
        let indexes = vec![
            "idx_thoughts_session: session_id, created_at",
            "idx_thoughts_chain: chain_id, created_at",
        ];

        let fields_len = fields.len();
        let indexes_len = indexes.len();

        // Check and create fields
        for (field_name, field_type) in &fields {
            let full_field_def = format!(
                "DEFINE FIELD {} ON TABLE thoughts TYPE {}",
                field_name, field_type
            );

            // Check if field exists (simple check - may not catch all cases)
            let check_query = "INFO FOR TABLE thoughts".to_string();
            if let Ok(mut response) = self.db.query(&check_query).await
                && let Ok(vec) = response.take::<Vec<serde_json::Value>>(0)
                && let Some(table_info) = vec.first()
                && let Some(fields_obj) = table_info.get("fields")
                && fields_obj.get(field_name).is_some()
            {
                existing_fields.push((*field_name).to_string());
                continue;
            }

            if !dry_run {
                match self.db.query(&full_field_def).await {
                    Ok(_) => {
                        created_fields.push((*field_name).to_string());
                        tracing::info!("Created continuity field: {}", field_name);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create continuity field {}: {}", field_name, e);
                        // Continue with other fields
                    }
                }
            } else {
                created_fields.push(format!("{} (dry-run)", field_name));
            }
        }

        // Check and create indexes
        for index_def in &indexes {
            let parts: Vec<&str> = index_def.split(": ").collect();
            if parts.len() != 2 {
                continue;
            }
            let index_name = parts[0];
            let index_cols = parts[1];
            let full_index_def = format!(
                "DEFINE INDEX {} ON TABLE thoughts FIELDS {};",
                index_name, index_cols
            );

            // Check if index exists (simple check - may not catch all cases)
            let check_query = "INFO FOR TABLE thoughts".to_string();
            if let Ok(mut response) = self.db.query(&check_query).await
                && let Ok(vec) = response.take::<Vec<serde_json::Value>>(0)
                && let Some(table_info) = vec.first()
                && let Some(indexes_obj) = table_info.get("indexes")
                && indexes_obj.get(index_name).is_some()
            {
                existing_indexes.push(index_name.to_string());
                continue;
            }

            if !dry_run {
                match self.db.query(&full_index_def).await {
                    Ok(_) => {
                        created_indexes.push(index_name.to_string());
                        tracing::info!("Created continuity index: {}", index_name);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create continuity index {}: {}", index_name, e);
                        // Continue with other indexes
                    }
                }
            } else {
                created_indexes.push(format!("{} (dry-run)", index_name));
            }
        }

        let result = json!({
            "created_fields": created_fields,
            "created_indexes": created_indexes,
            "existing_fields": existing_fields,
            "existing_indexes": existing_indexes,
            "dry_run": dry_run,
            "summary": format!(
                "Fields: {}/{} created, {}/{} existing. Indexes: {}/{} created, {}/{} existing.",
                created_fields.len(),
                fields_len,
                existing_fields.len(),
                fields_len,
                created_indexes.len(),
                indexes_len,
                existing_indexes.len(),
                indexes_len
            )
        });

        Ok(CallToolResult::structured(result))
    }

    async fn handle_health_check_embeddings(&self, _dry_run: bool) -> Result<CallToolResult> {
        // Determine expected embedding dimension from active embedder
        let expected = self.embedder.dimensions() as i64;
        let tables = vec!["thoughts", "kg_entities", "kg_observations", "kg_edges"];
        let mut report = serde_json::Map::new();

        report.insert("expected_dim".to_string(), json!(expected));

        for table in tables {
            // 1. Total count
            let q_total = format!("SELECT count() AS c FROM {} GROUP ALL", table);
            let total_res: Vec<serde_json::Value> = self.db.query(&q_total).await?.take(0)?;
            let total = total_res
                .first()
                .and_then(|v| v.get("c"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            // 2. OK count (valid array of correct length)
            let q_ok = format!(
                "SELECT count() AS c FROM {} WHERE type::is_array(embedding) AND array::len(embedding) = $d GROUP ALL",
                table
            );
            let ok_res: Vec<serde_json::Value> =
                self.db.query(&q_ok).bind(("d", expected)).await?.take(0)?;
            let ok = ok_res
                .first()
                .and_then(|v| v.get("c"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            // 3. Missing count (NONE or NULL)
            let q_missing = format!(
                "SELECT count() AS c FROM {} WHERE embedding IS NONE OR embedding = NULL GROUP ALL",
                table
            );
            let missing_res: Vec<serde_json::Value> = self.db.query(&q_missing).await?.take(0)?;
            let missing = missing_res
                .first()
                .and_then(|v| v.get("c"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            // 4. Mismatched count (Array but wrong length)
            let q_mismatched = format!(
                "SELECT count() AS c FROM {} WHERE type::is_array(embedding) AND array::len(embedding) != $d GROUP ALL",
                table
            );
            let mismatched_res: Vec<serde_json::Value> = self
                .db
                .query(&q_mismatched)
                .bind(("d", expected))
                .await?
                .take(0)?;
            let mismatched = mismatched_res
                .first()
                .and_then(|v| v.get("c"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            // 5. Get sample IDs for missing
            let q_sample_missing = format!(
                "SELECT meta::id(id) as id FROM {} WHERE embedding IS NONE OR embedding = NULL LIMIT 5",
                table
            );
            let sample_missing_res: Vec<serde_json::Value> =
                self.db.query(&q_sample_missing).await?.take(0)?;
            let sample_missing: Vec<String> = sample_missing_res
                .iter()
                .filter_map(|v| v.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()))
                .collect();

            // 6. Get sample IDs for mismatched
            let q_sample_mismatched = format!(
                "SELECT meta::id(id) as id FROM {} WHERE type::is_array(embedding) AND array::len(embedding) != $d LIMIT 5",
                table
            );
            let sample_mismatched_res: Vec<serde_json::Value> = self
                .db
                .query(&q_sample_mismatched)
                .bind(("d", expected))
                .await?
                .take(0)?;
            let sample_mismatched: Vec<String> = sample_mismatched_res
                .iter()
                .filter_map(|v| v.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()))
                .collect();

            let table_stats = json!({
                "total": total,
                "ok": ok,
                "missing": {
                    "count": missing,
                    "samples": sample_missing
                },
                "mismatched_dim": {
                    "count": mismatched,
                    "samples": sample_mismatched
                },
                "unknown_state": total.saturating_sub(ok + missing + mismatched) // Should be 0 if coverage is complete
            });

            report.insert(table.to_string(), table_stats);
        }

        // Add pending embeddings count (graceful degradation feature)
        let pending_query = r#"
            SELECT count() AS c FROM thoughts
            WHERE embedding_status IN ['pending', 'failed']
            GROUP ALL
        "#;
        let pending_res: Vec<serde_json::Value> = self.db.query(pending_query).await?.take(0)?;
        let pending_count = pending_res
            .first()
            .and_then(|v| v.get("c"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        report.insert(
            "pending_embeddings".to_string(),
            json!({
                "count": pending_count,
                "note": "Use 'maintain embed_pending' to retry these"
            }),
        );

        Ok(CallToolResult::structured(serde_json::Value::Object(
            report,
        )))
    }

    async fn handle_list_removal_candidates(
        &self,
        limit: usize,
        dry_run: bool,
    ) -> Result<CallToolResult> {
        tracing::info!("Listing removal candidates (dry_run={})", dry_run);

        let retention_days = std::env::var("SURR_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(30);

        // No need for cutoff, use time::now() directly in query

        let query = format!(
            "SELECT meta::id(id) as id, content, created_at FROM thoughts WHERE status = 'removal' AND created_at < time::now() - {}d LIMIT {}",
            retention_days, limit
        );

        let candidates: Vec<serde_json::Value> = self.db.query(&query).await?.take(0)?;

        let summary = json!({
            "total_candidates": candidates.len(),
            "retention_days": retention_days,
            "dry_run": dry_run,
            "candidates": candidates.into_iter().map(|c| {
                let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let content_preview = c.get("content").and_then(|v| v.as_str()).unwrap_or("").chars().take(100).collect::<String>();
                json!({
                    "id": id,
                    "content_preview": content_preview,
                    "created_at": c.get("created_at")
                })
            }).collect::<Vec<_>>()
        });

        Ok(CallToolResult::structured(summary))
    }

    async fn handle_export_removals(
        &self,
        limit: usize,
        format: &str,
        output_dir: &str,
        dry_run: bool,
    ) -> Result<CallToolResult> {
        tracing::info!(
            "Exporting removals (dry_run={}, format={}, output_dir={})",
            dry_run,
            format,
            output_dir
        );

        if format != "json" {
            return Err(SurrealMindError::Validation {
                message: format!("Unsupported format: {}. Only 'json' is supported.", format),
            });
        }

        // Get candidates
        let retention_days = std::env::var("SURR_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(30);

        let query = format!(
            "SELECT * FROM thoughts WHERE status = 'removal' AND created_at < time::now() - {}d LIMIT {}",
            retention_days, limit
        );

        let thoughts: Vec<serde_json::Value> = self.db.query(&query).await?.take(0)?;

        if thoughts.is_empty() {
            let summary = json!({
                "exported_count": 0,
                "file_path": null,
                "dry_run": dry_run,
                "message": "No thoughts to export"
            });
            return Ok(CallToolResult::structured(summary));
        }

        // Ensure output dir exists
        if !dry_run {
            fs::create_dir_all(output_dir).map_err(|e| SurrealMindError::Internal {
                message: format!("Failed to create output directory: {}", e),
            })?;
        }

        // Generate file path
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("thoughts_removal_{}.json", timestamp);
        let file_path = Path::new(output_dir).join(filename);

        // Serialize to JSON
        let json_data = serde_json::to_string_pretty(&thoughts).map_err(|e| {
            SurrealMindError::Serialization {
                message: format!("Failed to serialize thoughts: {}", e),
            }
        })?;

        if !dry_run {
            fs::write(&file_path, json_data).map_err(|e| SurrealMindError::Internal {
                message: format!("Failed to write export file: {}", e),
            })?;
        }

        let summary = json!({
            "exported_count": thoughts.len(),
            "file_path": file_path.to_string_lossy(),
            "dry_run": dry_run,
            "retention_days": retention_days
        });

        Ok(CallToolResult::structured(summary))
    }

    async fn handle_finalize_removal(&self, limit: usize, dry_run: bool) -> Result<CallToolResult> {
        tracing::info!("Finalizing removals (dry_run={})", dry_run);

        let retention_days = std::env::var("SURR_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(30);

        let query = format!(
            "SELECT meta::id(id) as id FROM thoughts WHERE status = 'removal' AND created_at < time::now() - {}d LIMIT {}",
            retention_days, limit
        );

        let candidates: Vec<serde_json::Value> = self.db.query(&query).await?.take(0)?;

        if candidates.is_empty() {
            let summary = json!({
                "deleted_count": 0,
                "dry_run": dry_run,
                "message": "No thoughts to delete"
            });
            return Ok(CallToolResult::structured(summary));
        }

        let ids: Vec<String> = candidates
            .into_iter()
            .filter_map(|c| c.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();

        let deleted_count = ids.len();

        if !dry_run {
            let delete_query = "DELETE FROM thoughts WHERE id IN $ids";
            self.db.query(delete_query).bind(("ids", ids)).await?;
        }

        let summary = json!({
            "deleted_count": deleted_count,
            "dry_run": dry_run,
            "retention_days": retention_days
        });

        Ok(CallToolResult::structured(summary))
    }

    async fn handle_reembed(&self, limit: usize, dry_run: bool) -> Result<CallToolResult> {
        // Call the reembed function from lib.rs
        let batch_size = 100; // Default batch size
        let stats = crate::run_reembed(batch_size, Some(limit), false, dry_run).await?;
        let result = reembed_stats_json(&stats, dry_run);
        Ok(CallToolResult::structured(result))
    }

    async fn handle_reembed_kg(&self, limit: usize, dry_run: bool) -> Result<CallToolResult> {
        // Call the library function directly
        let limit_opt = if limit == 0 { None } else { Some(limit) };
        let stats = crate::run_reembed_kg(limit_opt, dry_run)
            .await
            .map_err(|e| SurrealMindError::Internal {
                message: format!("KG reembed failed: {}", e),
            })?;

        let result = reembed_kg_stats_json(&stats, dry_run);
        Ok(CallToolResult::structured(result))
    }

    /// Handle embed_pending: retry embedding for thoughts with pending/failed status
    async fn handle_embed_pending(&self, limit: usize, dry_run: bool) -> Result<CallToolResult> {
        let limit_val = if limit == 0 { 100 } else { limit };

        // Query thoughts with pending or failed embedding status.
        // Use meta::id(id) so type::record('thoughts', $id) receives only
        // the record key, not a full thoughts:<id> value.
        // Note: SurrealDB 2.4+ requires ORDER BY fields in SELECT clause.
        // ORDER BY created_at ASC makes selection deterministic across runs: a row
        // that always errors (e.g. a persistent per-row statement error under N-2)
        // sorts to the same position every time instead of reshuffling, so it can
        // never wedge the whole backlog at zero progress by rotating in front of
        // rows behind it.
        let query = r#"
            SELECT meta::id(id) AS id, content, created_at
            FROM thoughts
            WHERE embedding_status IN ['pending', 'failed']
            ORDER BY created_at ASC
            LIMIT $limit;
        "#;

        let mut response = self
            .db
            .query(query)
            .bind(("limit", limit_val as i64))
            .await?;

        let rows: Vec<serde_json::Value> = response.take(0)?;

        if rows.is_empty() {
            return Ok(CallToolResult::structured(json!({
                "message": "No pending embeddings found",
                "processed": 0,
                "succeeded": 0,
                "failed": 0,
                "remaining": 0
            })));
        }

        let mut processed = 0;
        let mut succeeded = 0;
        let mut failed = 0;
        let (provider, model, dim) = self.get_embedding_metadata();

        for row in &rows {
            let raw_id = row
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let id = normalize_thought_record_key(&raw_id);
            let content = row
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            if id.is_empty() || content.is_empty() {
                continue;
            }

            processed += 1;

            if dry_run {
                tracing::info!(thought_id = %id, "Would embed (dry run)");
                succeeded += 1;
                continue;
            }

            // Attempt embedding
            match self.embedder.embed(&content).await {
                Ok(embedding) => {
                    if let Err(e) = crate::embeddings::ensure_generated_embedding_dimension(
                        &embedding,
                        dim as usize,
                    ) {
                        tracing::warn!(
                            thought_id = %id,
                            expected_dim = dim,
                            got_dim = embedding.len(),
                            error = %e,
                            "Embedding dimension mismatch; not writing, row stays pending/failed for retry"
                        );
                        failed += 1;
                        continue;
                    }
                    // Update thought with embedding
                    let update_query = r#"
                        UPDATE type::record('thoughts', $id) SET
                        embedding = $embedding,
                        embedding_provider = $provider,
                        embedding_model = $model,
                        embedding_dim = $dim,
                        embedded_at = time::now(),
                        embedding_status = 'complete'
                        RETURN meta::id(id) AS id, embedding_status, array::len(embedding) AS embedding_len;
                    "#;

                    let update_result = self
                        .db
                        .query(update_query)
                        .bind(("id", id.clone()))
                        .bind(("embedding", embedding))
                        .bind(("provider", provider.clone()))
                        .bind(("model", model.clone()))
                        .bind(("dim", dim))
                        .await;

                    match update_result {
                        Ok(mut response) => {
                            // N-2 fix: take(0) surfaces per-statement errors (e.g. a
                            // SCHEMAFULL type violation) that `query().await` itself
                            // does not report. Matching instead of `?` keeps a single
                            // poisoned row from aborting the whole embed_pending call.
                            match response.take::<Vec<serde_json::Value>>(0) {
                                Ok(updated) => {
                                    let updated_row = updated.first();
                                    let status_is_complete = updated_row
                                        .and_then(|r| r.get("embedding_status"))
                                        .and_then(|v| v.as_str())
                                        == Some("complete");
                                    let embedding_len_matches = updated_row
                                        .and_then(|r| r.get("embedding_len"))
                                        .and_then(|v| v.as_i64())
                                        == Some(dim);

                                    if status_is_complete && embedding_len_matches {
                                        tracing::info!(thought_id = %id, "Successfully embedded pending thought");
                                        succeeded += 1;
                                    } else {
                                        tracing::warn!(
                                            thought_id = %id,
                                            raw_thought_id = %raw_id,
                                            updated_rows = updated.len(),
                                            "Embedding update did not persist expected complete state"
                                        );
                                        failed += 1;
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(thought_id = %id, error = %e, "Statement error verifying embedding update");
                                    failed += 1;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(thought_id = %id, error = %e, "Failed to update thought with embedding");
                            failed += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(thought_id = %id, error = %e, "Embedding failed");
                    failed += 1;
                }
            }
        }

        // Count remaining pending.
        // GROUP ALL is required: SurrealDB 3.1's zero-arg count() returns a
        // hardcoded 1 per matched row without it, so this would report N=3
        // matching rows as three separate {cnt:1} rows instead of one {cnt:3}.
        let count_query = r#"
            SELECT count() AS cnt
            FROM thoughts
            WHERE embedding_status IN ['pending', 'failed']
            GROUP ALL;
        "#;
        let mut count_response = self.db.query(count_query).await?;
        let count_rows: Vec<serde_json::Value> = count_response.take(0)?;
        let remaining = count_rows
            .first()
            .and_then(|r| r.get("cnt"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as usize;

        Ok(CallToolResult::structured(json!({
            "message": if dry_run { "Dry run complete" } else { "Embedding retry complete" },
            "processed": processed,
            "succeeded": succeeded,
            "failed": failed,
            "remaining": remaining,
            "dry_run": dry_run
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CANCELLED, CapturedOutput, MAINTENANCE_OUTPUT_LIMIT, SurrealMindServer,
        maintenance_timeout, normalize_thought_record_key, reembed_kg_stats_json,
        reembed_stats_json, render_output, run_maintenance_command,
        run_maintenance_command_blocking,
    };
    use crate::{ReembedKgStats, ReembedStats};
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use std::sync::{Arc, atomic::AtomicU8};
    use std::time::{Duration, Instant};

    fn write_executable(path: &std::path::Path, body: &str) {
        std::fs::write(path, body).unwrap();
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    fn result_payload(result: rmcp::model::CallToolResult) -> serde_json::Value {
        result
            .structured_content
            .expect("structured maintenance result")
    }

    async fn wait_for_pid(path: &std::path::Path) -> i32 {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Ok(pid) = std::fs::read_to_string(path)
                && let Ok(pid) = pid.trim().parse()
            {
                return pid;
            }
            assert!(Instant::now() < deadline, "fixture did not publish its pid");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn assert_process_gone(pid: i32) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if unsafe { libc::kill(pid, 0) } != 0 {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "process {} was not terminated",
                pid
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[test]
    fn normalize_thought_record_key_accepts_plain_meta_id() {
        assert_eq!(normalize_thought_record_key("0f7ce74b"), "0f7ce74b");
    }

    #[test]
    fn normalize_thought_record_key_strips_table_prefix_and_backticks() {
        assert_eq!(
            normalize_thought_record_key("thoughts:`0f7ce74b`"),
            "0f7ce74b"
        );
    }

    #[test]
    fn reembed_json_surfaces_failed_count() {
        let stats = ReembedStats {
            expected_dim: 3,
            batch_size: 10,
            dry_run: false,
            missing_only: false,
            processed: 2,
            updated: 1,
            skipped: 0,
            missing: 1,
            mismatched: 0,
            no_match: 0,
            failed: 1,
        };
        assert_eq!(reembed_stats_json(&stats, false)["failed"], 1);
    }

    #[test]
    fn reembed_kg_json_surfaces_failed_counts_for_every_table() {
        let stats = ReembedKgStats {
            expected_dim: 3,
            provider: "fixture".to_string(),
            model: "fixture".to_string(),
            dry_run: false,
            entities_updated: 0,
            entities_skipped: 0,
            entities_missing: 0,
            entities_mismatched: 0,
            observations_updated: 0,
            observations_skipped: 0,
            observations_missing: 0,
            observations_mismatched: 0,
            edges_updated: 0,
            edges_skipped: 0,
            edges_missing: 0,
            edges_mismatched: 0,
            entities_no_match: 0,
            observations_no_match: 0,
            edges_no_match: 0,
            entities_failed: 1,
            observations_failed: 2,
            edges_failed: 3,
        };
        let result = reembed_kg_stats_json(&stats, false);
        assert_eq!(result["entities"]["failed"], 1);
        assert_eq!(result["observations"]["failed"], 2);
        assert_eq!(result["edges"]["failed"], 3);
    }

    #[test]
    fn maintenance_output_marks_reader_deadline_as_incomplete() {
        let rendered = render_output(
            CapturedOutput {
                bytes: b"partial".to_vec(),
                discarded: 0,
                incomplete: true,
            },
            "stderr",
        );
        let rendered = String::from_utf8_lossy(&rendered);
        assert!(rendered.contains("OUTPUT_TRUNCATED stream=stderr"));
        assert!(rendered.contains("incomplete=true"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn maintenance_command_does_not_block_independent_timer() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("child.pid");
        let script = format!(
            "printf '%s' $$ > '{}'; sleep 1; printf done",
            pid_path.display()
        );
        let mut command = Command::new("sh");
        command.args(["-c", &script]);
        let started = Instant::now();
        let (result, timer_elapsed) = tokio::join!(
            run_maintenance_command(command, Duration::from_secs(5)),
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                started.elapsed()
            }
        );
        let output = result.unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "done");
        let pid: i32 = std::fs::read_to_string(pid_path).unwrap().parse().unwrap();
        assert_ne!(
            unsafe { libc::kill(pid, 0) },
            0,
            "fixture child was not reaped"
        );
        assert!(
            timer_elapsed < Duration::from_millis(300),
            "independent Tokio timer was stalled for {:?}; output={:?}",
            timer_elapsed,
            output.stdout
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn maintenance_command_timeout_kills_descendants() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("descendant.pid");
        let script = format!(
            "sleep 30 & child=$!; printf '%s' $child > '{}'; wait $child",
            pid_path.display()
        );
        let mut command = Command::new("sh");
        command.args(["-c", &script]);
        let output = run_maintenance_command(command, Duration::from_millis(100))
            .await
            .unwrap();
        let pid = wait_for_pid(&pid_path).await;
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("TIMEOUT:"));
        assert_process_gone(pid).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn maintenance_command_leader_exit_still_kills_descendants() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("early-exit-descendant.pid");
        let script = format!(
            "sleep 30 & child=$!; printf '%s' $child > '{}'; exit 0",
            pid_path.display()
        );
        let mut command = Command::new("sh");
        command.args(["-c", &script]);
        let output = run_maintenance_command(command, Duration::from_secs(5))
            .await
            .unwrap();
        let pid = wait_for_pid(&pid_path).await;
        assert!(!output.status.success());
        assert_process_gone(pid).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn maintenance_command_cancellation_kills_descendants() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("cancelled-descendant.pid");
        let script = format!(
            "sleep 30 & child=$!; printf '%s' $child > '{}'; wait $child",
            pid_path.display()
        );
        let mut command = Command::new("sh");
        command.args(["-c", &script]);
        let task = tokio::spawn(run_maintenance_command(command, Duration::from_secs(30)));
        let pid = wait_for_pid(&pid_path).await;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_process_gone(pid).await;
    }

    #[test]
    fn maintenance_command_cancelled_before_spawn_does_not_launch_fixture() {
        let dir = tempfile::tempdir().unwrap();
        let sentinel = dir.path().join("spawned");
        let script = format!("printf spawned > '{}'", sentinel.display());
        let mut command = Command::new("sh");
        command.args(["-c", &script]);
        let state = Arc::new(AtomicU8::new(CANCELLED));
        let output =
            run_maintenance_command_blocking(command, Duration::from_secs(5), state).unwrap();
        assert!(!output.status.success());
        assert!(!sentinel.exists(), "cancelled command launched its fixture");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn maintenance_command_timeout_does_not_kill_unrelated_process() {
        let mut sentinel = Command::new("sleep").arg("2").spawn().unwrap();
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30"]);
        let output = run_maintenance_command(command, Duration::from_millis(100))
            .await
            .unwrap();
        assert!(!output.status.success());
        assert!(sentinel.try_wait().unwrap().is_none());
        sentinel.kill().unwrap();
        sentinel.wait().unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn maintenance_command_bounds_stdout_and_stderr() {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "head -c 100000 /dev/zero; head -c 100000 /dev/zero >&2",
        ]);
        let output = run_maintenance_command(command, Duration::from_secs(5))
            .await
            .unwrap();
        assert!(output.status.success());
        assert!(output.stdout.len() <= MAINTENANCE_OUTPUT_LIMIT + 200);
        assert!(output.stderr.len() <= MAINTENANCE_OUTPUT_LIMIT + 200);
        assert!(String::from_utf8_lossy(&output.stdout).contains("OUTPUT_TRUNCATED stream=stdout"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("OUTPUT_TRUNCATED stream=stderr"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn maintenance_command_preserves_nonzero_status_and_output() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf ok; printf bad >&2; exit 7"]);
        let output = run_maintenance_command(command, Duration::from_secs(5))
            .await
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
        assert_eq!(String::from_utf8_lossy(&output.stderr), "bad");
    }

    #[test]
    fn maintenance_timeout_has_safe_default_and_strict_bounds() {
        assert_eq!(
            maintenance_timeout(None).unwrap(),
            Duration::from_secs(1_800)
        );
        assert_eq!(
            maintenance_timeout(Some(100)).unwrap(),
            Duration::from_millis(100)
        );
        assert!(maintenance_timeout(Some(99)).is_err());
        assert!(maintenance_timeout(Some(3_600_001)).is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn maintenance_handlers_propagate_timeout_for_binary_script_and_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let script_dir = dir.path().join("scripts");
        std::fs::create_dir(&script_dir).unwrap();
        write_executable(
            &dir.path().join("kg_populate"),
            "#!/bin/sh\nprintf binary-ready\nsleep 30\n",
        );
        write_executable(
            &dir.path().join("kg_wander"),
            "#!/bin/sh\nprintf '%s' \"$KG_WANDER_PARENT_SUPERVISED\"\n",
        );
        write_executable(
            &script_dir.join("sm_health.sh"),
            "#!/bin/sh\nprintf script-ready\nsleep 30\n",
        );
        let paths = super::MaintenancePaths {
            bin_dir: Some(dir.path().to_path_buf()),
            script_dir: Some(script_dir.clone()),
        };

        let binary = SurrealMindServer::handle_spawn_binary_at(
            "kg_populate",
            false,
            &[],
            Duration::from_millis(100),
            &paths,
        )
        .await
        .unwrap();
        let binary_payload = result_payload(binary);
        assert!(!binary_payload["success"].as_bool().unwrap());
        assert!(
            binary_payload["stderr"]
                .as_str()
                .unwrap()
                .contains("TIMEOUT:")
        );

        let wander = SurrealMindServer::handle_spawn_binary_at(
            "kg_wander",
            false,
            &[],
            Duration::from_secs(1),
            &paths,
        )
        .await
        .unwrap();
        let wander_payload = result_payload(wander);
        assert!(wander_payload["success"].as_bool().unwrap());
        assert_eq!(wander_payload["stdout"].as_str().unwrap(), "1");

        let script = SurrealMindServer::handle_spawn_script_at(
            "scripts/sm_health.sh",
            false,
            Duration::from_millis(100),
            &paths,
        )
        .await
        .unwrap();
        let script_payload = result_payload(script);
        assert!(!script_payload["success"].as_bool().unwrap());
        assert!(
            script_payload["stderr"]
                .as_str()
                .unwrap()
                .contains("TIMEOUT:")
        );

        let tasks = SurrealMindServer::try_run_subprocess_only_tasks_at(
            Some("populate"),
            false,
            Duration::from_millis(100),
            &paths,
        )
        .await
        .unwrap();
        let tasks_payload = tasks.unwrap().structured_content.unwrap();
        let tasks_payload = &tasks_payload["results"][0];
        assert!(!tasks_payload["success"].as_bool().unwrap());
        assert!(
            tasks_payload["stderr"]
                .as_str()
                .unwrap()
                .contains("TIMEOUT:")
        );

        for task_list in ["", "   ", ",,,", "all", "populate,report"] {
            assert!(
                SurrealMindServer::try_run_subprocess_only_tasks_at(
                    Some(task_list),
                    false,
                    Duration::from_millis(100),
                    &paths,
                )
                .await
                .unwrap()
                .is_none(),
                "task list {:?} bypassed the public fallback path",
                task_list
            );
        }
    }
}
