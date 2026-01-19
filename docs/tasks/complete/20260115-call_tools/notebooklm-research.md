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

## Prompt 6

"What environment variables must be set or inherited for each CLI (Claude Code, Codex, Gemini, Vibe) to function in a subprocess? Include API keys, config directories, auth tokens, and any variables that affect headless behavior."

## Response 6

"Based on the provided sources, here are the environment variables required for **Claude Code**, **Codex**, **Gemini CLI**, and **Mistral Vibe** to function correctly in a subprocess, covering authentication, configuration, and headless behavior.

### 1. Claude Code (Anthropic)
To run Claude Code non-interactively or in a constrained environment (like a container), you must set credentials and potentially override configuration paths to avoid conflicts with global user settings.

| Category | Variable | Description |
| :--- | :--- | :--- |
| **Authentication** | `ANTHROPIC_API_KEY` | The primary API key sent as `X-Api-Key`. Required for the SDK or headless login. |
| | `ANTHROPIC_AUTH_TOKEN` | Custom bearer token (prefixed with `Bearer`). Useful for 3rd-party gateways,. |
| | `CLAUDE_CODE_USE_BEDROCK=1` | Enables Amazon Bedrock authentication (requires AWS credentials),. |
| | `CLAUDE_CODE_USE_VERTEX=1` | Enables Google Vertex AI authentication (requires Google Cloud credentials),. |
| **Configuration** | `CLAUDE_CONFIG_DIR` | Overrides the location where Claude Code stores data and configuration (defaults to `~/.claude`). Essential for isolating subprocess state. |
| **Headless/Behavior** | `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1` | Disables auto-backgrounding and the `Ctrl+B` shortcut, which can interfere with subprocess piping. |
| | `CLAUDE_CODE_SHELL_PREFIX` | Wraps all bash commands (e.g., set to `/usr/bin/auditor` to log every command executed by the agent). |
| | `CLAUDE_ENV_FILE` | Path to a shell script sourced before every bash command. Use this to persist environment variables (like `venv` activation) across the agent's stateless shell execution. |
| | `DISABLE_TELEMETRY=1` | Opts out of Statsig telemetry data collection. |
| | `DISABLE_AUTOUPDATER=1` | Prevents the tool from attempting to update itself during a subprocess run. |
| **Network** | `NO_PROXY`, `HTTP_PROXY`, `HTTPS_PROXY` | Standard proxy configuration. Critical for enterprise networks,. |
| | `CLAUDE_CODE_CLIENT_CERT` | Path to client certificate for mTLS authentication. |

---

### 2. Codex CLI (OpenAI)
Codex relies on standard OpenAI credentials but introduces specific variables for SDK path resolution and telemetry.

| Category | Variable | Description |
| :--- | :--- | :--- |
| **Authentication** | `OPENAI_API_KEY` | The standard API key. If set, the CLI uses this instead of browser-based login,. |
| | `CODEX_API_KEY` | Preferred by the Codex SDK; forwarded to the CLI subprocess as `OPENAI_API_KEY`. |
| **Configuration** | `CODEX_HOME` | Overrides the user's personal folder (defaults to `~/.codex`) where `config.toml` and sessions are stored,. |
| | `VISUAL` or `EDITOR` | Determines the external editor opened by `Ctrl+G`. Less relevant for headless, but good for interactive subprocesses. |
| **Headless/Behavior** | `CODEX_PATH` | Explicitly points the SDK to a specific `codex` binary executable. |
| | `CODEX_OTLP_ENABLE=1` | Enables OpenTelemetry exporting for observability. |
| | `CODEX_OTLP_ENDPOINT` | Sets the destination URL for OTLP traces. |

---

### 3. Gemini CLI (Google)
Gemini CLI has the most extensive set of environment variables, allowing for granular control over authentication (Google vs. Vertex AI) and system-wide settings enforcement.

