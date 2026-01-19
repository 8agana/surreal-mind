---
date: 2025-12-30
prompt type: Implementation Plan (Tool Refactor)
justification: To make the delegate_gemini tool fire and forget instead of blocking.
status: Complete
completion date: 2025-12-30
prompt doc: docs/prompts/20251230-delegate_gemini-async.md
related_docs:
  - docs/prompts/20251227-gemini-cli-implementation.md
  - docs/prompts/20251228-delegate-gemini-tool.md
  - docs/prompts/20251230-delegate-gemini-cwd.md
  - docs/prompts/20251230-delegate-gemini-timeout.md
---

# Planning for delegate_gemini tool async execution

**Conversation with Grok Code Fast 1**
## User

Will you take a look at the delegate_gemini tool and tell me how we could make if fire and forget instead of blocking?

**Tool Call: Search files for regex `delegate_gemini`**
Status: Completed

Showing matches 1-20 (there were more matches found; use offset: 20 to see next page):

## Matches in surreal-mind/.serena/memories/code_conventions.md

### L61-62
```
- Files match tool names: `delegate_gemini.rs` for `delegate_gemini` tool

```

## Matches in surreal-mind/.serena/memories/code_structure.md

### L27-31
```
│   ├── maintenance.rs   # maintenance_ops
│   ├── detailed_help.rs # detailed_help
│   ├── delegate_gemini.rs # delegate_gemini
│   └── curiosity.rs     # curiosity_add/get/search
├── clients/             # External CLI clients
```

## Matches in surreal-mind/.serena/memories/project_overview.md

### L28-29
```
6. `delegate_gemini` - Delegate to Gemini CLI
7. `curiosity_add/get/search` - Lightweight curiosity entries
```

## Matches in surreal-mind/CHANGELOG.md

### L14-15
```
- (2025-12-30) Removed `inner_voice` tool and all supporting code, tests, scripts, and documentation. The tool's retrieval + synthesis + auto-extract workflow has been replaced by `legacymind_search` + `delegate_gemini` combinations. Removed environment variables: `SURR_ENABLE_INNER_VOICE`, `SURR_DISABLE_INNER_VOICE`, `SURR_INNER_VOICE_*`, `SURR_IV_*`, `IV_ALLOW_GROK`, `INNER_VOICE_LOCAL_FALLBACK`. Removed Cargo dependencies: `blake3`, `unicode-normalization`. Removed scripts: `scripts/iv_extract.js`, `lib/iv_utils.js`. Updated tool roster to 9 tools.
- (2025-12-19) Fixed `recency_days` parameter in search tools - was being ignored, now properly filters by date.
```

## Matches in surreal-mind/README.md

### L69-70
```
- `delegate_gemini`: Delegate prompts to Gemini CLI with persisted exchange tracking. `prompt` required, optional `task_name`, `model`.
- `curiosity_add`: Add lightweight curiosity entries. `content` required, optional `tags[]`, `agent`, `topic`, `in_reply_to`.
```

## Matches in surreal-mind/docs/prompts/20251228-delegate-gemini-tool.md

### # Tool Implementation: `delegate_gemini` › L11-12
```
# Tool Implementation: `delegate_gemini`

```

### L13-15
```
Update (2025-12-30): This plan is now folded into the PersistedAgent middleware pivot.
`delegate_gemini` becomes a thin wrapper around PersistedAgent, which centralizes DB
persistence and session management for all federation agents.
```

### L18-20
```
Create a PersistedAgent middleware that wraps federation agent calls (Gemini, Codex, Grok,
Claude, etc.) and centralizes the **SubconsciousLink** (DB persistence + session management).
`delegate_gemini` is now a thin tool wrapper that forwards to PersistedAgent with agent metadata.
```

### ### 2.3 Thin Tool Wrapper (delegate_gemini) › L39-41
```
### 2.3 Thin Tool Wrapper (delegate_gemini)
**Location:** `src/tools/delegate_gemini.rs`
**Responsibility:** pass `agent_source=gemini` + model/task options to PersistedAgent and return IDs.
```

