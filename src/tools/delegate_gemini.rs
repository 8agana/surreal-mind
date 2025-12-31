//! delegate_gemini tool handler to call Gemini CLI with persistence

use crate::clients::traits::CognitiveAgent;
use crate::clients::{AgentError, GeminiClient, PersistedAgent};
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client as WsClient;

const DEFAULT_MODEL: &str = "gemini-2.5-pro";
const DEFAULT_TIMEOUT_MS: u64 = 60_000;

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
    /// Fire and forget mode - spawn async background task
    #[serde(default)]
    pub fire_and_forget: bool,
}

#[derive(Debug, Deserialize)]
struct SessionResult {
    #[serde(default)]
    last_agent_session_id: Option<String>,
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
        let model = model_override.clone().unwrap_or_else(default_model_name);
        let cwd = normalize_optional_string(params.cwd);
        let timeout = params.timeout_ms.unwrap_or_else(gemini_timeout_ms);
        let fire_and_forget = params.fire_and_forget;

        if fire_and_forget {
            // Async path: spawn background task
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
            )
            .await?;

            Ok(CallToolResult::structured(json!({
                "status": "queued",
                "job_id": job_id,
                "message": "Job queued for background execution. Use agent_job_status to check progress."
            })))
        } else {
            // Sync path: existing behavior
            let resume_session = fetch_last_session_id(self.db.as_ref(), task_name.clone()).await?;

            let mut gemini = match model_override {
                Some(custom) => GeminiClient::with_timeout_ms(custom, timeout),
                None => GeminiClient::with_timeout_ms(default_model_name(), timeout),
            };
            if let Some(ref dir) = cwd {
                gemini = gemini.with_cwd(dir);
            }
            let agent = PersistedAgent::new(
                gemini,
                self.db.clone(),
                "gemini",
                model.clone(),
                task_name.clone(),
            );

            let response = agent
                .call(&prompt, resume_session.as_deref())
                .await
                .map_err(map_agent_error)?;

            Ok(CallToolResult::structured(json!({
                "response": response.response,
                "session_id": response.session_id,
                "exchange_id": response.exchange_id
            })))
        }
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

fn default_model_name() -> String {
    std::env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
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

fn map_agent_error(err: AgentError) -> SurrealMindError {
    match err {
        AgentError::Timeout { timeout_ms } => SurrealMindError::Timeout {
            operation: "delegate_gemini".to_string(),
            timeout_ms,
        },
        AgentError::CliError(message) => SurrealMindError::Internal {
            message: format!("delegate_gemini failed: {}", message),
        },
        AgentError::ParseError(message) => SurrealMindError::Serialization {
            message: format!("delegate_gemini parse error: {}", message),
        },
        AgentError::StdinError(message) => SurrealMindError::Internal {
            message: format!("delegate_gemini stdin error: {}", message),
        },
        AgentError::NotFound => SurrealMindError::Internal {
            message: "delegate_gemini failed: gemini cli not found".to_string(),
        },
    }
}

// Helper functions for async job management

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
) -> Result<()> {
    // Convert Option<String> to JSON values that SurrealDB can handle
    let model_override_json: Value = model_override.map(Value::String).unwrap_or(Value::Null);
    let cwd_json: Value = cwd.map(Value::String).unwrap_or(Value::Null);
    
    let sql = "CREATE agent_jobs SET job_id = $job_id, tool_name = $tool_name, agent_source = $agent_source, agent_instance = $agent_instance, prompt = $prompt, task_name = $task_name, model_override = $model_override, cwd = $cwd, timeout_ms = $timeout_ms, status = 'queued', created_at = time::now();";
    let mut response = db.query(sql)
        .bind(("job_id", job_id))
        .bind(("tool_name", tool_name))
        .bind(("agent_source", agent_source))
        .bind(("agent_instance", agent_instance))
        .bind(("prompt", prompt))
        .bind(("task_name", task_name))
        .bind(("model_override", model_override_json))
        .bind(("cwd", cwd_json))
        .bind(("timeout_ms", timeout_ms as i64))
        .await?;
    let _: Vec<serde_json::Value> = response.take(0)?;
    Ok(())
}

async fn complete_job(
    db: &Surreal<WsClient>,
    job_id: String,
    session_id: Option<String>,
    exchange_id: Option<String>,
    duration_ms: i64,
) -> Result<()> {
    let mut sql = "UPDATE agent_jobs SET status = 'completed', completed_at = time::now(), duration_ms = $duration_ms".to_string();

    if session_id.is_some() {
        sql.push_str(", session_id = $session_id");
    }
    if exchange_id.is_some() {
        sql.push_str(", exchange_id = $exchange_id");
    }
    sql.push_str(" WHERE job_id = $job_id;");

    let mut query = db
        .query(&sql)
        .bind(("job_id", job_id))
        .bind(("duration_ms", duration_ms));

    if let Some(sid) = session_id {
        query = query.bind(("session_id", sid));
    }
    if let Some(eid) = exchange_id {
        query = query.bind(("exchange_id", eid));
    }

    let mut response = query.await?;
    let _: Vec<serde_json::Value> = response.take(0)?;
    Ok(())
}