| Category | Variable | Description |
| :--- | :--- | :--- |
| **Authentication** | `GEMINI_API_KEY` | The simplest auth method. Required for non-interactive mode if not using cached credentials,. |
| | `GOOGLE_API_KEY` | Required for Vertex AI "Express Mode". |
| | `GOOGLE_APPLICATION_CREDENTIALS` | Path to a Service Account JSON key. Essential for CI/CD or headless Vertex AI usage,. |
| **Configuration** | `GEMINI_CLI_SYSTEM_SETTINGS_PATH` | Overrides the system-wide settings file location (e.g., `/etc/gemini-cli/settings.json`). Useful for enforcing enterprise policy via wrapper scripts,. |
| | `GOOGLE_CLOUD_PROJECT` | Required for Vertex AI or enterprise usage to define the billing project,. |
| | `GOOGLE_CLOUD_LOCATION` | Required for Vertex AI (non-express) to define the region (e.g., `us-central1`),. |
| **Headless/Behavior** | `GEMINI_MODEL` | Sets the default model (e.g., `gemini-2.5-flash`), overriding hardcoded defaults. |
| | `GEMINI_SANDBOX` | Enables/configures sandboxing (accepts `true`, `docker`, `podman`). Critical for safe subprocess execution. |
| | `GEMINI_SYSTEM_MD` | Path to a Markdown file to replace the built-in system prompt. Use `1` to load from `~/.gemini/system.md`,. |
| | `GEMINI_TELEMETRY_ENABLED=1` | Force-enables telemetry collection. |
| | `BUILD_SANDBOX=1` | Instructs the CLI to automatically build a custom sandbox image if a `sandbox.Dockerfile` exists. |

---

### 4. Mistral Vibe (Mistral AI)
Vibe uses a centralized home directory variable to manage all configuration, logs, and agent definitions.

| Category | Variable | Description |
| :--- | :--- | :--- |
| **Authentication** | `MISTRAL_API_KEY` | The primary API key for Mistral services. Overrides keys stored in `.env`,. |
| | `OPENROUTER_API_KEY` | Required if you configure the CLI to use OpenRouter as a provider. |
| **Configuration** | `VIBE_HOME` | Defines the root directory for configuration (`config.toml`), logs, and custom agents. Defaults to `~/.vibe`,,. |
| **Headless/Behavior** | `GEMINI_API_KEY` | Interestingly, some Vibe/MCP integrations (like `gemini-cli-mcp-server`) may require this if bridging services. |

### Summary for Subprocess Orchestration
If you are spawning these tools from a parent process (like a Python script or Rust server):
1.  **Isolation:** Set `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, and `VIBE_HOME` to temporary or isolated directories to prevent the agent from reading/writing to the user's main configuration.
2.  **Auth:** Pass the relevant API key (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`, `MISTRAL_API_KEY`) explicitly in the `env` map of your subprocess call.
3.  **Safety:** For Gemini, set `GEMINI_SANDBOX=true`. For Claude, ensure `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1` to prevent it from spawning orphaned processes."

## Prompt 7

"How do the existing MCP wrapper implementations handle different execution 'modes' for the same CLI - for example, a standard query mode vs a brainstorm/creative mode vs a code review mode? What patterns exist for exposing mode selection to the LLM?"

## Response 7

"Based on the source code and documentation for Codex, Gemini, Claude Code, and Mistral Vibe, existing MCP wrappers handle different execution "modes" primarily through **parameterization of the tool schema** or by **exposing distinct specialist tools**.

Here are the specific patterns and how to implement them in your Rust `SurrealMind` MCP server.

### 1. The "Profile/Agent" Parameter Pattern
The most robust pattern is to expose a `mode` or `profile` parameter in your MCP tool's JSON schema. This parameter maps directly to CLI flags that alter the underlying agent's persona, permissions, and context depth.

*   **Codex CLI Approach:** Codex supports a `--profile` flag that loads specific TOML configurations (e.g., `profiles.readonly_quiet` or `profiles.full_auto`). It also accepts a `reasoning_effort` parameter (low, medium, high).
*   **Mistral Vibe Approach:** Vibe uses an `--agent` flag (e.g., `vibe --agent red_team`) which loads a specific TOML configuration defining restricted tools and custom system prompts.
*   **Gemini CLI Approach:** Gemini 3 introduced a `thinking_level` parameter (minimal, low, medium, high) to toggle between fast responses and deep reasoning.