### L203-207
```
3.  **Thin Tool Wrapper:** `delegate_gemini` passes `agent_source=gemini` into PersistedAgent:
    - Get or create session (atomic)
    - Invoke agent via PersistedAgent
    - Return response + IDs to caller
4.  **Registration:** Register the wrapper in `src/server/router.rs` and `src/tools/mod.rs`
```

### L263-265
```
`delegate_gemini` is now a thin wrapper on top of PersistedAgent. Additional tools
(`delegate_codex`, `delegate_grok`, etc.) follow the same wrapper pattern, while
PersistedAgent remains the single owner of persistence and session logic.
```

### L270-271
```
- Single delegate_gemini calls succeed ✓
- Exchange persists to agent_exchanges table ✓
```

## Matches in surreal-mind/docs/prompts/20251230-db-schema-implementation.md

### L23-33
```
DEFINE TABLE agent_exchanges SCHEMAFULL;
DEFINE FIELD id ON agent_exchanges TYPE record<agent_exchanges>;
DEFINE FIELD agent_source ON agent_exchanges TYPE string;       -- 'gemini', 'claude'
DEFINE FIELD agent_instance ON agent_exchanges TYPE string;     -- 'gemini-2.5-pro'
DEFINE FIELD prompt ON agent_exchanges TYPE string;
DEFINE FIELD response ON agent_exchanges TYPE string;
DEFINE FIELD tool_name ON agent_exchanges TYPE string;          -- 'delegate_gemini'
DEFINE FIELD session_id ON agent_exchanges TYPE string;         -- The CLI session ID
DEFINE FIELD metadata ON agent_exchanges TYPE object;
DEFINE FIELD created_at ON agent_exchanges TYPE datetime DEFAULT time::now();

```

3 lines remaining in ancestor node. Read the file to see all.

## Matches in surreal-mind/docs/prompts/20251230-delegate-gemini-cwd.md

### # delegate_gemini cwd Parameter Implementation › L1-2
```
# delegate_gemini cwd Parameter Implementation

```

### L9-10
```
When `delegate_gemini` spawns Gemini CLI, it inherits surreal-mind's working directory (wherever the MCP server was started from). This prevents Gemini from accessing files relative to the caller's actual working directory.

```

### L11-12
```
For Federation operations where Claude Code calls delegate_gemini from `/Users/samuelatagana/Projects/SomeProject`, Gemini would try to resolve relative paths from `/Users/samuelatagana/Projects/LegacyMind/surreal-mind` instead.

```

### L15-16
```
Add optional `cwd` parameter to delegate_gemini that gets passed through to `std::process::Command::current_dir()`.

```

### ### 2. src/tools/delegate_gemini.rs › L26-27
```
### 2. src/tools/delegate_gemini.rs
- Added `cwd: Option<String>` field to `DelegateGeminiParams`
```

### L32-34
```
- Added `"cwd": {"type": "string"}` to delegate_gemini_schema properties

## Remaining Work
```

### L38-39
```
3. Test actual functionality (call delegate_gemini with cwd param, verify Gemini runs in correct directory)
4. Commit with proper message
```


**Tool Call: 1 match**
Status: Completed

file:///Users/samuelatagana/Projects/LegacyMind/surreal-mind/src/tools/delegate_gemini.rs

**Tool Call: Read file `src/tools/delegate_gemini.rs`**
Status: Completed

```surreal-mind/src/tools/delegate_gemini.rs
//! delegate_gemini tool handler to call Gemini CLI with persistence

use crate::clients::{AgentError, GeminiClient, PersistedAgent};
use crate::clients::traits::CognitiveAgent;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{json, Value};
use surrealdb::engine::remote::ws::Client as WsClient;
use surrealdb::Surreal;

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
            serde_json::from_value(Value::Object(args)).map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
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
            .unwrap_or_else(|| default_model_name());

        let resume_session = fetch_last_session_id(self.db.as_ref(), task_name.clone()).await?;
        let cwd = normalize_optional_string(params.cwd);
        let timeout = params.timeout_ms.unwrap_or_else(gemini_timeout_ms);

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

```


**Tool Call: No matches**
Status: Completed

```json
{"offset":0,"current_matches_page":[],"all_matches_len":0}
```

**Tool Call: Search files for regex `PersistedAgent`**
Status: Completed

Found 6 matches:

## Matches in surreal-mind/src/clients/mod.rs

