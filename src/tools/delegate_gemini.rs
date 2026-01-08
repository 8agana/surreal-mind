//! delegate_gemini tool handler to call Gemini CLI with persistence

use crate::clients::traits::CognitiveAgent;
use crate::clients::{AgentError, GeminiClient, PersistedAgent};
use crate::error::{Result, SurrealMindError};
use crate::registry;
use crate::server::SurrealMindServer;
use once_cell::sync::Lazy;
use regex::Regex;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client as WsClient;

const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_CANCEL_POLL_MS: u64 = 250;

static INVALID_PROMPT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"record `(?P<id>agent_jobs:[^`]+)`").unwrap());

/// Parameters for the delegate_gemini tool
#[derive(Debug, Deserialize)]
pub struct DelegateGeminiParams {
    pub prompt: String,
    #[serde(default)]
    pub task_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Working directory for the Gemini CLI subprocess
    #[serde(default)]
    pub cwd: Option<String>,
    /// Timeout in milliseconds (overrides GEMINI_TIMEOUT_MS env var)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Per-tool timeout in milliseconds (overrides GEMINI_TOOL_TIMEOUT_MS env var)
    #[serde(default)]
    pub tool_timeout_ms: Option<u64>,
    /// Expose streaming events in response
    #[serde(default)]
    pub expose_stream: bool,
}

#[derive(Debug, Deserialize)]
struct SessionResult {
    #[serde(default)]
    last_agent_session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JobStatusRow {
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    const fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }
}

enum JobOutcome {
    Completed(std::result::Result<crate::clients::traits::AgentResponse, AgentError>),
    Cancelled,
}