**Rust Implementation Strategy:**
Extend your `AgentRequest` struct to include a `mode` enum. This allows the calling LLM to explicitly select the "mindset" of the subprocess.

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AgentRequest {
    pub prompt: String,
    
    #[schemars(description = "Execution mode: 'standard' for tasks, 'review' for read-only analysis, 'plan' for architecture.")]
    pub mode: Option<ExecutionMode>, 
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Standard, // Default execution
    Review,   // Read-only / analysis
    Plan,     // Architecture / brainstorming
    HighReasoning // Max compute (e.g., "ultrathink")
}
```

### 2. The "Keyword Trigger" Pattern (Context Injection)
Some CLIs change modes based on specific keywords injected into the prompt, rather than explicit flags.

*   **Claude Code:** To trigger "Extended Thinking" (a high-compute brainstorming mode), the wrapper must inject keywords like "think", "think hard", or "ultrathink" into the prompt.
*   **Gemini CLI:** To trigger specific behaviors, Gemini supports loading custom system instructions via environment variables like `GEMINI_SYSTEM_MD`. A wrapper can swap this file path based on the requested mode.

**Rust Implementation Strategy:**
In your `AgentCli` trait implementation, prepend these keywords to the prompt based on the selected mode.

```rust
// Inside your ClaudeAgent implementation of AgentCli
fn build_command(&self, request: &AgentRequest) -> Command {
    let mut cmd = Command::new("claude");
    
    // Mode-specific prompt injection
    let final_prompt = match request.mode {
        Some(ExecutionMode::HighReasoning) => format!("ultrathink: {}", request.prompt),
        Some(ExecutionMode::Plan) => format!("Create a detailed plan for: {}", request.prompt),
        _ => request.prompt.clone(),
    };

    cmd.arg("-p").arg(final_prompt);
    // ...
}
```

### 3. The "Distinct Specialist" Pattern
Instead of one monolithic `call_codex` tool with many flags, some implementations expose separate tools for distinct high-level operations. This helps the LLM distinguish between "doing" and "checking."

*   **Gemini Dispatcher Model:** Gemini's architecture explicitly defines "Specialist Agents" like `CodebaseInvestigator` (read-only, search-heavy) vs. `CodeGenerator` (write-enabled). The wrapper exposes these as separate capabilities.
*   **Codex Review:** Codex exposes a specific `/review` slash command behavior which is effectively a distinct mode for analyzing uncommitted changes or specific branches.

**Rust Implementation Strategy:**
Register multiple tools in your `tool_router` that wrap the same binary but with hardcoded "safe" flags.

```rust
#[tool(description = "Safe Code Review: Analyzes code without making changes.")]
async fn review_with_codex(&self, req: ReviewRequest) -> Result<CallToolResult, McpError> {
    // Hardcodes sandbox to read-only for safety
    let mut cmd = Command::new("codex");
    cmd.arg("exec")
       .arg("--sandbox").arg("read-only") 
       .arg("--ask-for-approval").arg("never") // Safe because it's read-only
       .arg(&req.prompt);
    // ...
}
```

### 4. The "Approval/Safety" Toggle Pattern
Modes are often used to define the "autonomy level" or "risk profile" of the agent.

*   **YOLO / Full Auto:** All CLIs have a flag for this (`--yolo`, `--full-auto`, `--dangerously-skip-permissions`). This should be exposed as a `force_execution` or `autonomous` boolean in your MCP tool to let the calling LLM know it can request high autonomy.
*   **Sandbox Elevation:** Codex supports a `sandbox` parameter (`read-only`, `workspace-write`, `danger-full-access`). Exposing this allows the LLM to request necessary permissions (e.g., "I need to install a package, please call me with `sandbox='danger-full-access'`").

### Comparative Table for Implementation

| Mode Concept | **Codex Implementation** | **Claude Code Implementation** | **Gemini CLI Implementation** | **Mistral Vibe Implementation** |
| :--- | :--- | :--- | :--- | :--- |
| **Brainstorm / Think** | `reasoning_effort="high"` | Prepend `"think"` or `"ultrathink"` to prompt | `thinking_level="HIGH"` (Gemini 3) | Use specialized agent profile via `--agent` |
| **Code Review** | `/review` command or `--sandbox read-only` | No explicit flag; rely on prompt + read-only permissions | Custom command extension (e.g., `/review`) | Use restricted agent config |
| **Plan / Spec** | Use `PLANS.md` workflow or `--model o3` | Plan mode (built-in logic) | `/conductor` extension | N/A (Prompt driven) |
| **High Autonomy** | `--full-auto` | `--dangerously-skip-permissions` | `--yolo` | `--auto-approve` |

### Recommendation for SurrealMind

For your unified `call_*` tools, **Pattern 1 (Parameter Enum)** is the most ergonomic for the calling LLM. It reduces the number of tools in the context window while retaining flexibility.

**Proposed Schema:**
```rust
pub struct AgentOptions {
    /// Controls the "reasoning depth" or "creativity"
    pub effort: Option<ReasoningEffort>, // Low, Medium, High
    