### L1-8
```
pub mod gemini;
pub mod persisted;
pub mod traits;

pub use gemini::GeminiClient;
pub use persisted::PersistedAgent;
pub use traits::{AgentError, AgentResponse, CognitiveAgent};

```

## Matches in surreal-mind/src/clients/persisted.rs

### pub struct PersistedAgent › L12-19
```
pub struct PersistedAgent<A: CognitiveAgent> {
    agent: A,
    db: Arc<Surreal<WsClient>>,
    agent_source: String,
    agent_instance: String,
    tool_name: String,
    base_metadata: Map<String, Value>,
}
```

### impl PersistedAgent<A> › L32-42
```
impl<A: CognitiveAgent> PersistedAgent<A> {
    pub fn new(
        agent: A,
        db: Arc<Surreal<WsClient>>,
        agent_source: impl Into<String>,
        agent_instance: impl Into<String>,
        tool_name: impl Into<String>,
    ) -> Self {
        Self {
            agent,
            db,
```

18 lines remaining in ancestor node. Read the file to see all.

### impl CognitiveAgent for PersistedAgent<A> › L63-73
```
impl<A: CognitiveAgent> CognitiveAgent for PersistedAgent<A> {
    async fn call(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<AgentResponse, AgentError> {
        let context_sql = "SELECT prompt, response, created_at FROM agent_exchanges WHERE tool_name = $tool_name ORDER BY created_at ASC;";
        let mut context_response = self
            .db
            .query(context_sql)
            .bind(("tool_name", self.tool_name.clone()))
```

70 lines remaining in ancestor node. Read the file to see all.

## Matches in surreal-mind/src/tools/delegate_gemini.rs

### L1-11
```
//! delegate_gemini tool handler to call Gemini CLI with persistence

use crate::clients::{AgentError, GeminiClient, PersistedAgent};
use crate::clients::traits::CognitiveAgent;
use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::{json, Value};
use surrealdb::engine::remote::ws::Client as WsClient;
use surrealdb::Surreal;
```

144 lines remaining in ancestor node. Read the file to see all.

### impl SurrealMindServer › pub async fn handle_delegate_gemini › L75-79
```
            gemini = gemini.with_cwd(dir);
        }
        let agent = PersistedAgent::new(
            gemini,
            self.db.clone(),
```


**Tool Call: Read file `src/clients/persisted.rs`**
Status: Completed

