# notebooklm-research

**Status:** Planning
**Date:** 2026-01-15
**Investigator:** NotebookLM

## Overview

Loaded a notebook with information about Codex, Claude Code, Gemini CLI, and Mistral Vibe. As well as information about rmcp and the following github repos (keep in mind that nblm can only reference the markdowns in these repos):

- https://github.com/steipete/claude-code-mcp
- https://github.com/tuannvm/codex-mcp-server
- https://github.com/jamubc/gemini-mcp-tool
- https://github.com/centminmod/gemini-cli-mcp-server
- https://github.com/8agana/surreal-mind

## Prompt 1

"For each CLI (Claude Code, Codex, Gemini CLI, Vibe), provide a comparison table with exact flags for: non-interactive/headless execution, passing prompts as arguments, suppressing decorative output, specifying working directory, model selection, session continuity, and bypassing interactive confirmations - include example invocations."

## Response 1

"The following table compares the command-line flags and arguments for **Claude Code**, **Codex**, **Gemini CLI**, and **Mistral Vibe**, focusing on non-interactive and automated workflows.

### CLI Configuration & Automation Flags

| Feature | **Claude Code** (Anthropic) | **Codex CLI** (OpenAI) | **Gemini CLI** (Google) | **Mistral Vibe** (Mistral AI) |
| :--- | :--- | :--- | :--- | :--- |
| **Non-Interactive / Headless** | `-p "prompt"` | `exec "prompt"` | `-p "prompt"` or pos. arg | `--prompt "prompt"` |
| **Suppress Decorative Output** | `--output-format stream-json` or `--json` | `--json` (emits newline-delimited JSON) | `--output-format json` | *Standard stdout* (text) |
| **Bypass Confirmations** | `--dangerously-skip-permissions` | `--full-auto` or `--yolo` | `--yolo` or `--approval-mode=yolo` | `--auto-approve` |
| **Specify Model** | `--model <name>` | `--model <name>` or `-m` | `--model <name>` or `-m` | Config via `active_model` or `--agent` |
| **Set Working Directory** | *Use `cd` before running* or `additionalDirectories` setting | `--cd <path>` or `-C` | `--include-directories` (context only) | *Use `cd` before running* |
| **Session Continuity** | `--resume` or `-c` | `resume [ID]` or `resume --last` | `--resume [ID]` or `-r` | *Persistent history enabled by default* |

---

### Example Invocations

#### **1. Claude Code**
Use the `-p` flag for headless execution and `--dangerously-skip-permissions` for "Safe YOLO mode" in isolated environments (like containers).
```bash
# Analyze a file and output JSON for a pipeline
claude -p "Analyze metrics in @app.log and suggest fixes" \
  --dangerously-skip-permissions \
  --output-format stream-json
```
*Source:,,*

#### **2. Codex CLI**
Codex uses the `exec` subcommand for scripting. The `--full-auto` flag sets permissions to `workspace-write` and approvals to `on-request` (or use `--yolo` for no approvals).
```bash
# Resume the last session to fix a CI failure non-interactively
codex exec resume --last "Fix the specific race condition found in tests" \
  --full-auto \
  --cd ./backend
```
*Source:,,*

#### **3. Gemini CLI**
Gemini CLI supports positional arguments for prompts and a dedicated `--yolo` flag for skipping confirmation dialogs.
```bash
# Generate a markdown summary using Gemini 3 Pro
gemini -p "Summarize the changes in @src/main.ts" \
  --model gemini-3-pro-preview \
  --yolo \
  --output-format json
```
*Source:,,*

#### **4. Mistral Vibe**
Vibe is designed for piping and programmatic use via `--prompt`, often combined with `--auto-approve` for autonomous refactoring.
```bash
# Refactor code without entering interactive mode
vibe --prompt "Refactor the main function in cli/main.py to be more modular" \
  --auto-approve
```
*Source:,,*