async fn fail_job(
    db: &Surreal<WsClient>,
    job_id: String,
    error: String,
    duration_ms: i64,
) -> Result<()> {
    let sql = "UPDATE agent_jobs SET status = 'failed', error = $error, completed_at = time::now(), duration_ms = $duration_ms WHERE job_id = $job_id;";
    let mut response = db.query(sql)
        .bind(("job_id", job_id))
        .bind(("error", error))
        .bind(("duration_ms", duration_ms))
        .await?;
    let _: Vec<serde_json::Value> = response.take(0)?;
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
    prompt: String,
    task_name: String,
    model_override: Option<String>,
    cwd: Option<String>,
    timeout_ms: Option<i64>,
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

    loop {
        let claimed = match claim_next_job(db.as_ref()).await {
            Ok(job) => job,
            Err(e) => {
                eprintln!("[ERROR delegate_gemini worker] Claim failed: {}", e);
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

        let timeout = job
            .timeout_ms
            .and_then(|v| u64::try_from(v).ok())
            .unwrap_or_else(gemini_timeout_ms);

        if job.prompt.trim().is_empty() {
            let _ = fail_job(
                db.as_ref(),
                job.job_id,
                "Missing prompt in job metadata".to_string(),
                0,
            )
            .await;
            continue;
        }

        let started_at = chrono::Utc::now();
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout),
            execute_gemini_call(
                db.clone(),
                &job.prompt,
                &job.task_name,
                job.model_override.as_deref(),
                job.cwd.as_deref(),
                timeout,
            ),
        )
        .await;

        let completed_at = chrono::Utc::now();
        let duration_ms = (completed_at - started_at).num_milliseconds();

        match result {
            Ok(Ok(response)) => {
                if let Err(e) = complete_job(
                    db.as_ref(),
                    job.job_id,
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
                }
            }
            Ok(Err(agent_err)) => {
                let error_msg = format!("Agent error: {}", agent_err);
                if let Err(e) = fail_job(db.as_ref(), job.job_id, error_msg, duration_ms).await {
                    eprintln!(
                        "[ERROR delegate_gemini worker] Failed to mark job as failed: {}",
                        e
                    );
                }
            }
            Err(_) => {
                let error_msg = format!("Timeout after {}ms", timeout);
                if let Err(e) = fail_job(db.as_ref(), job.job_id, error_msg, duration_ms).await {
                    eprintln!(
                        "[ERROR delegate_gemini worker] Failed to mark job as timed out: {}",
                        e
                    );
                }
            }
        }
    }
}

async fn claim_next_job(db: &Surreal<WsClient>) -> Result<Option<QueuedJobRow>> {
    let mut response = db
        .query(
            "SELECT job_id, created_at FROM agent_jobs WHERE status = 'queued' AND prompt IS NOT NONE ORDER BY created_at ASC LIMIT 1;",
        )
        .await?;
    let rows: Vec<QueuedJobIdRow> = response.take(0)?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };

    let mut response = db
        .query(
            "UPDATE agent_jobs SET status = 'running', started_at = time::now() WHERE job_id = $job_id AND status = 'queued' AND prompt IS NOT NONE RETURN job_id, prompt, task_name, model_override, cwd, timeout_ms;",
        )
        .bind(("job_id", row.job_id.clone()))
        .await?;
    let rows: Vec<QueuedJobRow> = response.take(0)?;
    Ok(rows.into_iter().next())
}

async fn execute_gemini_call(
    db: std::sync::Arc<Surreal<WsClient>>,
    prompt: &str,
    task_name: &str,
    model_override: Option<&str>,
    cwd: Option<&str>,
    timeout: u64,
) -> std::result::Result<crate::clients::traits::AgentResponse, AgentError> {
    let resume_session = fetch_last_session_id(db.as_ref(), task_name.to_string())
        .await
        .map_err(|e| AgentError::CliError(format!("Failed to fetch session: {}", e)))?;

    let model = model_override
        .map(|s| s.to_string())
        .unwrap_or_else(default_model_name);

    let mut gemini = GeminiClient::with_timeout_ms(model.clone(), timeout);
    if let Some(dir) = cwd {
        gemini = gemini.with_cwd(dir);
    }

    let agent = PersistedAgent::new(gemini, db.clone(), "gemini", model, task_name.to_string());

    agent.call(prompt, resume_session.as_deref()).await
}