```surreal-mind/src/clients/persisted.rs
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Map, Value};
use surrealdb::engine::remote::ws::Client as WsClient;
use surrealdb::Surreal;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};
use crate::utils::db::upsert_tool_session;

pub struct PersistedAgent<A: CognitiveAgent> {
    agent: A,
    db: Arc<Surreal<WsClient>>,
    agent_source: String,
    agent_instance: String,
    tool_name: String,
    base_metadata: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct IdResult {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ExchangeRow {
    prompt: String,
    response: String,
}

impl<A: CognitiveAgent> PersistedAgent<A> {
    pub fn new(
        agent: A,
        db: Arc<Surreal<WsClient>>,
        agent_source: impl Into<String>,
        agent_instance: impl Into<String>,
        tool_name: impl Into<String>,
    ) -> Self {
        Self {
            agent,
            db,
            agent_source: agent_source.into(),
            agent_instance: agent_instance.into(),
            tool_name: tool_name.into(),
            base_metadata: Map::new(),
        }
    }

    fn build_metadata(&self, resume_session: Option<&str>) -> Value {
        let mut metadata = self.base_metadata.clone();
        if let Some(resume) = resume_session {
            metadata.insert(
                "resume_session_id".to_string(),
                Value::String(resume.to_string()),
            );
        }
        Value::Object(metadata)
    }
}

#[async_trait]
impl<A: CognitiveAgent> CognitiveAgent for PersistedAgent<A> {
    async fn call(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<AgentResponse, AgentError> {
        let context_sql = "SELECT prompt, response, created_at FROM agent_exchanges WHERE tool_name = $tool_name ORDER BY created_at ASC;";
        let mut context_response = self
            .db
            .query(context_sql)
            .bind(("tool_name", self.tool_name.clone()))
            .await
            .map_err(|e| AgentError::CliError(format!("db context query failed: {}", e)))?;
        let exchanges: Vec<ExchangeRow> = context_response
            .take::<Vec<ExchangeRow>>(0)
            .map_err(|e| AgentError::CliError(format!("db context response failed: {}", e)))?;

        let context = if exchanges.is_empty() {
            None
        } else {
            let mut assembled = String::new();
            for (idx, exchange) in exchanges.iter().enumerate() {
                assembled.push_str(&format!(
                    "Previous exchange {}:\nUser: {}\nAssistant: {}\n\n",
                    idx + 1,
                    exchange.prompt,
                    exchange.response
                ));
            }
            Some(assembled)
        };

        let prompt_to_send = if let Some(context) = context {
            format!("{context}\n\nCurrent question:\n{prompt}")
        } else {
            prompt.to_string()
        };

        let mut response = self.agent.call(&prompt_to_send, None).await?;
        let metadata = self.build_metadata(session_id);

        let sql = "CREATE agent_exchanges SET created_at = time::now(), agent_source = $arg_source, agent_instance = $instance, prompt = $prompt, response = $response, tool_name = $arg_tool, session_id = $arg_session, metadata = $metadata RETURN <string>id AS id;";
        let mut db_response = self
            .db
            .query(sql)
            .bind(("arg_source", self.agent_source.clone()))
            .bind(("instance", self.agent_instance.clone()))
            .bind(("prompt", prompt.to_string()))
            .bind(("response", response.response.clone()))
            .bind(("arg_tool", self.tool_name.clone()))
            .bind(("arg_session", response.session_id.clone()))
            .bind(("metadata", metadata))
            .await
            .map_err(|e| AgentError::CliError(format!("db insert failed: {}", e)))?;

        eprintln!(
            "[DEBUG persisted.rs] Raw SurrealDB response: {:?}",
            db_response
        );

        let created: Vec<IdResult> = db_response
            .take::<Vec<IdResult>>(0)
            .map_err(|e| AgentError::CliError(format!("db response failed: {}", e)))?;

        let exchange_id = created.first().map(|row| row.id.clone()).ok_or_else(|| {
            AgentError::ParseError(format!("missing exchange id; created={:?}", created))
        })?;

        upsert_tool_session(
            self.db.as_ref(),
            self.tool_name.clone(),
            response.session_id.clone(),
            exchange_id.to_string(),
        )
        .await
        .map_err(|e| AgentError::CliError(format!("db tool session upsert failed: {}", e)))?;

        response.exchange_id = Some(exchange_id.clone());
        Ok(response)
    }
}

```


**Tool Call: Read file `src/clients/gemini.rs`**
Status: Completed