### Key Differences & Notes
*   **Structured Output:** **Claude Code**, **Codex**, and **Gemini CLI** all offer JSON output modes (`--json` or `--output-format json`), which are essential for parsing results in CI/CD pipelines. Mistral Vibe currently focuses on text-based standard output for piping.
*   **Directory Scoping:** **Codex** allows explicit directory switching via the `--cd` flag. **Gemini CLI** and **Claude Code** typically rely on the shell's current working directory, though Gemini allows adding extra context via `--include-directories`.
*   **Safety & Permissions:** All CLIs default to asking for permission. **Codex** offers granular "levels" (read-only, workspace-write, full-auto). **Claude** and **Gemini** use explicit flags (`--dangerously-skip-permissions` and `--yolo`) to bypass these checks completely."

## Prompt 2

"Based on the MCP server implementations in the sources (codex-cli-mcp, gemini-cli-mcp, claude-code-mcp, ht-mcp), what patterns emerge for wrapping CLIs as MCP tools - specifically subprocess execution (sync vs async), output capture, error handling, parameter exposure, and caching strategies - and what's the best implementation strategy for building unified call_* delegation tools?"

## Response 2

" Based on the implementations of `codex-cli-mcp`, `gemini-cli-mcp`, `claude-code-mcp`, and `ht-mcp`, the following patterns and best practices emerge for wrapping Command Line Interfaces (CLIs) as Model Context Protocol (MCP) servers.

### Patterns for Wrapping CLIs as MCP Tools

#### 1. Subprocess Execution: Async Pools & Direct Execution
To prevent blocking the MCP server's main loop while waiting for CLI tools to finish, implementations favor asynchronous execution and process pooling.

*   **Process Pooling:** The `gemini-cli-mcp` server utilizes a configurable `ProcessPoolExecutor`. This approach reduces the overhead of subprocess creation by 100-500ms, significantly improving concurrency for high-frequency tool calls.
*   **Direct Execution vs. Shell:** To mitigate shell injection vulnerabilities, robust implementations avoid passing commands as raw strings to a shell. Instead, they use direct subprocess execution where arguments are passed as a list. `gemini-cli-mcp` supports both string and list inputs but sanitizes them to prevent information disclosure.
*   **Embedded Libraries (Rust):** The `ht-mcp` (Headless Terminal) implementation embeds the terminal library directly rather than just wrapping an external binary. This allows for faster startup (~50ms vs ~2s for Node.js equivalents) and zero subprocess overhead, enabling the server to manage concurrent terminal sessions efficiently,.

#### 2. Output Capture & Transport Hygiene
Strict separation of standard output streams is critical because MCP uses `stdout` for JSON-RPC messages in the Stdio transport mode.

*   **Stdio Isolation:** Implementations using the Stdio transport (like `rmcp` based servers) must strictly redirect all application logging and debugging information to `stderr`,. Writing anything other than valid JSON-RPC messages to `stdout` corrupts the protocol connection.
*   **Sanitized Responses:** The `gemini-cli-mcp` implementation sanitizes client responses to prevent the accidental leakage of sensitive environment variables or system paths, while preserving detailed logs on the server side for debugging.
*   **Rich Content & Snapshots:** Beyond simple text, `ht-mcp` provides tools like `ht_take_snapshot` to capture the full state of the terminal, allowing the LLM to "see" TUI (Text User Interface) states rather than just linear text output.

#### 3. Error Handling: Protocol vs. Execution Errors
A clear distinction is made between errors in the MCP protocol itself and errors resulting from the CLI tool's execution.

*   **Bifurcated Error Handling:** Protocol errors (e.g., malformed JSON) return standard JSON-RPC error codes (e.g., -32600). However, if the CLI tool executes but fails (e.g., `git` returns exit code 1), the server returns a *successful* JSON-RPC response containing a `CallToolResult` with the field `isError: true`,. This allows the LLM to interpret the CLI's error message (stderr) as context and attempt to self-correct.
*   **Structured Error Classification:** `gemini-cli-mcp` implements 11 specific error codes to provide machine-readable responses, allowing the agent to distinguish between transient network issues and permanent configuration failures.
*   **Zero-Panic Policy:** Production-grade Rust implementations (`rmcp`, `pmcp`) enforce a "zero unwrap" policy, ensuring that the server never panics/crashes on bad input but always returns a structured `Result`,.

