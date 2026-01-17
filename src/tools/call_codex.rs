//! call_codex tool handler to call Codex CLI with persistence

use crate::clients::codex::{CodexClient, CodexExecution};
use crate::clients::traits::AgentError;
use crate::error::{Result, SurrealMindError};
use crate::registry;
use crate::server::SurrealMindServer;
use crate::utils::db::upsert_tool_session;
use once_cell::sync::Lazy;
use regex::Regex;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client as WsClient;

const DEFAULT_MODEL: &str = "gpt-5.2-codex";
const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_TOOL_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_CANCEL_POLL_MS: u64 = 250;

static INVALID_PROMPT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"record `(?P<id>agent_jobs:[^`]+)`").unwrap());

/// Parameters for the call_codex tool
#[derive(Debug, Deserialize)]
pub struct CallCodexParams {
    pub prompt: String,
    #[serde(default)]
    pub task_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Working directory for the Codex CLI subprocess
    #[serde(default)]
    pub cwd: Option<String>,
    /// Resume a specific Codex session id
    #[serde(default)]
    pub resume_session_id: Option<String>,
    /// Resume the latest Codex session
    #[serde(default)]
    pub continue_latest: bool,
    /// Timeout in milliseconds (outer call timeout)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Per-tool timeout in milliseconds (mapped to TOOL_TIMEOUT_SEC)
    #[serde(default)]
    pub tool_timeout_ms: Option<u64>,
    /// Expose streaming events in response metadata
    #[serde(default)]
    pub expose_stream: bool,
    /// If true, job is enqueued and not awaited (async-only for now)
    #[serde(default)]
    pub fire_and_forget: bool,
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
    Completed(std::result::Result<CodexExecution, AgentError>),
    Cancelled,
}