```surreal-mind/src/clients/gemini.rs
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::time::timeout;

use crate::clients::traits::{AgentError, AgentResponse, CognitiveAgent};

const STDERR_CAP_BYTES: usize = 10 * 1024;
const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_MODEL: &str = "gemini-2.5-pro";

#[derive(Debug, Deserialize)]
pub struct GeminiResponse {
    pub session_id: String,
    pub response: String,
}

#[derive(Debug, Clone)]
pub struct GeminiClient {
    model: String,
    timeout: Duration,
    cwd: Option<PathBuf>,
}

impl Default for GeminiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiClient {
    pub fn new() -> Self {
        let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let timeout_ms = std::env::var("GEMINI_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        Self {
            model,
            timeout: Duration::from_millis(timeout_ms),
            cwd: None,
        }
    }

    pub fn with_timeout_ms(model: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            model: model.into(),
            timeout: Duration::from_millis(timeout_ms),
            cwd: None,
        }
    }

    /// Set the working directory for the Gemini CLI subprocess
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }
}

#[async_trait]
impl CognitiveAgent for GeminiClient {
    async fn call(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<AgentResponse, AgentError> {
        let mut cmd = Command::new("gemini");
        cmd.kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-m")
            .arg(&self.model)
            .arg("-e")
            .arg("")
            .arg("-o")
            .arg("json");
        if let Some(sid) = session_id {
            cmd.arg("--resume").arg(sid);
        }
        cmd.arg(prompt);
        if let Some(ref dir) = self.cwd {
            cmd.current_dir(dir);
        }
        let mut child = cmd.spawn().map_err(map_spawn_err)?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::CliError("stdout unavailable".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AgentError::CliError("stderr unavailable".to_string()))?;
        let stdout_task = tokio::spawn(async move { read_all(stdout).await });
        let stderr_task =
            tokio::spawn(async move { read_with_cap(stderr, STDERR_CAP_BYTES).await });

        let status = match timeout(self.timeout, child.wait()).await {
            Ok(res) => res.map_err(|e| AgentError::CliError(e.to_string()))?,
            Err(_) => {
                let _ = child.kill().await;
                stdout_task.abort();
                stderr_task.abort();
                return Err(AgentError::Timeout {
                    timeout_ms: self.timeout.as_millis() as u64,
                });
            }
        };
        let stdout_bytes = stdout_task
            .await
            .map_err(|e| AgentError::CliError(e.to_string()))?
            .map_err(|e| AgentError::CliError(e.to_string()))?;
        let stderr_bytes = stderr_task
            .await
            .map_err(|e| AgentError::CliError(e.to_string()))?
            .map_err(|e| AgentError::CliError(e.to_string()))?;

        if !status.success() {
            let stderr_str = String::from_utf8_lossy(&stderr_bytes);
            return Err(AgentError::CliError(format!(
                "gemini exit {}: {}",
                status,
                stderr_str.trim(),
            )));
        }

        let stdout_str = String::from_utf8_lossy(&stdout_bytes);
        let response = parse_gemini_response(&stdout_str)?;
        let cleaned = strip_ansi_codes(&response.response).trim().to_string();

        Ok(AgentResponse {
            session_id: response.session_id,
            response: cleaned,
            exchange_id: None,
        })
    }
}

fn map_spawn_err(err: std::io::Error) -> AgentError {
    if err.kind() == std::io::ErrorKind::NotFound {
        AgentError::NotFound
    } else {
        AgentError::CliError(err.to_string())
    }
}

async fn read_all<R: AsyncRead + Unpin>(mut reader: R) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await?;
    Ok(buf)
}

async fn read_with_cap<R: AsyncRead + Unpin>(
    mut reader: R,
    cap: usize,
) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = reader.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        let remaining = cap.saturating_sub(buf.len());
        if remaining > 0 {
            let take = remaining.min(n);
            buf.extend_from_slice(&chunk[..take]);
        }
        if buf.len() >= cap {
            while reader.read(&mut chunk).await? != 0 {}
            break;
        }
    }
    Ok(buf)
}

fn parse_gemini_response(output: &str) -> Result<GeminiResponse, AgentError> {
    let cleaned = strip_ansi_codes(output);
    if let Ok(resp) = serde_json::from_str::<GeminiResponse>(&cleaned) {
        return Ok(resp);
    }

    let candidates = extract_json_candidates(&cleaned);
    for candidate in candidates.iter().rev() {
        if let Ok(resp) = serde_json::from_str::<GeminiResponse>(candidate) {
            return Ok(resp);
        }
    }

    let snippet = truncate_chars(cleaned.trim(), 500);
    Err(AgentError::ParseError(format!(
        "no valid JSON object found in Gemini output: {}",
        snippet
    )))
}

fn extract_json_candidates(text: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut depth: u32 = 0;
    let mut start: Option<usize> = None;
    let mut in_string = false;
    let mut escape = false;

    for (idx, ch) in text.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start.take() {
                            candidates.push(text[s..idx + 1].to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    candidates
}

fn strip_ansi_codes(input: &str) -> String {
    static ANSI_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").unwrap());
    ANSI_RE.replace_all(input, "").to_string()
}

fn truncate_chars(input: &str, max: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in input.chars().enumerate() {
        if idx >= max {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

```


## Assistant

To make the `delegate_gemini` tool "fire and forget" instead of blocking, we need to modify the handler so that it spawns an asynchronous task to perform the actual work (calling Gemini CLI and persisting the result to the database) without waiting for completion. The tool would then return an immediate acknowledgment with a job ID, allowing the client to continue without blocking.

Here's how this could be implemented:

### Proposed Changes

1. **Modify the `DelegateGeminiParams` struct** to include an optional flag to enable fire-and-forget mode:
   - Add `fire_and_forget: Option<bool>` (defaults to `false` for backward compatibility).