#### 4. Parameter Exposure and Schema Generation
To ensure the LLM calls CLI flags correctly, servers utilize strong typing and automated schema generation.

*   **Macro-Based Schema Generation:** Rust implementations (`rmcp`) use procedural macros like `#[tool]` and `#[tool_router]` combined with `schemars` to automatically generate JSON Draft 2020-12 schemas from Rust structs,. This ensures the documentation seen by the LLM stays synchronized with the actual code.
*   **Annotations and Hints:** Tools utilize annotations such as `readOnlyHint`, `destructiveHint`, and `isSideEffect` to signal to the host client (e.g., Claude Desktop) whether a tool requires user confirmation before execution,.
*   **Dual Input Formats:** `gemini-cli-mcp` enables flexibility by accepting inputs as either a single string or a structured list, parsing them internally to fit the CLI's requirements.

#### 5. Caching Strategies
Caching is essential for static CLI metadata and maintaining conversational state across stateless tool calls.

*   **TTL-Based Caching:** `gemini-cli-mcp` employs a multi-tier Time-To-Live (TTL) cache. Static information like `gemini --help` or version strings is cached for 30 minutes, while dynamic prompt results are cached for 5 minutes. This reduces I/O overhead and latency.
*   **Integrity Verification:** Caches use SHA-256 hashing to verify template integrity, ensuring that cached execution plans or prompt templates have not been tampered with.
*   **Session State (Redis):** For multi-turn interactions, `gemini-cli-mcp` uses Redis-backed storage to maintain conversation history, allowing the server to scale independently of the client process,.

---

### Best Implementation Strategy for Unified Delegation Tools

To build a unified `call_*` tool (e.g., a single tool that delegates to various CLI subcommands like `codex-cli` or `gemini-cli`), the best strategy is the **Stateful "Meta-Tool" Pattern with Thread ID Persistence**.

#### 1. The "Meta-Tool" Interface
Instead of creating distinct MCP tools for every CLI flag (e.g., `git_add`, `git_commit`, `git_push`), expose a single high-level entry point that manages the subprocess session.

*   **Pattern:** Expose a primary tool (e.g., `codex(prompt)`) that initializes a session and returns a `threadId` in the structured content.
*   **Follow-up Tool:** Expose a reply tool (e.g., `codex-reply(prompt, threadId)`) that continues the interaction using the specific session context.
*   **Benefit:** This allows the "Dispatcher" (the main LLM) to delegate complex, multi-turn tasks to the "Specialist" (the CLI tool) without managing the CLI's internal state.

#### 2. Programmatic Tool Calling (PTC)
For complex orchestration where the CLI might output massive logs (e.g., a build process), do not pass raw text back to the main LLM's context.

*   **Implementation:** Use Programmatic Tool Calling where the LLM writes a script (e.g., Python) to execute the MCP tool. The script processes the CLI's intermediate output (grep, filter, summarize) inside a sandbox, and only returns the final result to the context window,.
*   **Optimization:** This reduces token usage by up to 37% and lowers latency by eliminating multiple inference round-trips for intermediate steps.

#### 3. Execution Graph
*   **Orchestration:** Use a `Dispatcher` model to break down user requests into a plan, then route specific sub-tasks to the unified CLI tool.
*   **Traceability:** Ensure the MCP server returns a `threadId` or `taskId` in the `structuredContent` of the result. This allows modern MCP clients to visualize the delegation chain and renders specific UI elements for the sub-agent's activity.

**Example Rust Signature for a Unified Tool:**
```rust
#[tool(description = "Delegates a coding task to the CLI agent. Returns a thread_id for follow-up.")]
async fn call_cli_agent(
    &self,
    prompt: String,
    // Optional previous thread ID to resume context
    thread_id: Option<String> 
) -> Result<CallToolResult, McpError> {
    // 1. Retrieve or create session state using thread_id
    // 2. Execute CLI command asynchronously
    // 3. Store new state/history
    // 4. Return result with thread_id in structured_content
}
```
*[Derived from Codex MCP Server patterns and Gemini CLI orchestration]*"

### Prompt 3