impl SurrealMindServer {
    /// Handle the call_codex tool call
    pub async fn handle_call_codex(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: CallCodexParams = serde_json::from_value(Value::Object(args)).map_err(|e| {
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

        let cwd = normalize_optional_string(params.cwd);
        let cwd = cwd.ok_or_else(|| SurrealMindError::InvalidParams {
            message: "cwd is required and cannot be empty".into(),
        })?;

        let resume_session_id = normalize_optional_string(params.resume_session_id);
        if resume_session_id.is_some() && params.continue_latest {
            return Err(SurrealMindError::InvalidParams {
                message: "resume_session_id and continue_latest cannot both be set".into(),
            });
        }

        let task_name =
            normalize_optional_string(params.task_name).unwrap_or_else(|| "call_codex".to_string());
        let model =
            normalize_optional_string(params.model).unwrap_or_else(|| DEFAULT_MODEL.to_string());
        let timeout = params.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        let tool_timeout = params.tool_timeout_ms.unwrap_or(DEFAULT_TOOL_TIMEOUT_MS);

        let job_id = uuid::Uuid::new_v4().to_string();

        create_job_record(
            self.db.as_ref(),
            job_id.clone(),
            "call_codex".to_string(),
            "codex".to_string(),
            "codex".to_string(),
            prompt.clone(),
            task_name.clone(),
            Some(model.clone()),
            Some(cwd.clone()),
            timeout,
            Some(tool_timeout),
            params.expose_stream,
            resume_session_id.clone(),
            params.continue_latest,
            params.fire_and_forget,
        )
        .await?;

        Ok(CallToolResult::structured(json!({
            "status": "queued",
            "job_id": job_id,
            "message": "Job queued for background execution. Use call_status to check progress."
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
    resume_session_id: Option<String>,
    continue_latest: bool,
    fire_and_forget: bool,
) -> Result<()> {
    let model_override_json: Value = model_override.map(Value::String).unwrap_or(Value::Null);
    let cwd_json: Value = cwd.map(Value::String).unwrap_or(Value::Null);
    let tool_timeout_json: Value = tool_timeout_ms.map(Value::from).unwrap_or(Value::Null);
    let resume_session_json: Value = resume_session_id.map(Value::String).unwrap_or(Value::Null);

    let sql = "CREATE agent_jobs SET job_id = $job_id, tool_name = $tool_name, agent_source = $agent_source, agent_instance = $agent_instance, prompt = $prompt, task_name = $task_name, model_override = $model_override, cwd = $cwd, timeout_ms = $timeout_ms, tool_timeout_ms = $tool_timeout_ms, expose_stream = $expose_stream, resume_session_id = $resume_session_id, continue_latest = $continue_latest, fire_and_forget = $fire_and_forget, status = $status, created_at = time::now();";
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
        .bind(("resume_session_id", resume_session_json))
        .bind(("continue_latest", continue_latest))
        .bind(("fire_and_forget", fire_and_forget))
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
    metadata: Option<Value>,
) -> Result<()> {
    let mut sql =
        "UPDATE agent_jobs SET status = $status, completed_at = time::now(), duration_ms = $duration_ms"
            .to_string();

    if session_id.is_some() {
        sql.push_str(", session_id = $session_id");
    }
    if exchange_id.is_some() {
        sql.push_str(", exchange_id = type::thing($exchange_id)");
    }
    if metadata.is_some() {
        sql.push_str(", metadata = $metadata");
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
    if let Some(meta) = metadata {
        query = query.bind(("metadata", meta));
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
    resume_session_id: Option<String>,
    continue_latest: Option<bool>,
}

pub async fn run_call_codex_worker(
    db: std::sync::Arc<Surreal<WsClient>>,
    semaphore: std::sync::Arc<tokio::sync::Semaphore>,
) {
    tracing::info!("call_codex worker started");
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
                eprintln!("[ERROR call_codex worker] Claim failed: {}", err_msg);
                tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
                continue;
            }
        };

        let Some(job) = claimed else {
            tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
            continue;
        };

        tracing::info!(job_id = %job.job_id, "call_codex worker claimed job");
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
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        let tool_timeout = job
            .tool_timeout_ms
            .and_then(|v| u64::try_from(v).ok())
            .unwrap_or(DEFAULT_TOOL_TIMEOUT_MS);

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
        let task_name = job.task_name.as_deref().unwrap_or("call_codex").trim();
        let cwd = job.cwd.as_deref().unwrap_or("").trim();
        if cwd.is_empty() {
            let _ = fail_job(
                db.as_ref(),
                &job.job_id,
                "Missing cwd in job metadata".to_string(),
                0,
            )
            .await;
            continue;
        }

        let started_at = chrono::Utc::now();
        let job_id = job.job_id.clone();
        let prompt = prompt.to_string();
        let task_name = task_name.to_string();
        let model_override = job.model_override.clone();
        let cwd = cwd.to_string();
        let expose_stream = job.expose_stream.unwrap_or(false);
        let resume_session_id = job.resume_session_id.clone();
        let continue_latest = job.continue_latest.unwrap_or(false);

        let prompt_for_job = prompt.clone();
        let mut handle = tokio::spawn(async move {
            execute_codex_call(CodexCallParams {
                prompt: prompt_for_job.as_str(),
                model_override: model_override.as_deref(),
                cwd: cwd.as_str(),
                timeout,
                tool_timeout,
                expose_stream,
                resume_session_id: resume_session_id.as_deref(),
                continue_latest,
            })
            .await
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
                        Err(err) => Err(AgentError::CliError(format!("call_codex task join error: {}", err))),
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

        registry::unregister_job(&job_id);

        match result {
            JobOutcome::Completed(Ok(execution)) => {
                let metadata = build_metadata(&execution, expose_stream);
                let session_id = execution.session_id.clone();
                let exchange_id = if let Some(sid) = execution
                    .session_id
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    match insert_exchange(
                        db.as_ref(),
                        &task_name,
                        &prompt,
                        &execution.response,
                        sid,
                        &metadata,
                    )
                    .await
                    {
                        Ok(id) => Some(id),
                        Err(e) => {
                            tracing::warn!(job_id = %job_id, "exchange insert failed: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };

                if let Some(ref sid) = session_id
                    && let Some(ref eid) = exchange_id
                {
                    let _ = upsert_tool_session(
                        db.as_ref(),
                        task_name.clone(),
                        sid.clone(),
                        eid.clone(),
                    )
                    .await;
                }

                if let Err(e) = complete_job(
                    db.as_ref(),
                    &job_id,
                    session_id,
                    exchange_id,
                    duration_ms,
                    metadata,
                )
                .await
                {
                    eprintln!("[ERROR call_codex worker] Failed to complete job: {}", e);
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
                        "[ERROR call_codex worker] Failed to mark job as failed: {}",
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
            "SELECT job_id, created_at FROM agent_jobs WHERE status = 'queued' AND tool_name = 'call_codex' AND type::is::string(prompt) AND prompt != '' AND type::is::string(cwd) AND cwd != '' ORDER BY created_at ASC LIMIT 1;",
        )
        .await?;
    let rows: Vec<QueuedJobIdRow> = response.take(0)?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };

    let mut response = db
        .query(
            "UPDATE agent_jobs SET status = 'running', started_at = time::now() WHERE job_id = $job_id AND status = 'queued' AND tool_name = 'call_codex' RETURN job_id, prompt, task_name, model_override, cwd, timeout_ms, tool_timeout_ms, expose_stream, resume_session_id, continue_latest;",
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
struct CodexCallParams<'a> {
    prompt: &'a str,
    model_override: Option<&'a str>,
    cwd: &'a str,
    timeout: u64,
    tool_timeout: u64,
    expose_stream: bool,
    resume_session_id: Option<&'a str>,
    continue_latest: bool,
}

async fn execute_codex_call(
    params: CodexCallParams<'_>,
) -> std::result::Result<CodexExecution, AgentError> {
    let model = params
        .model_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());

    let mut codex = CodexClient::new(Some(model));
    codex = codex
        .with_cwd(params.cwd)
        .with_tool_timeout_ms(params.tool_timeout);

    if let Some(resume) = params.resume_session_id {
        codex = codex.with_resume_session_id(resume.to_string());
    } else if params.continue_latest {
        codex = codex.with_continue_latest(true);
    }
    if params.expose_stream {
        codex = codex.with_expose_stream(true);
    }

    let timeout = std::time::Duration::from_millis(params.timeout);
    let execution = tokio::time::timeout(timeout, codex.execute(params.prompt))
        .await
        .map_err(|_| AgentError::Timeout {
            timeout_ms: params.timeout,
        })??;

    if execution.response.trim().is_empty() {
        return Err(AgentError::CliError(
            "Empty Codex response: no content captured.".to_string(),
        ));
    }

    Ok(execution)
}

fn build_metadata(execution: &CodexExecution, expose_stream: bool) -> Option<Value> {
    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "response".to_string(),
        Value::String(execution.response.clone()),
    );

    if expose_stream {
        metadata.insert(
            "stream_events".to_string(),
            Value::Array(execution.events.clone()),
        );
    }
    if !execution.stderr.trim().is_empty() {
        metadata.insert(
            "stderr".to_string(),
            Value::String(execution.stderr.clone()),
        );
    }
    if !execution.stdout.trim().is_empty() {
        metadata.insert(
            "stdout".to_string(),
            Value::String(execution.stdout.clone()),
        );
    }

    if metadata.is_empty() {
        None
    } else {
        Some(Value::Object(metadata))
    }
}

async fn insert_exchange(
    db: &Surreal<WsClient>,
    tool_name: &str,
    prompt: &str,
    response: &str,
    session_id: &str,
    metadata: &Option<Value>,
) -> Result<String> {
    #[derive(Debug, Deserialize)]
    struct IdResult {
        id: String,
    }

    let metadata_value = metadata.clone().unwrap_or(Value::Null);
    let sql = "CREATE agent_exchanges SET created_at = time::now(), agent_source = $arg_source, agent_instance = $instance, prompt = $prompt, response = $response, tool_name = $arg_tool, session_id = $arg_session, metadata = $metadata RETURN <string>id AS id;";
    let mut db_response = db
        .query(sql)
        .bind(("arg_source", "codex"))
        .bind(("instance", "codex"))
        .bind(("prompt", prompt.to_string()))
        .bind(("response", response.to_string()))
        .bind(("arg_tool", tool_name.to_string()))
        .bind(("arg_session", session_id.to_string()))
        .bind(("metadata", metadata_value))
        .await?;

    let created: Vec<IdResult> = db_response.take(0)?;
    let exchange_id =
        created
            .first()
            .map(|row| row.id.clone())
            .ok_or_else(|| SurrealMindError::Mcp {
                message: "missing exchange id".into(),
            })?;
    Ok(exchange_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_defaults() {
        assert_eq!(DEFAULT_MODEL, "gpt-5.2-codex");
        assert_eq!(DEFAULT_TIMEOUT_MS, 60_000);
        assert_eq!(DEFAULT_TOOL_TIMEOUT_MS, 300_000);
    }
}