2. **Update the `handle_delegate_gemini` method**:
   - If `fire_and_forget` is `true`, generate a unique job ID (e.g., using UUID or timestamp-based string).
   - Spawn an asynchronous task using `tokio::spawn` that performs the full `agent.call()` operation in the background.
   - Return immediately with a response containing the job ID and a status message.
   - If `fire_and_forget` is `false` (default), maintain the current synchronous behavior.

3. **Add logic to generate a job ID**:
   - Use something like `uuid::Uuid::new_v4()` or a simple counter/timestamp for uniqueness.

4. **Handle the background task**:
   - The spawned task would do the same as the current code: query context, call Gemini, persist to DB.
   - Since it's fire-and-forget, errors in the background task could be logged but wouldn't affect the immediate response. You might want to add error handling/logging within the task.

5. **Update the MCP schema** for the tool to include the new parameter and adjusted response format.

### Code Outline

Here's a rough sketch of the updated `handle_delegate_gemini` method (assumes adding `uuid` as a dependency for job IDs):

```rust
// In DelegateGeminiParams
#[derive(Debug, Deserialize)]
pub struct DelegateGeminiParams {
    pub prompt: String,
    #[serde(default)]
    pub task_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub fire_and_forget: Option<bool>,  // New field
}

// In handle_delegate_gemini
pub async fn handle_delegate_gemini(
    &self,
    request: CallToolRequestParam,
) -> Result<CallToolResult> {
    let args = request.arguments.ok_or_else(|| /* ... */ )?;
    let params: DelegateGeminiParams = /* ... */;
    
    let fire_and_forget = params.fire_and_forget.unwrap_or(false);
    
    // ... existing validation and setup ...
    
    if fire_and_forget {
        // Generate job ID
        let job_id = uuid::Uuid::new_v4().to_string();
        
        // Clone necessary data for the background task
        let agent = PersistedAgent::new(/* ... */);
        let prompt = prompt.clone();
        let resume_session = resume_session.clone();
        
        // Spawn background task
        tokio::spawn(async move {
            match agent.call(&prompt, resume_session.as_deref()).await {
                Ok(response) => {
                    // Log success or handle as needed
                    tracing::info!("Fire-and-forget task completed for job {}", job_id);
                }
                Err(e) => {
                    // Log error
                    tracing::error!("Fire-and-forget task failed for job {}: {}", job_id, e);
                }
            }
        });
        
        // Return immediate response
        Ok(CallToolResult::structured(json!({
            "status": "queued",
            "job_id": job_id,
            "message": "Gemini delegation started in background"
        })))
    } else {
        // Existing synchronous logic
        let response = agent.call(&prompt, resume_session.as_deref()).await.map_err(map_agent_error)?;
        Ok(CallToolResult::structured(json!({
            "response": response.response,
            "session_id": response.session_id,
            "exchange_id": response.exchange_id
        })))
    }
}
```

### Considerations

- **Result Retrieval**: Since MCP doesn't natively support asynchronous result polling, clients wouldn't automatically receive the Gemini response. You might need to add a companion tool (e.g., `query_delegate_result`) that queries the database for the result by job ID or session ID.
- **Error Handling**: Background tasks should have proper logging for failures, as errors won't propagate back to the client.
- **Persistence**: Ensure the background task still performs all DB operations (persisting exchanges, updating tool sessions).
- **Timeouts and Resources**: Monitor for resource leaks (e.g., too many background tasks) and ensure timeouts are still respected in the background.
- **Testing**: Thoroughly test both synchronous and asynchronous modes to ensure DB consistency and no race conditions.

This approach allows the tool to initiate long-running Gemini operations without blocking the client, while maintaining full functionality for synchronous use. If you'd like me to implement this change or refine the details, let me know!

---
**status**: Complete
**completion date**: 2025-12-30
**prompt doc**: docs/prompts/20251230-delegate_gemini-async.md
**related_docs**:
  - docs/prompts/20251227-gemini-cli-implementation.md
  - docs/prompts/20251228-delegate-gemini-tool.md
  - docs/prompts/20251230-delegate-gemini-cwd.md
  - docs/prompts/20251230-delegate-gemini-timeout.md
**next docs**:
  - docs/prompts/20251230-delegate_gemini-async-codex.md