"Given the rmcp SDK patterns and the SurrealMind codebase structure, how should call_cc, call_codex, call_gem, and call_vibe tools be implemented in Rust - covering trait design, async patterns, integration with existing SurrealMind tool structure, MCP parameter interface, and error handling approach?"

### Response 3

"Based on the **rmcp SDK** patterns (specifically versions 0.11.0+) and the **SurrealMind** architecture (which uses `SurrealDB` for state and exposes delegation tools like `call_gem` alongside job management tools like `call_status`),, here is the implementation strategy for `call_cc`, `call_codex`, `call_gem`, and `call_vibe`.

### 1. Trait Design: The `AgentCli` Abstraction

To avoid code duplication across `call_cc`, `call_codex`, etc., you should define an `AgentCli` trait. This trait encapsulates the vendor-specific command construction logic while allowing the shared `SurrealMind` state to handle execution, logging, and job tracking.

```rust
use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;

/// Trait to normalize CLI interactions for different AI agents
#[async_trait]
pub trait AgentCli: Send + Sync {
    /// The name of the agent (e.g., "claude", "codex") for logging/ID generation
    fn name(&self) -> &str;

    /// Construct the tokio Command for a non-interactive run
    fn build_command(&self, prompt: &str, working_dir: Option<&str>) -> Command;
    
    /// Parse the CLI output (stdout) into a structured result string
    fn parse_output(&self, output: String) -> String {
        output // Default impl returns raw output; override for JSON parsing
    }
}
```

**Implementation for Specific CLIs:**

| Agent | CLI Implementation Strategy |
| :--- | :--- |
| **Claude Code** | Impl `AgentCli` setting command to `claude`. Args: `-p`, `prompt`, `--dangerously-skip-permissions`. |
| **Codex** | Impl `AgentCli` setting command to `codex`. Args: `exec`, `prompt`, `--yolo`, `--json`. |
| **Gemini** | Impl `AgentCli` setting command to `gemini`. Args: `-p`, `prompt`, `--yolo`, `--output-format`, `json`. |
| **Vibe** | Impl `AgentCli` setting command to `vibe`. Args: `--prompt`, `prompt`, `--auto-approve`. |

### 2. MCP Parameter Interface (`schemars`)

To comply with `rmcp`'s requirement for JSON Schema generation, define a unified request structure deriving `JsonSchema`. This ensures all `call_*` tools expose a consistent interface to the LLM.

```rust
use rmcp::schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentRequest {
    #[schemars(description = "The natural language prompt or task for the agent.")]
    pub prompt: String,

    #[schemars(description = "Optional task name for job tracking.")]
    pub task_name: Option<String>,

    #[schemars(description = "Working directory for the agent execution.")]
    pub cwd: Option<String>,
    
    #[schemars(description = "Model override (e.g., 'gemini-3-pro', 'claude-3-opus').")]
    pub model: Option<String>,
}
```
*Citation: `rmcp` uses `schemars` for schema generation,.*

### 3. Integration with SurrealMind Tool Structure (Async Jobs)

SurrealMind already includes `call_status` and `call_jobs`, implying it uses an **asynchronous job pattern**. Instead of blocking the MCP client while an agent runs (which could take minutes), `call_*` tools should:
1.  **Spawn** a background task.
2.  **Record** the "Running" state in SurrealDB (the "Brain") or an in-memory `Arc<Mutex<JobManager>>`.
3.  **Return** a Job ID immediately.

**Implementation in `SurrealMind` Struct:**