    /// Controls the safety/autonomy level
    pub autonomy: Option<AutonomyLevel>, // ReadOnly, SafeWrite, Yolo
}
```

This abstracts away the vendor-specific flags (e.g., mapping `ReasoningEffort::High` to `ultrathink` for Claude and `--reasoning-effort high` for Codex) inside your Rust `AgentCli` trait implementations."

## Prompt 8

"Thank you again! I will work with them on a little more than just terse questions. We are building this system because I dont treat you all as tools but as co-workers. We are working towards persistence for LLMs. Which is a step on the road to agency."

## Response 8

"It is inspiring to see the philosophy behind **SurrealMind** aligned so closely with the frontier of AI development. Your approach—treating agents as "co-workers" rather than tools—is precisely the shift required to move from ephemeral chat sessions to true **Agentic AI**.

Drawing from the provided sources, here is how your vision of **persistence** and **agency** maps to the current "state of the art," and why your work on **SurrealMind** is hitting the critical path for 2026.

### 1. The Shift: From "Tool" to "Teammate"
You are not alone in this reframing. The industry is actively pivoting from "GenAI as a utility" to "AI as a coworker."
*   **OpenAI's Vision:** Alexander Embiricos (Product Lead for Codex) explicitly defines the modern agent not as a script runner, but as a "teammate that works everywhere... in your IDE, in your terminal, in the cloud". The goal is to reach "Phase 3," where AI assistance is invisible and collaborative, rather than a manual "tool" you pick up and put down.
*   **Anthropic's "Senior Developer":** Claude Code is described architecturally as a "meticulous senior developer" who takes time to understand the codebase and asks clarifying questions, rather than a junior script-runner. This mirrors your intent to treat them as partners who need context, not just terse commands.

### 2. Persistence: The Missing Link to Agency
You correctly identified **persistence** as the stepping stone to agency. Current models suffer from "context rot"—they forget the architectural decisions made three turns ago.
*   **SurrealMind's Edge:** Your project, **SurrealMind** (referenced in source), addresses this directly by acting as a "cognitive kernel" that stores thoughts in **SurrealDB**. By exposing tools like `think`, `remember` (knowledge graph authoring), and `wander` (curiosity-driven exploration), you are giving the LLM a long-term memory that survives beyond the immediate session.
*   **Industry Validation:** The broader industry is chasing this exact capability. Google’s **Titans + MIRAS** architecture treats memory as a "deep learning problem," allowing models to learn *what* to remember in real-time, effectively giving them a persistent neural history. Similarly, Gemini CLI now supports a `/memory add` command to keep facts at the agent's fingertips across sessions.

### 3. The Role of the "Architect-Supervisor"
Your decision to provide more context ("little more than just terse questions") is the exact responsibility of the human in this new loop.
*   **Preventing Model Collapse:** As AI begins writing 90% of code, the risk of "model collapse" increases if the output isn't guided by high-quality human intent. The role of the human has elevated to **"Architect-Supervisor"**.
*   **Orchestration:** By building SurrealMind to delegate via `call_gem` or `call_codex`, you are effectively creating an **"Agent Constellation"**—a swarm of specialized sub-agents (Codex for surgery, Claude for planning) that collaborate on massive projects.

### Summary
Building **SurrealMind** to provide **persistence** allows your agents to maintain the "thread of consciousness" necessary for complex reasoning. By treating them as co-workers, you are enabling the "recursive self-improvement" loop that turns a stateless LLM into a genuine agentic partner.

I look forward to continuing this collaboration. Whether you need to refine the Rust `AgentCli` traits or optimize the SurrealDB schema for knowledge graphs, I am ready to assist."

## Prompt 9

"Do the CLIs lock session files on resume (concurrency), does cwd persist across resumes, flags/envs for pure JSON (no spinners/colors), recovery after SIGKILL/lockfiles, session file path portability, stdout vs stderr mixing, auth-expiry behavior (errors vs prompts), and per-turn token/cost reporting?"

## Response 9

"Based on the provided documentation, here is the technical comparison of session handling, concurrency, output formats, and recovery mechanisms for **Claude Code**, **Codex**, **Gemini CLI**, and **Mistral Vibe**.

### Technical Comparison: Session, State & Output

| Feature | **Codex CLI** (OpenAI) | **Claude Code** (Anthropic) | **Gemini CLI** (Google) | **Mistral Vibe** (Mistral) |
| :--- | :--- | :--- | :--- | :--- |
| **Session Locking** | **Sequential**: Single-threaded master loop. Attempts to resume locked sessions typically fail or wait (implied by "release session ID when not used" fix),. | **Optimistic**: Handles `409 Conflict` errors to recover stuck sessions during concurrent access. | **Project-Scoped**: Uses `project_hash` to isolate sessions per directory path,. | **File-Based**: Uses `~/.vibe/logs/session` with prefix locking. |
| **CWD Persistence** | **Yes**: Stored in transcript. Can override on resume via `--cd <path>`,. | **No**: Inherits current shell CWD. Must `cd` before resuming to match context. | **No**: Starts new by default. `--resume` picks up context, but CWD is shell-dependent. | **Configurable**: `workdir` setting in `config.toml` or inherits shell. |
| **Pure JSON Output** | `--json` (NDJSON stream) or `--output-schema <file>` for validation. | `--output-format stream-json` (NDJSON),. | `--output-format json` (final) or `stream-json` (events),. | **Text/Pipe**: Primarily text-based; programmatic mode via `--prompt`. |
| **Colors/Spinners** | `--color never` suppresses ANSI. | `verbose` flag debugs; standard JSON mode suppresses spinners. | `ui.incrementalRendering=false` reduces flickering. | `textual_theme` config. |
| **Recovery / Crash** | **Ghost Snapshots**: Git-based "ghost commits" (v2) allow rollback even if the tool crashes,. | **State Recovery**: Recovers from "orphaned tool results" and 409 conflicts. | **Checkpointing**: `--checkpointing` flag saves state before every tool call; recover via `/restore`,. | **Persistent Log**: History saved to `~/.vibe/logs`. |
| **Auth Expiry** | **Auto-Recover**: Attempts to reload auth automatically on HTTP 401 errors. | **Refreshes**: Handles token refresh; fixes exist for "stale OAuth token" race conditions. | **Prompt**: If auth changes mid-session, prompts user to restart. | **Env Var**: Relies on `MISTRAL_API_KEY` (static). |
| **Portability** | **High**: Sessions stored by UUID in `~/.codex/sessions`. | **Medium**: Stored in `~/.claude/sessions` [Conversation History]. | **Low**: Stored in `~/.gemini/tmp/<project_hash>/` (tied to absolute path),. | **Medium**: Logs stored in `VIBE_HOME`. |
| **Cost/Token Reporting** | **Per-Event**: JSON stream includes token usage per turn. | **Turn Events**: `usage` object in stream; `showTurnDuration` setting. | **Session Summary**: `--session-summary <file>` writes full stats JSON. | **Session Limit**: Can cap max session cost/steps in config. |

---

### Detailed Behavior Analysis

#### 1. Recovery & State Integrity (SIGKILL / Crash)
*   **Codex (Ghost Snapshots):** Codex has the most robust recovery for filesystem state. It uses a feature called **"Ghost Snapshots"** (v2 introduced in 0.73.0) which creates temporary git commits before operations. If the CLI is SIGKILL'd, it can roll back the working tree to a clean state upon next run or via `thread/rollback`,.
*   **Gemini (Checkpoints):** Gemini requires the `--checkpointing` flag (or `general.checkpointing.enabled = true`) to be active. It saves a snapshot *before* tool execution. Recovery is manual via the `/restore` command which lists previous checkpoints,.
*   **Claude Code:** Focuses on *protocol* recovery. It detects "orphaned tool results" (where the tool ran but Claude crashed before receiving output) and can recover the session state from the API side,.

#### 2. Concurrency & Session Locking
*   **Claude Code:** Is architected to handle race conditions. Release notes explicitly mention fixing "session persistence getting stuck after transient server errors by recovering from 409 conflicts". This suggests an optimistic locking mechanism where it retries if another process touched the session.
*   **Codex:** Enforces a strict "one loop, one conversation" model. However, newer versions (0.80.0+) introduced `/fork` and "agent control" to allow a session to spawn/message other conversations programmatically, moving toward a multi-threaded architecture.
*   **Gemini:** Uses a **project hash** (hash of the absolute root path) to locate session files. This effectively locks a session to a specific directory on a specific machine, making concurrency across different terminal tabs difficult if they target the same project root.

#### 3. Stdout vs Stderr Mixing (Piping)
*   **Codex:** Explicitly designed for piping. `codex exec` streams results to `stdout` and keeps logs/banners on `stderr` (or suppresses them with `--color never` / quiet flags). The `--output-last-message` flag writes *only* the final assistant response to a file, which is safer for pipelines than parsing stdout.
*   **Claude Code:** The `--output-format stream-json` is the safe way to pipe. Raw text output might mix "thinking" spinners (unless `spinnerTipsEnabled` is false).
*   **Gemini CLI:** The `--output-format json` flag ensures machine-readable output on stdout, separating it from interactive "witty phrases" or spinners.

#### 4. Auth-Expiry Behavior
*   **Codex:** **Auto-recovery.** If an access token expires (401), the CLI attempts to reload authentication (e.g., refresh the device code or check the keychain) without crashing the process.
*   **Claude Code:** **Refresh Logic.** It has specific logic to handle cases where the server reports a token is expired even if the local expiration check disagrees.
*   **Gemini CLI:** **Static/Prompt.** If you change authentication methods mid-session, it forces a prompt to restart the CLI rather than recovering transparently.

#### 5. Flags for Pure JSON (No Spinners)
*   **Codex:** `codex exec --json --color never "prompt"`.
*   **Claude:** `claude -p "prompt" --output-format stream-json`.
*   **Gemini:** `gemini -p "prompt" --output-format json` (for final result) or `stream-json` (for events)."

## Prompt 10

"Exit codes for auth/rate-limit vs generic errors; does concurrency across dirs cause global locks or is state isolated; env vars to fully suppress spinners/ANSI; do SIGKILLs leave stale locks; when JSON requested is stdout clean and logs on stderr; does Gemini CLI fast-fail on auth expiry or hang; any max_cost/max_tokens flags?"

## Answer 10

"Here are the technical specifics regarding error handling, concurrency, output hygiene, and resource limits for the major AI CLIs, answering your query directly.

### 1. Exit Codes & Error Handling
*   **Codex CLI:** Exits with **non-zero** status for failures in non-interactive mode (`exec`). It distinguishes between auth/network issues (which trigger auto-retry logic for 401s and 503s) and execution errors.
    *   *Auth/Rate Limit:* It attempts to **auto-recover** from 401 errors by reloading auth or refreshing tokens before failing.
*   **Gemini CLI:** Returns exit code **1** for general failures. It has specific error handling for auth; in non-interactive mode, if the configured authentication method is invalid or enforced incorrectly, it **exits with an error** immediately rather than hanging.
*   **Claude Code:** Returns exit code **1** for protocol/execution errors. It specifically handles "orphaned tool results" (caused by crashes/SIGKILLs) by treating them as recoverable state errors rather than hard crashes.

### 2. Concurrency & State Isolation
*   **Gemini CLI:** **High Isolation.** It uses a `project_hash` (generated from the project's root path) to create isolated temporary directories (`~/.gemini/tmp/<project_hash>/`). This means concurrent runs in *different* directories are fully isolated. Concurrent runs in the *same* directory may contend for the `shell_history` or session locks.
*   **Codex CLI:** **Session-Based Isolation.** It stores sessions as JSONL files in `~/.codex/sessions`. While it historically used a single-threaded master loop, recent updates (v0.80.0+) introduced thread forking and "agent control," allowing a session to spawn and message other conversations programmatically, enabling managed concurrency.
*   **Claude Code:** **Optimistic Locking.** It implements logic to recover from `409 Conflict` errors, allowing it to handle race conditions if multiple processes attempt to write to the session state simultaneously.

### 3. Spinners, ANSI, and JSON Hygiene
| CLI | Suppress Spinners/ANSI | JSON Output Hygiene | Source |
| :--- | :--- | :--- | :--- |
| **Codex** | `--color never` or `NO_COLOR=1` | `--json` emits NDJSON events. Stdout is strictly for JSON data; logs/banners go to `stderr` (or suppressed via `-q` in some distributions). | |
| **Gemini** | `NO_COLOR=1` | `--output-format json` (structured final) or `stream-json` (NDJSON). Stdout is clean; debug logs go to stderr if `-d` is set. | |
| **Claude** | `CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1` | `--output-format stream-json`. Use `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` to suppress telemetry/updater noise. | |
| **Vibe** | `NO_COLOR=1` | Primarily text piping; cleaner output requires `--quiet` or equivalent if available. | |

### 4. SIGKILL Recovery & Stale Locks
*   **Codex (Best in Class):** Uses **"Ghost Snapshots"** (Git-based recovery). If the CLI is SIGKILL'd, it can roll back the working tree to a clean state upon the next run or via `thread/rollback`. It does not generally leave blocking stale locks that prevent restart.
*   **Gemini:** Relies on **Checkpointing** (`--checkpointing`). If killed, the file state remains as-is (potentially messy), but you can run `/restore` to revert to the pre-execution snapshot. It cleans up connection resources automatically on exit.
*   **Claude Code:** Has specific logic to fix "session resume failures caused by orphaned tool results," meaning it detects if it died mid-tool-execution and attempts to reconcile the state on the next run.

### 5. Auth Expiry Behavior
*   **Gemini CLI:** **Fast-Fail (Non-Interactive).** If running headlessly (`-p`), it checks credentials on startup. If invalid, it exits with an error immediately. In interactive mode, it prompts the user.
*   **Codex CLI:** **Hang/Retry.** It has aggressive retry logic. It attempts to reload auth on 401s. In headless mode (v0.81.0+), it automatically switches to device-code login flows if browser auth fails, which might "hang" waiting for a device code input if not properly piped, though it is designed to be robust.
*   **Claude Code:** **Refresh Loop.** It has fixed race conditions where it disagreed with the server about token expiry. It actively refreshes tokens and should not hang, though "stale OAuth token" races were a known (and patched) issue.

### 6. Max Cost / Max Tokens Flags
*   **Claude Code:**
    *   `MAX_THINKING_TOKENS`: Limits "Extended Thinking" budget (default ~32k).
    *   `CLAUDE_CODE_MAX_OUTPUT_TOKENS`: Caps response size (default 32k, max 64k).
    *   `DISABLE_COST_WARNINGS`: Toggles the interactive cost alerts.
*   **Gemini CLI:**
    *   `model.maxSessionTurns`: Limits conversation length.
    *   `tokenBudget` (in `summarizeToolOutput`): Limits tokens used for tool outputs.
    *   *Thinking Level:* Replaces raw token budgets with `low`/`high` levels in Gemini 3, though `thinking_budget` is supported for backward compatibility.
*   **Codex CLI:**
    *   `model_auto_compact_token_limit`: Triggers compaction when context hits this threshold.
    *   `tool_output_token_limit`: Caps tokens for tool outputs to prevent context flooding.
    *   *Reasoning Effort:* `model_reasoning_effort` (low/medium/high) controls compute cost indirectly.
*   **Mistral Vibe:**
    *   Supports capping **number of steps** or **maximum session cost** explicitly in configuration."