impl SurrealMindServer {
    /// Handle the delegate_gemini tool call
    pub async fn handle_delegate_gemini(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: DelegateGeminiParams =
            serde_json::from_value(Value::Object(args)).map_err(|e| {
                SurrealMindError::InvalidParams {
                    message: format!("Invalid parameters: {}", e),
                }
            })?;

        let prompt = params.prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "prompt cannot be empty".into(),
            });
        }

        let task_name = normalize_optional_string(params.task_name)
            .unwrap_or_else(|| "delegate_gemini".to_string());
        let model_override = normalize_optional_string(params.model);
        let model = model_override
            .clone()
            .unwrap_or_else(|| default_model_name(Some(&self.config)));
        let cwd = normalize_optional_string(params.cwd);
        let timeout = params.timeout_ms.unwrap_or_else(gemini_timeout_ms);
        let _tool_timeout = params.tool_timeout_ms.unwrap_or_else(|| {
            std::env::var("GEMINI_TOOL_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(300_000) // 5 minutes default
        });

        // Async-only: spawn background task
        let job_id = uuid::Uuid::new_v4().to_string();

        // Create job record
        create_job_record(
            self.db.as_ref(),
            job_id.clone(),
            "delegate_gemini".to_string(),
            "gemini".to_string(),
            model.clone(),
            prompt.clone(),
            task_name.clone(),
            model_override.clone(),
            cwd.clone(),
            timeout,
            params.tool_timeout_ms,
            params.expose_stream,
        )
        .await?;

        Ok(CallToolResult::structured(json!({
            "status": "queued",
            "job_id": job_id,
            "message": "Job queued for background execution. Use agent_job_status to check progress."
        })))
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn default_model_name(config: Option<&crate::config::Config>) -> String {
    std::env::var("GEMINI_MODEL").unwrap_or_else(|_| {
        config
            .map(|c| c.system.gemini_model.clone())
            .unwrap_or_else(|| "auto".to_string())
    })
}
fn gemini_timeout_ms() -> u64 {
    std::env::var("GEMINI_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_MS)
}

async fn fetch_last_session_id(
    db: &Surreal<WsClient>,
    tool_name: String,
) -> Result<Option<String>> {
    let sql = "SELECT last_agent_session_id FROM tool_sessions WHERE tool_name = $tool LIMIT 1;";
    let rows: Vec<SessionResult> = db
        .query(sql)
        .bind(("tool", tool_name))
        .await?
        .take::<Vec<SessionResult>>(0)?;
    Ok(rows
        .first()
        .and_then(|row| row.last_agent_session_id.clone()))
}

// Helper functions for async job management

#[allow(clippy::too_many_arguments)]
async fn create_job_record(
    db: &Surreal<WsClient>,
    job_id: String,
    tool_name: String,
    agent_source: String,
    agent_instance: String,
    prompt: String,
    task_name: String,
    model_override: Option<String>,
    cwd: Option<String>,
    timeout_ms: u64,
    tool_timeout_ms: Option<u64>,
    expose_stream: bool,
) -> Result<()> {
    // Convert Option<String> to JSON values that SurrealDB can handle
    let model_override_json: Value = model_override.map(Value::String).unwrap_or(Value::Null);
    let cwd_json: Value = cwd.map(Value::String).unwrap_or(Value::Null);
    let tool_timeout_json: Value = tool_timeout_ms.map(Value::from).unwrap_or(Value::Null);

    let sql = "CREATE agent_jobs SET job_id = $job_id, tool_name = $tool_name, agent_source = $agent_source, agent_instance = $agent_instance, prompt = $prompt, task_name = $task_name, model_override = $model_override, cwd = $cwd, timeout_ms = $timeout_ms, tool_timeout_ms = $tool_timeout_ms, expose_stream = $expose_stream, status = $status, created_at = time::now();";
    db.query(sql)
        .bind(("job_id", job_id))
        .bind(("tool_name", tool_name))
        .bind(("agent_source", agent_source))
        .bind(("agent_instance", agent_instance))
        .bind(("prompt", prompt))
        .bind(("task_name", task_name))
        .bind(("model_override", model_override_json))
        .bind(("cwd", cwd_json))
        .bind(("timeout_ms", timeout_ms as i64))
        .bind(("tool_timeout_ms", tool_timeout_json))
        .bind(("expose_stream", expose_stream))
        .bind(("status", JobStatus::Queued.as_str()))
        .await?;
    Ok(())
}

async fn complete_job(
    db: &Surreal<WsClient>,
    job_id: &str,
    session_id: Option<String>,
    exchange_id: Option<String>,
    duration_ms: i64,
) -> Result<()> {
    let mut sql = "UPDATE agent_jobs SET status = $status, completed_at = time::now(), duration_ms = $duration_ms".to_string();

    if session_id.is_some() {
        sql.push_str(", session_id = $session_id");
    }
    if exchange_id.is_some() {
        sql.push_str(", exchange_id = type::thing($exchange_id)");
    }
    sql.push_str(" WHERE job_id = $job_id AND status != 'cancelled';");

    let mut query = db
        .query(&sql)
        .bind(("job_id", job_id.to_string()))
        .bind(("status", JobStatus::Completed.as_str()))
        .bind(("duration_ms", duration_ms));

    if let Some(sid) = session_id {
        query = query.bind(("session_id", sid));
    }
    if let Some(eid) = exchange_id {
        query = query.bind(("exchange_id", eid));
    }

    query.await?;
    Ok(())
}

async fn fail_job(
    db: &Surreal<WsClient>,
    job_id: &str,
    error: String,
    duration_ms: i64,
) -> Result<()> {
    let sql = "UPDATE agent_jobs SET status = $status, error = $error, completed_at = time::now(), duration_ms = $duration_ms WHERE job_id = $job_id AND status != 'cancelled';";
    db.query(sql)
        .bind(("job_id", job_id.to_string()))
        .bind(("status", JobStatus::Failed.as_str()))
        .bind(("error", error))
        .bind(("duration_ms", duration_ms))
        .await?;
    Ok(())
}

async fn is_job_cancelled(db: &Surreal<WsClient>, job_id: &str) -> Result<bool> {
    let sql = "SELECT status FROM agent_jobs WHERE job_id = $job_id LIMIT 1;";
    let mut response = db.query(sql).bind(("job_id", job_id.to_string())).await?;
    let rows: Vec<JobStatusRow> = response.take(0)?;
    Ok(rows
        .first()
        .map(|row| row.status.as_str() == "cancelled")
        .unwrap_or(false))
}

async fn mark_job_cancelled(db: &Surreal<WsClient>, job_id: &str, duration_ms: i64) -> Result<()> {
    let sql = "UPDATE agent_jobs SET status = 'cancelled', completed_at = time::now(), duration_ms = $duration_ms WHERE job_id = $job_id;";
    db.query(sql)
        .bind(("job_id", job_id.to_string()))
        .bind(("duration_ms", duration_ms))
        .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct QueuedJobIdRow {
    job_id: String,
    #[serde(rename = "created_at")]
    _created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QueuedJobRow {
    job_id: String,
    prompt: Option<String>,
    task_name: Option<String>,
    model_override: Option<String>,
    cwd: Option<String>,
    timeout_ms: Option<i64>,
    tool_timeout_ms: Option<i64>,
    expose_stream: Option<bool>,
}

pub async fn run_delegate_gemini_worker(
    db: std::sync::Arc<Surreal<WsClient>>,
    semaphore: std::sync::Arc<tokio::sync::Semaphore>,
) {
    tracing::info!("delegate_gemini worker started");
    let poll_ms = std::env::var("SURR_JOB_POLL_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(500);
    let cancel_poll_ms = std::env::var("SURR_JOB_CANCEL_POLL_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_CANCEL_POLL_MS);

    loop {
        let claimed = match claim_next_job(db.as_ref()).await {
            Ok(job) => job,
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("Found NONE for field `prompt`")
                    && let Some(caps) = INVALID_PROMPT_RE.captures(&err_msg)
                    && let Some(id) = caps.name("id")
                {
                    let _ = fail_invalid_prompt(db.as_ref(), id.as_str()).await;
                }
                eprintln!("[ERROR delegate_gemini worker] Claim failed: {}", err_msg);
                tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
                continue;
            }
        };

        let Some(job) = claimed else {
            tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
            continue;
        };

        tracing::info!(job_id = %job.job_id, "delegate_gemini worker claimed job");
        let _permit = semaphore.acquire().await.expect("semaphore closed");

        if is_job_cancelled(db.as_ref(), &job.job_id)
            .await
            .unwrap_or(false)
        {
            let _ = mark_job_cancelled(db.as_ref(), &job.job_id, 0).await;
            continue;
        }

        let timeout = job
            .timeout_ms
            .and_then(|v| u64::try_from(v).ok())
            .unwrap_or_else(gemini_timeout_ms);

        let tool_timeout = job
            .tool_timeout_ms
            .and_then(|v| u64::try_from(v).ok())
            .unwrap_or(300_000); // 5 minutes default

        let prompt = job.prompt.as_deref().unwrap_or("").trim();
        if prompt.is_empty() {
            let _ = fail_job(
                db.as_ref(),
                &job.job_id,
                "Missing prompt in job metadata".to_string(),
                0,
            )
            .await;
            continue;
        }
        let task_name = job.task_name.as_deref().unwrap_or("delegate_gemini").trim();

        // GeminiClient now handles activity-based timeout internally.
        // No outer timeout wrapper needed - this prevents double-timeout issues.
        let started_at = chrono::Utc::now();
        let job_id = job.job_id.clone();
        let prompt = prompt.to_string();
        let task_name = task_name.to_string();
        let model_override = job.model_override.clone();
        let cwd = job.cwd.clone();
        let expose_stream = job.expose_stream.unwrap_or(false);
        let mut handle = tokio::spawn({
            let db = db.clone();
            async move {
                execute_gemini_call(
                    db,
                    GeminiCallParams {
                        prompt: prompt.as_str(),
                        task_name: task_name.as_str(),
                        model_override: model_override.as_deref(),
                        cwd: cwd.as_deref(),
                        timeout,
                        tool_timeout,
                        expose_stream,
                    },
                )
                .await
            }
        });

        // Create a dummy task for registry so we can abort via call_cancel
        let registry_handle = tokio::spawn(async { std::future::pending::<()>().await });
        registry::register_job(job_id.clone(), registry_handle);

        let mut cancel_interval =
            tokio::time::interval(std::time::Duration::from_millis(cancel_poll_ms));
        let result = loop {
            tokio::select! {
                res = &mut handle => {
                    let output = match res {
                        Ok(inner) => inner,
                        Err(err) => Err(AgentError::CliError(format!("delegate_gemini task join error: {}", err))),
                    };
                    break JobOutcome::Completed(output);
                }
                _ = cancel_interval.tick() => {
                    match is_job_cancelled(db.as_ref(), &job_id).await {
                        Ok(true) => {
                            handle.abort();
                            let _ = handle.await;
                            break JobOutcome::Cancelled;
                        }
                        Ok(false) => {}
                        Err(e) => {
                            tracing::warn!(job_id = %job_id, "cancel check failed: {}", e);
                        }
                    }
                }
            }
        };

        let completed_at = chrono::Utc::now();
        let duration_ms = (completed_at - started_at).num_milliseconds();

        // Unregister from the job registry now that the job is complete
        registry::unregister_job(&job_id);

        match result {
            JobOutcome::Completed(Ok(response)) => {
                if let Err(e) = complete_job(
                    db.as_ref(),
                    &job_id,
                    Some(response.session_id.clone()),
                    response.exchange_id.clone(),
                    duration_ms,
                )
                .await
                {
                    eprintln!(
                        "[ERROR delegate_gemini worker] Failed to complete job: {}",
                        e
                    );
                    let _ = fail_job(
                        db.as_ref(),
                        &job_id,
                        format!("Completion update failed: {}", e),
                        duration_ms,
                    )
                    .await;
                }
            }
            JobOutcome::Completed(Err(agent_err)) => {
                let error_msg = format!("Agent error: {}", agent_err);
                if let Err(e) = fail_job(db.as_ref(), &job_id, error_msg, duration_ms).await {
                    eprintln!(
                        "[ERROR delegate_gemini worker] Failed to mark job as failed: {}",
                        e
                    );
                }
            }
            JobOutcome::Cancelled => {
                let _ = mark_job_cancelled(db.as_ref(), &job_id, duration_ms).await;
            }
        }
    }
}

async fn claim_next_job(db: &Surreal<WsClient>) -> Result<Option<QueuedJobRow>> {
    let mut response = db
        .query(
            "SELECT job_id, created_at FROM agent_jobs WHERE status = 'queued' AND type::is::string(prompt) AND prompt != '' ORDER BY created_at ASC LIMIT 1;",
        )
        .await?;
    let rows: Vec<QueuedJobIdRow> = response.take(0)?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };

    let mut response = db
        .query(
            "UPDATE agent_jobs SET status = 'running', started_at = time::now() WHERE job_id = $job_id AND status = 'queued' AND type::is::string(prompt) AND prompt != '' RETURN job_id, prompt, task_name, model_override, cwd, timeout_ms, tool_timeout_ms, expose_stream;",
        )
        .bind(("job_id", row.job_id.clone()))
        .await?;
    let rows: Vec<QueuedJobRow> = response.take(0)?;
    Ok(rows.into_iter().next())
}

async fn fail_invalid_prompt(db: &Surreal<WsClient>, record_id: &str) -> Result<()> {
    let record_id = record_id.to_string();
    let sql = "UPDATE $record SET status = 'failed', error = $error, prompt = '', completed_at = time::now(), duration_ms = 0";
    db.query(sql)
        .bind(("record", record_id))
        .bind(("error", "Invalid prompt in job record"))
        .await?;
    Ok(())
}

#[derive(Debug)]
struct GeminiCallParams<'a> {
    prompt: &'a str,
    task_name: &'a str,
    model_override: Option<&'a str>,
    cwd: Option<&'a str>,
    timeout: u64,
    tool_timeout: u64,
    expose_stream: bool,
}

async fn execute_gemini_call(
    db: std::sync::Arc<Surreal<WsClient>>,
    params: GeminiCallParams<'_>,
) -> std::result::Result<crate::clients::traits::AgentResponse, AgentError> {
    let resume_session = fetch_last_session_id(db.as_ref(), params.task_name.to_string())
        .await
        .map_err(|e| AgentError::CliError(format!("Failed to fetch session: {}", e)))?;

    let config = crate::config::Config::load().ok();
    let model = params
        .model_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_model_name(config.as_ref()));

    let mut gemini = GeminiClient::with_timeout_ms(model.clone(), params.timeout);
    gemini = gemini.with_tool_timeout_ms(params.tool_timeout);
    if let Some(dir) = params.cwd {
        gemini = gemini.with_cwd(dir);
    }
    if params.expose_stream {
        gemini = gemini.with_expose_stream(true);
    }

    let agent = PersistedAgent::new(
        gemini,
        db.clone(),
        "gemini",
        model,
        params.task_name.to_string(),
    );

    agent.call(params.prompt, resume_session.as_deref()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that the cancel poll interval has been reduced to enable faster cancellation
    /// The registry module tests handle the core abort functionality
    #[tokio::test]
    async fn test_cancel_poll_interval_reduced() {
        // Verify the cancel poll interval has been reduced from 1000ms to 250ms
        // This is important for AC #1: "call_cancel changes job status to cancelled within seconds"

        let expected_poll_ms = 250u64;
        assert_eq!(DEFAULT_CANCEL_POLL_MS, expected_poll_ms);
    }

    #[tokio::test]
    #[ignore]
    async fn test_job_registry_integration() {
        // Test that delegate_gemini worker properly registers and unregisters jobs
        // NOTE: Ignored because registry is global and tests interfere with each other.
        // The registry::tests module provides comprehensive coverage.

        // Register a job like the worker does
        let job_id = format!("test-gemini-job-{}", uuid::Uuid::new_v4());
        let handle = tokio::spawn(async { std::future::pending::<()>().await });
        let before_register = crate::registry::registry_size();
        crate::registry::register_job(job_id.clone(), handle);
        let after_register = crate::registry::registry_size();

        // Verify it's registered
        assert_eq!(after_register, before_register + 1);

        // Simulate cancel being called
        let was_aborted = crate::registry::abort_job(&job_id);
        assert!(was_aborted);

        // Verify it's gone
        let after_abort = crate::registry::registry_size();
        assert_eq!(after_abort, before_register);
    }

    #[tokio::test]
    #[ignore]
    async fn test_multiple_jobs_independent_cancellation() {
        // Test that cancelling one job doesn't affect others
        // NOTE: Ignored because registry is global. See registry::tests for comprehensive coverage.

        let job_1 = format!("test-job-1-{}", uuid::Uuid::new_v4());
        let job_2 = format!("test-job-2-{}", uuid::Uuid::new_v4());

        let handle1 = tokio::spawn(async { std::future::pending::<()>().await });
        let handle2 = tokio::spawn(async { std::future::pending::<()>().await });

        let before = crate::registry::registry_size();
        crate::registry::register_job(job_1.clone(), handle1);
        crate::registry::register_job(job_2.clone(), handle2);

        assert_eq!(crate::registry::registry_size(), before + 2);

        // Cancel only job_1
        let aborted_1 = crate::registry::abort_job(&job_1);
        assert!(aborted_1);

        // Job 2 should still be there
        assert_eq!(crate::registry::registry_size(), before + 1);

        // Can still abort job_2
        let aborted_2 = crate::registry::abort_job(&job_2);
        assert!(aborted_2);

        assert_eq!(crate::registry::registry_size(), before);
    }

    #[tokio::test]
    #[ignore]
    async fn test_cancel_job_removes_from_registry() {
        // Test that when a job completes normally, it's unregistered
        // This prevents registry bloat and ensures cleanup
        // NOTE: Ignored because registry is global. See registry::tests for comprehensive coverage.

        let job_id = format!("test-cleanup-job-{}", uuid::Uuid::new_v4());
        let handle = tokio::spawn(async {});
        let initial_size = crate::registry::registry_size();

        crate::registry::register_job(job_id.clone(), handle);
        assert!(crate::registry::registry_size() > initial_size);

        // Simulate what happens at end of worker loop
        crate::registry::unregister_job(&job_id);
        assert_eq!(crate::registry::registry_size(), initial_size);
    }

    #[tokio::test]
    async fn test_immediate_abort_vs_polling() {
        // Test that registry-based abort is faster than polling
        // AC #1 requires cancellation within seconds

        let job_id = format!("test-fast-cancel-{}", uuid::Uuid::new_v4());
        let handle = tokio::spawn(async { std::future::pending::<()>().await });
        crate::registry::register_job(job_id.clone(), handle);

        let start = std::time::Instant::now();
        let was_aborted = crate::registry::abort_job(&job_id);
        let elapsed = start.elapsed();

        assert!(was_aborted);
        // Abort should be nearly instant (< 1ms), much faster than 250ms poll interval
        assert!(elapsed.as_millis() < 10);
    }
}