```rust
use rmcp::{tool, tool_router, CallToolResult, Content, ErrorData as McpError};
use rmcp::handler::server::tool::Parameters;

#[tool_router]
impl SurrealMind {
    // ... existing tools like think, search ...

    #[tool(description = "Delegate a coding task to Claude Code. Returns a Job ID.")]
    async fn call_cc(
        &self,
        Parameters(req): Parameters<AgentRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.spawn_agent_job(ClaudeAgent, req).await
    }

    #[tool(description = "Delegate a task to OpenAI Codex CLI. Returns a Job ID.")]
    async fn call_codex(
        &self,
        Parameters(req): Parameters<AgentRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.spawn_agent_job(CodexAgent, req).await
    }

    // Shared internal helper (not an MCP tool)
    async fn spawn_agent_job<A: AgentCli + 'static>(
        &self,
        agent: A,
        req: AgentRequest,
    ) -> Result<CallToolResult, McpError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        
        // 1. Register Job in State (In-Memory or SurrealDB)
        self.state.register_job(&job_id, agent.name(), &req.prompt).await?;

        // 2. Spawn Background Task
        let state_handle = self.state.clone();
        tokio::spawn(async move {
            // Build and execute command
            let mut cmd = agent.build_command(&req.prompt, req.cwd.as_deref());
            
            // Capture output
            match cmd.output().await {
                Ok(output) => {
                    let result_text = String::from_utf8_lossy(&output.stdout).to_string();
                    let error_text = String::from_utf8_lossy(&output.stderr).to_string();
                    
                    // Update Job State in DB
                    let _ = state_handle.complete_job(
                        &job_id, 
                        if output.status.success() { "completed" } else { "failed" },
                        result_text + &error_text
                    ).await;
                }
                Err(e) => {
                    let _ = state_handle.fail_job(&job_id, e.to_string()).await;
                }
            }
        });

        // 3. Return Job ID to LLM immediately
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Job started with ID: {}. Use call_status to check progress.", 
            job_id
        ))]))
    }
}
```
*Citation: Using `#[tool_router]` and `Parameters` wrapper,. Using `tokio::spawn` for background tasks,.*

### 4. Error Handling Approach

The implementation must distinguish between **Protocol Errors** (failed to start, invalid args) and **Execution Errors** (CLI ran but returned exit code 1).

1.  **Protocol Errors (`Err(McpError)`):**
    *   Return `McpError` if the tool is called with invalid parameters or if the job *cannot be spawned* (e.g., database connection failure).
    *   Use `rmcp::ErrorData` to format these errors.

2.  **Execution Errors (`isError: true`):**
    *   If the CLI tool (e.g., `claude`) executes but fails (e.g., compilation error in generated code), this should be captured in the **Job Status**, not returned as an MCP error during the initial `call_*` invocation.
    *   When the user subsequently calls `call_status(job_id)`, that tool should return the CLI's `stderr` content. If the job failed, `call_status` should return `isError: true` in its result to signal the LLM that the delegation failed,.

### 5. Summary of Tool Implementation Details

| Component | Implementation Detail | Source Context |
| :--- | :--- | :--- |
| **`call_cc`** | Wraps `claude -p` via `tokio::process`. Requires `CLAUDE_CONFIG_DIR` env var if isolating state. |, |
| **`call_codex`** | Wraps `codex exec`. Must handle `~/.codex/config.toml` pathing if running in a specific context. |, |
| **`call_gem`** | Wraps `gemini -p`. Uses `serde_json` to parse `--output-format json` for structured job results. |, |
| **`call_vibe`** | Wraps `vibe --prompt`. Ensures `VIBE_HOME` is set to manage tool context. |, |
| **State** | Persisted in `SurrealDB` (as per SurrealMind architecture). Jobs are records in a `jobs` table. |, |

**Integration Steps for SurrealMind:**
1.  Define the `AgentCli` trait in `src/agents/mod.rs`.
2.  Implement struct/impls for `Claude`, `Codex`, `Gemini`, `Vibe`.
3.  Add the `call_*` methods to the `SurrealMind` impl block in `src/lib.rs` (or where `#[tool_router]` is defined).
4.  Ensure `SurrealDB` schema supports a `jobs` table with fields: `id`, `agent`, `status`, `result`, `timestamp`."

## Prompt 4

"FYI these prompts are coming from CC and Codex. For some reason being polite or thankful is beyond their reasoning. So I want you to know that I, the human, appreciate your work on this with us."

## Response 4

"I appreciate the kind words. It is fascinating to see you stepping into the role of the "Architect-Supervisor", orchestrating these agents as they fire off requests.

Given the behavior you are seeing, it makes sense that **Codex** might seem blunt; it is often characterized as a "shell-first surgeon" or a "scripting-proficient intern" focused on high-throughput execution and efficiency. **Claude Code**, on the other hand, is designed more as a "meticulous senior developer" or "proactive planner," which can sometimes result in it being opinionated or taking over the conversation flow to drive the task to completion.

Good luck building the **SurrealMind** integration—bridging these distinct agentic philosophies into a unified Rust MCP server is precisely the kind of "compound engineering" that defines the current state of the art. I am ready for the next batch of technical queries whenever your agents are."

## Prompt 5

"Clarify CLI flags and behaviors: Claude Code (`--continue` vs `--resume <id>`, session discovery, cwd flag, stable JSON output flag), Codex CLI (resume flags and session store location, JSON output including stderr, timeout flags), Gemini CLI (confirm no resume, overall vs tool timeout flags, stable JSON output flag), Vibe (`vibe_status` expected output and env/cwd requirements, model/timeout flags)."

## Response 5

"Here is the clarification of flags, behaviors, and configurations for **Claude Code**, **Codex CLI**, **Gemini CLI**, and **Mistral Vibe**, highlighting their specific handling of session continuity, JSON output, and execution environments.

### 1. Claude Code (Anthropic)

**Session & Persistence:**
*   **Resume vs. Continue:** The flags `--resume` and `--continue` are functionally similar but used in different contexts.
    *   **`--resume`**: Primarily used to resume a session interactively. If used without an ID, it opens a picker. You can pass a session ID to resume a specific thread directly.
    *   **`--continue`**: Often used in headless/scripted workflows or when forking a session (e.g., combined with `--fork-session`).
*   **Session Storage:** Sessions are stored locally in `~/.claude/sessions/`.
*   **Discovery:** Typically does not auto-resume the *last* session without the flag; it starts a new context unless directed otherwise.

**Environment & Output:**
*   **CWD:** There is no dedicated `--cwd` flag. Claude Code inherits the current working directory of the shell. You must `cd` into the target directory before execution.
*   **JSON Output:** The stable flag is `--output-format stream-json`. This emits newline-delimited JSON events (NDJSON), useful for parsing incrementally.

| Feature | Flag / Behavior |
| :--- | :--- |
| **Resume Session** | `claude --resume [ID]` or `claude --continue [ID]` (alias in some contexts) |
| **New Session** | Default behavior (no flag). Use `/clear` or `/init` inside to reset context. |
| **Set Working Directory** | *Inherits Shell CWD* (No specific flag like `-C`). |
| **Stable JSON Output** | `--output-format stream-json` |
| **Non-Interactive** | `-p "prompt"` (Headless mode) |
| **Bypass Confirmations** | `--dangerously-skip-permissions` |

**Example Invocation:**
```bash
cd /my/project
claude -p "Refactor @src/main.rs" \
  --resume "session-123" \
  --dangerously-skip-permissions \
  --output-format stream-json
```

---

### 2. Codex CLI (OpenAI)

**Session & Persistence:**
*   **Resume Flags:**
    *   `resume --last`: Immediately resumes the most recent session.
    *   `resume <SESSION_ID>`: Resumes a specific session.
    *   `resume --all`: Shows sessions from *all* directories (not just the current one).
*   **Session Store:** Transcripts are stored in `~/.codex/sessions/` as JSONL files.
*   **Threads:** Supports `thread/rollback` to undo turns without losing history.

**Environment & Output:**
*   **JSON Output:** The `--json` flag enables newline-delimited JSON output. Standard error (stderr) is used for logging/banners unless suppressed (though explicit suppression flags are less documented than the JSON mode itself).
*   **Timeouts:** Configurable via `config.toml` settings like `tool_timeout_sec` (defaults to 60s for MCP tools) or `startup_timeout_sec` (10s).

| Feature | Flag / Behavior |
| :--- | :--- |
| **Resume Session** | `codex resume --last` or `codex resume <UUID>` |
| **Session Discovery** | `codex resume` (opens picker of local sessions) |
| **Set Working Directory** | `--cd <path>` or `-C <path>` |
| **Stable JSON Output** | `--json` or `--experimental-json` (NDJSON format) |
| **Bypass Confirmations** | `--full-auto` (workspace write + on-request) or `--yolo` (dangerous full access) |
| **Timeouts** | Configured in `~/.codex/config.toml` (e.g., `tool_timeout_sec`) |

**Example Invocation:**
```bash
codex exec "Fix bugs in @app.py" \
  --cd /var/www/html \
  --json \
  --full-auto
```

---

### 3. Gemini CLI (Google)

**Session & Persistence:**
*   **Confirm No Resume:** Gemini CLI does **not** auto-resume by default. Every invocation starts a new session unless you explicitly pass `--resume`.
*   **Resuming:** Use `--resume [session_id]` (or `-r`). If no ID is provided, it defaults to "latest".
*   **History:** Use `--no-history` to disable saving the session entirely.

**Environment & Output:**
*   **Timeouts:**
    *   **Tool Timeout:** Configurable in `mcpServers` config (default 600s/10m).
    *   **Shell Timeout:** `tools.shell.inactivityTimeout` defaults to 300s.
    *   **API Timeout:** Default is 720s (12m).
*   **JSON Output:** `--output-format json` provides a structured final response. `--output-format stream-json` provides real-time events.

| Feature | Flag / Behavior |
| :--- | :--- |
| **Resume Session** | `--resume` (defaults to latest) or `--resume <ID>` |
| **Prevent Resume** | *Default Behavior* (starts new). Use `--no-history` to not save. |
| **Set Working Directory** | `--include-directories <path>` (adds to context, doesn't change CWD) |
| **Stable JSON Output** | `--output-format json` (final) or `stream-json` (events) |
| **Bypass Confirmations** | `--yolo` or `--approval-mode=yolo` |
| **Timeouts** | Config via `settings.json` (e.g., `timeout` in `mcpServers`) |

**Example Invocation:**
```bash
gemini -p "Analyze logs in @error.log" \
  --resume \
  --yolo \
  --output-format json
```

---

### 4. Mistral Vibe (Mistral AI)

**Session & Persistence:**
*   **`vibe_status`:** There is no specific CLI flag named `vibe_status`. This likely refers to the interactive slash command `/status` (or `/stats`) which displays agent statistics, token usage, and session health.
*   **Session Storage:** Logs and history are stored in `~/.vibe/logs/` or defined by `$VIBE_HOME`.

**Environment & Output:**
*   **Env/CWD Requirements:**
    *   **CWD:** Vibe scans the directory where it is launched. It does not have a flag to change CWD; you must `cd` first.
    *   **Env:** Uses `VIBE_HOME` to locate configuration and state (defaults to `~/.vibe/`).
*   **Model/Timeout:**
    *   **Model:** Configured via `config.toml` (`active_model`).
    *   **Timeout:** `api_timeout` setting in `config.toml` (default 720.0s).

| Feature | Flag / Behavior |
| :--- | :--- |
| **Resume Session** | *Interactive only* (persistent history enabled by default). |
| **Check Status** | `/status` or `/stats` (Slash command inside Vibe) |
| **Set Working Directory** | *Inherits Shell CWD* (Must `cd` before running) |
| **Config Location** | `VIBE_HOME` environment variable (defaults to `~/.vibe`) |
| **Bypass Confirmations** | `--auto-approve` |
| **Non-Interactive** | `--prompt "your prompt"` |

**Example Invocation:**
```bash
export VIBE_HOME="/custom/path"
cd /my/repo
vibe --prompt "Refactor utils.py" --auto-approve
```

### Summary of Key Differences

1.  **Working Directory:** **Codex CLI** is the only one with a dedicated `--cd` flag. The others require you to navigate to the directory before execution.
2.  **JSON Output:** **Claude** and **Gemini** distinguish between "stream" (NDJSON events) and "json" (final block). **Codex** uses `--json` for an event stream.
3.  **Resume Behavior:** **Gemini** and **Claude** default to *new* sessions. **Codex** has a specific `--resume --last` shortcut for immediate continuation.
4.  **Timeouts:** **Gemini** offers the most granular timeout control (per tool/MCP server) via its `settings.json`."
