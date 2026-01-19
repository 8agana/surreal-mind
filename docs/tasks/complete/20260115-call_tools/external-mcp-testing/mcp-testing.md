# MCP Delegation Tools Evaluation

**Date:** 2026-01-15 (Updated: 2026-01-16)
**Purpose:** Evaluate existing MCP delegation patterns to inform `call_cc` tool design
**Status:** Complete

---

## 1. Overview

Testing four MCP delegation implementations to understand interface patterns, parameter design, and operational characteristics before implementing `call_cc`.

### Tools Under Evaluation

| Tool | Server | Purpose |
|------|--------|---------|
| `call_gem` | surreal-mind | Gemini CLI delegation |
| `mcp__claude-code-mcp__*` | claude-code-mcp | Claude Code delegation |
| `mcp__gemini-cli__*` | gemini-cli | Gemini CLI (MCP wrapper) |
| `mcp__codex-cli__*` | codex-cli | OpenAI Codex delegation |
| `mcp__vibe-cli__*` | vibe-cli | Mistral Vibe delegation |

---

## 2. Evaluation Criteria

### 2.1 Interface Design
- Tool naming conventions
- Parameter structure (flat vs nested)
- Required vs optional parameters
- Default value patterns

### 2.2 Parameter Patterns
- How prompts are passed
- Model selection mechanism
- Timeout/resource configuration
- Working directory handling

### 2.3 Error Handling
- Error response format
- Timeout behavior
- Partial failure handling
- Recovery mechanisms

### 2.4 Output Format
- Response structure
- Streaming vs blocking
- Metadata inclusion
- Status reporting

### 2.5 Context Passing
- How context is provided to delegated agent
- File/code context mechanisms
- Conversation history handling

### 2.6 Session Support
- Fire-and-forget vs tracked jobs
- Job status querying
- Cancellation support
- Session persistence

---

## 3. Test Results

### 3.1 claude-code-mcp

| Criterion | Observation | Notes |
|-----------|-------------|-------|
| Interface Design | Single unified tool `claude_code`, flat parameter structure | Clean, minimal API surface |
| Parameter Patterns | `prompt` (required), `workFolder` (required for file ops) | No model selection, timeout, or advanced config |
| Error Handling | Natural language error messages embedded in response | No structured error objects |
| Output Format | Plain text responses, conversational tone | No JSON/metadata, purely content-focused |
| Context Passing | Via `workFolder` + `@file` syntax in prompts | Agent reads files itself, no explicit context param |
| Session Support | None exposed via MCP | Stateless from MCP perspective |

**Raw Test Output:**
```
Test 1: Simple question
> Prompt: "Explain the Fibonacci sequence in one sentence."
> Response: "The Fibonacci sequence is a series of numbers where each number is the sum of the two preceding ones, starting from 0 and 1 (0, 1, 1, 2, 3, 5, 8, 13, 21, ...)."

Test 2: File reading (workFolder: /tmp)
> Prompt: "Read the test.txt file and tell me what it says."
> Response: "The file contains a single line: Hello from Codex"

Test 3: File creation (workFolder: /tmp)
> Prompt: "Create a file called claude_test.txt with the text 'Claude Code was here'"
> Response: "Done. Created `claude_test.txt` in `/private/tmp/` with the requested text."
> Verification: File created successfully

Key Finding: Claude Code wraps complexity - handles tool dispatch internally, returns human-readable results only.
```

---

### 3.2 gemini-cli (MCP)

| Criterion | Observation | Notes |
|-----------|-------------|-------|
| Interface Design | Multiple tools: `ping`, `Help`, `ask-gemini`, `brainstorm`, `timeout-test`, `fetch-chunk` | Most granular API of the four tested |
| Parameter Patterns | `prompt` (required), `model`, `sandbox`, `changeMode`, chunking support | `changeMode` attempts structured edits but fragile |
| Error Handling | Mixed - some structured (changeMode), some plain text | changeMode returned garbage when tool-calling attempted |
| Output Format | Prefixed with "Gemini response:" for plain queries, JSON for chunked | Inconsistent formatting between modes |
| Context Passing | `@filename` syntax in prompts, agent reads files | Similar to claude-code |
| Session Support | None at MCP level | CLI has sessions but not exposed via MCP |

**Raw Test Output:**
```
Test 1: Connectivity
> Tool: ping
> Response: "Pong!"

Test 2: Help
> Tool: Help
> Response: Full CLI usage with all flags, commands, options

Test 3: Simple query
> Prompt: "Explain the Fibonacci sequence in one sentence."
> Response: "Gemini response: The Fibonacci sequence is a mathematical progression where each number is the sum of the two preceding ones, typically starting with 0 and 1."

Test 4: File context (model: gemini-2.5-flash)
> Prompt: "@/tmp/test.txt What does this file contain?"
> Response: Returned workspace setup info instead of file contents (context misfire)

Test 5: Brainstorm mode
> Prompt: "Ways to test MCP delegation tools"
> Parameters: ideaCount=5, methodology=divergent
> Response: Returned 5 structured ideas with feasibility/impact/innovation scores
> Format: Markdown with idea titles, descriptions, assessments

Test 6: changeMode (FAILED)
> Prompt: "Write a bash one-liner to count files in /tmp"
> changeMode: true
> Response: "No edits found in Gemini's response. Please ensure Gemini uses the OLD/NEW format."
> Actual: Repeated attempts to use non-existent tools (execute_shell_command, run_shell_command)
> Finding: changeMode expects specific output format, breaks when agent tries to use tools

Test 7: Sandbox mode
> Prompt: "What is 2 + 2?"
> sandbox: true
> Response: "Gemini response: 4."
> Note: Sandbox flag accepted but unclear what it does for simple queries

Key Findings:
- Most feature-rich MCP wrapper (brainstorm, changeMode, chunking)
- Brainstorm mode produces high-quality structured output
- changeMode is brittle - fails when underlying agent uses tools
- File context via @ syntax works but can misfire
```

---

### 3.3 call_gem (surreal-mind native)

| Criterion | Observation | Notes |
|-----------|-------------|-------|
| Interface Design | `call_gem`, `call_status`, `call_jobs`, `call_cancel` | Async job model with tracking |
| Parameter Patterns | `prompt` (required), `model`, `cwd`, `timeout_ms`, `fire_and_forget`, `expose_stream` | Most comprehensive config surface |
| Error Handling | Structured JSON responses with error field | Jobs can fail, status queryable |
| Output Format | JSON responses, async status updates | No direct output - must query agent_exchanges table |
| Context Passing | Via `cwd` parameter + agent reads files | No explicit context passing mechanism |
| Session Support | Full session tracking in database | Sessions persist with exchange history |

**Raw Test Output:**
```
Test 1: Basic delegation
> Tool: call_gem
> Prompt: "Explain the Fibonacci sequence in one sentence."
> Response: {"status":"queued","job_id":"c742e435-6dd6-4f35-857a-fc51337ade44","message":"Job queued for background execution. Use agent_job_status to check progress."}

Test 2: Status check (after 3s)
> Tool: call_status
> job_id: "c742e435-6dd6-4f35-857a-fc51337ade44"
> Response: {"job_id":"...","status":"completed","created_at":"2026-01-17T02:50:45.586067Z","started_at":"2026-01-17T02:50:45.712044Z","completed_at":"2026-01-17T02:50:53.952439Z","duration_ms":8238,"session_id":"89fc02da-3f76-4834-ba1f-6ded8f6c82ec","exchange_id":"agent_exchanges:e10vsucct2o1qnimafqb",...}
> Note: Returns metadata but NOT actual response content

Test 3: Job listing
> Tool: call_jobs
> Parameters: limit=5, status_filter="completed"
> Response: {"jobs":[...5 jobs with timestamps, durations, tool names...],"total":5}

Test 4: Output retrieval attempt
> Tool: search (searching for exchange_id)
> Result: Returned KG memories, not the actual Gemini response
> Finding: Agent output stored in agent_exchanges table but not exposed via MCP tools

Key Findings:
- Only tool tested with true async job model
- Status tracking comprehensive (created/started/completed timestamps, duration)
- Critical gap: No MCP tool to retrieve actual agent output
- Session persistence works (session_id, exchange_id tracked)
- Fire-and-forget mode available but untested
- Known bugs (from Jan 15 Codex analysis): dummy cancel handle, timeout_ms unused, brittle error parsing
```

---

### 3.4 codex-cli

| Criterion | Observation | Notes |
|-----------|-------------|-------|
| Interface Design | `codex`, `review`, `ping`, `help`, `listSessions` | Clear command/query split |
| Parameter Patterns | `prompt` (required), `sandbox`, `fullAuto`, `workingDirectory`, `sessionId`, `model`, `resetSession`, `reasoningEffort` | Most mature parameter design |
| Error Handling | Natural language errors in response content | No structured error format but clear messaging |
| Output Format | Plain text, conversational, inline | Synchronous blocking calls |
| Context Passing | Via `workingDirectory` (-C flag), agent has full filesystem access within sandbox | Most transparent context model |
| Session Support | Full session persistence via `sessionId`, `listSessions` shows metadata | Sessions survive across MCP calls |

**Raw Test Output:**
```
Test 1: Connectivity
> Tool: ping
> message: "Test connectivity"
> Response: "Test connectivity"

Test 2: Help
> Tool: help
> Response: Full CLI documentation with all subcommands, flags, options
> Note: Includes experimental features (MCP server mode, app-server, cloud)

Test 3: Simple query (sync)
> Prompt: "What is the Fibonacci sequence? Provide a simple explanation."
> sandbox: read-only
> Response: "The Fibonacci sequence is a list of numbers where each number is the sum of the two before it. It usually starts 0, 1, so the sequence goes 0, 1, 1, 2, 3, 5, 8, 13, 21, and so on. Simple rule: next = previous + the one before that."
> Duration: ~2s
> Note: Blocking synchronous response

Test 4: File creation with fullAuto
> Prompt: "Create a simple test.txt file with 'Hello from Codex' in the current working directory."
> workingDirectory: /tmp
> fullAuto: true
> Response: "Created ` /tmp/test.txt ` with the requested content. If you want me to verify or modify it, say the word."
> Verification: File created successfully with exact content

Test 5: Code generation with session
> Prompt: "Write a Python script that calculates prime numbers up to 100"
> workingDirectory: /tmp
> fullAuto: true
> sessionId: "test-session-1"
> Response: "Script written to `/tmp/primes_up_to_100.py`. Run it: `python3 /tmp/primes_up_to_100.py` Want it to print one per line or as comma-separated text instead of a list?"
> Verification: Script works, outputs [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97]

Test 6: Session continuation
> Prompt: "Perfect, thanks."
> sessionId: "test-session-1"
> Response: "Anytime. Try not to miss your primes."
> Note: Maintains conversational context

Test 7: Session listing
> Tool: listSessions
> Response: [{"id":"test-session-1","createdAt":"2026-01-17T02:51:21.649Z","lastAccessedAt":"2026-01-17T02:52:18.259Z","turnCount":2}]

Key Findings:
- Most polished UX - natural conversation, helpful suggestions
- Sandbox modes actually work (read-only, workspace-write, danger-full-access)
- fullAuto mode eliminates approval prompts (equivalent to -a on-request --sandbox workspace-write)
- Sessions persist across calls with full context
- Synchronous model - blocks until response ready
- Strong defaults - safe by default (approval prompts), opt-in to automation
- Best documentation of the four (comprehensive help output)
```

---

### 3.5 vibe-cli

| Criterion | Observation | Notes |
|-----------|-------------|-------|
| Interface Design | Not tested | Not available/configured in current environment |
| Parameter Patterns | Not tested | - |
| Error Handling | Not tested | - |
| Output Format | Not tested | - |
| Context Passing | Not tested | - |
| Session Support | Not tested | - |

**Raw Test Output:**
```
Vibe MCP not tested - no vibe-cli tools available in current MCP configuration.
Note: Based on Session 3 testing, Vibe MCP exists but is a rules unification tool,
not a delegation interface. Not relevant for call_cc design.
```

---

## 4. Best Practices Identified

### 4.1 Parameter Design

**Required vs Optional Balance:**
- **Codex** gets this right: `prompt` only required field, sensible defaults for everything else
- **Claude Code** minimalist: `prompt` + `workFolder`, nothing else exposed
- **Gemini-CLI** middle ground: `prompt` required, optional model/sandbox/changeMode
- **call_gem** maximal: exposes all knobs (timeout, fire_and_forget, expose_stream, model override)

**Best practice:** Start minimal (prompt only), add optional parameters as needed. Don't expose internals unless users actually need control.

**Working Directory Handling:**
- **Codex** (`workingDirectory`): Clear, explicit, optional
- **Claude Code** (`workFolder`): Required for file ops, clear error if missing
- **call_gem** (`cwd`): Optional, defaults to inherited
- **Gemini-CLI**: No explicit param, uses session working directory

**Best practice:** Optional `working_directory` parameter with clear semantics. Agent operates from that path, can access files via relative paths.

**Model Selection:**
- **Codex**: Optional `model` param with clear choices (o3, gpt-5.2-codex, gpt-4o, etc.)
- **Gemini-CLI**: Optional `model` param (gemini-2.5-flash, gemini-2.5-pro)
- **call_gem**: Optional `model_override`
- **Claude Code**: No model selection (hardcoded to default)

**Best practice:** Optional `model` parameter. Default to best general-purpose model, allow override for speed/capability tradeoffs.

### 4.2 Error Handling

**Three approaches observed:**

1. **Natural language errors** (Codex, Claude Code): Errors embedded in response text, no special structure
   - Pro: User-friendly, readable
   - Con: Harder to programmatically detect failures

2. **Structured JSON errors** (call_gem): Separate `error` field in response object
   - Pro: Programmatic error detection easy
   - Con: Requires parsing, more complex

3. **Mixed** (Gemini-CLI): Some structured (changeMode failures), some plain text
   - Con: Inconsistent, worst of both worlds

**Best practice:** Return structured errors at MCP level (`{"status": "error", "error": "message", "code": "ERROR_TYPE"}`), include human-readable explanation. Allows both programmatic handling and user debugging.

### 4.3 Output Format

**Sync vs Async:**
- **Synchronous** (Codex, Claude Code, Gemini-CLI): Block until response ready, return content directly
  - Pro: Simple mental model, immediate results
  - Con: Can timeout on long operations, wastes context window while waiting

- **Asynchronous** (call_gem): Return job ID, poll for completion
  - Pro: Non-blocking, supports long-running tasks
  - Con: More complex (requires status polling), **critical gap: no way to retrieve output**

**Best practice for call_cc:**
- Default to synchronous for simplicity
- Support async mode via optional `mode: "async"` parameter
- If async: return job_id immediately, provide `call_cc_result(job_id)` tool to retrieve output

**Response Format:**
- **Plain text** (Codex, Claude Code): Natural conversation, markdown formatting
- **Prefixed text** (Gemini-CLI): "Gemini response: ..." (awkward)
- **JSON** (call_gem): Structured but incomplete (missing actual output)

**Best practice:** Return plain text/markdown for content. Use JSON wrapper only for metadata (job_id, status, timing).

### 4.4 Context Management

**File Context Mechanisms:**
1. **Working directory** (all tools): Agent operates from specified path, reads files directly
2. **@ syntax** (Gemini-CLI, Claude Code): `@filename` in prompt includes file content
3. **No explicit context passing** (all tools): Agent uses its own Read tool

**Observations:**
- All tools rely on agent's built-in file reading capability
- No explicit "context" parameter with pre-loaded file contents
- Working directory scope is sufficient for most use cases

**Best practice:**
- Support `working_directory` parameter (optional, defaults to calling agent's cwd)
- Document that agent can read files in that directory
- No need for explicit context injection - let delegated agent use its tools

### 4.5 Session/Job Tracking

**Session Persistence Models:**

1. **No persistence** (Claude Code via MCP): Each call is isolated
   - Pro: Simple, stateless
   - Con: Can't have multi-turn conversations

2. **Session ID** (Codex): Optional `sessionId` param, full conversation context preserved
   - Pro: Natural multi-turn interaction
   - Con: Sessions accumulate, need cleanup mechanism

3. **Database tracking** (call_gem): Every job stored with full metadata
   - Pro: Complete audit trail, can query historical jobs
   - Con: Storage overhead, requires DB

**Best practice for call_cc:**
- Support optional `session_id` parameter
- If provided: continue existing conversation
- If omitted: one-shot stateless call
- Store session metadata in DB (created_at, last_accessed, turn_count) for debugging
- Provide `call_cc_sessions()` tool to list/inspect active sessions

**Job Metadata Worth Tracking:**
- `job_id` (if async mode)
- `created_at`, `started_at`, `completed_at`
- `duration_ms`
- `status` (queued, running, completed, failed)
- `session_id` (if multi-turn)
- `model_used`
- `working_directory`
- Prompt hash (for deduplication)

**Status Querying:**
- **call_gem** provides `call_status(job_id)` and `call_jobs(limit, filter)`
- **Codex** provides `listSessions()` for session metadata

**Best practice:** Provide both:
- `call_cc_status(job_id)` - get single job status + output
- `call_cc_jobs(limit, status_filter)` - list recent jobs
- `call_cc_sessions()` - list active sessions

---

## 5. Synthesis: Unified Pattern for `call_cc`

### 5.1 Recommended Interface

```rust
// Parameter structure based on evaluation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CallCcParams {
    /// The prompt/task to send to Claude Code
    pub prompt: String,

    /// Optional working directory (defaults to caller's cwd)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,

    /// Optional model selection (haiku, sonnet, opus)
    /// Defaults to haiku for cost efficiency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Optional session ID for multi-turn conversations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Execution mode: "sync" (default) or "async"
    #[serde(default = "default_mode")]
    pub mode: String,

    /// Optional timeout in milliseconds (default: 120000)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    /// Optional brainstorm mode (adds brainstorming context to prompt)
    #[serde(default)]
    pub brainstorm: bool,
}

fn default_mode() -> String {
    "sync".to_string()
}
```

**Supporting Tools:**
```rust
// For async mode
pub struct CallCcStatusParams {
    pub job_id: String,
}

pub struct CallCcJobsParams {
    pub limit: Option<u32>,
    pub status_filter: Option<String>, // queued, running, completed, failed
}

pub struct CallCcSessionsParams {
    pub limit: Option<u32>,
}
```

### 5.2 Key Design Decisions

| Decision | Chosen Approach | Rationale |
|----------|-----------------|-----------|
| Sync vs Async | Default sync, optional async via `mode` parameter | Codex/Claude Code prove sync is simpler for most use cases; async adds complexity but needed for long tasks |
| Model Selection | Optional, default to Haiku | Cost efficiency - escalate to Sonnet/Opus only when needed |
| Session Support | Optional `session_id` like Codex | Enables multi-turn conversations when needed, stateless by default |
| Working Directory | Optional, defaults to caller's cwd | Codex pattern works well, clear semantics |
| Error Handling | Structured JSON with human-readable messages | Best of both worlds - programmatic + debuggable |
| Output Retrieval | `call_cc_status(job_id)` returns full output | Fixes call_gem's critical gap |
| Context Passing | Agent uses its own tools, no explicit injection | All tested tools work this way successfully |
| Brainstorm Mode | Boolean flag on main tool | Per Sam's direction from Session 3, not separate tool |

### 5.3 Differentiation from Existing Tools

**What `call_cc` should do differently from call_gem:**

1. **Return actual output in sync mode** - Don't require database queries to get results
2. **Fix the async gap** - If async, provide `call_cc_status(job_id)` that returns output, not just metadata
3. **Simplify defaults** - Sync by default, async opt-in (call_gem is async-only)
4. **Model tiering** - Default to Haiku, explicit escalation to Sonnet/Opus (call_gem uses Pro/Flash unclear)
5. **Working session tracking** - Actually implement cancel (call_gem has dummy handle)
6. **Use timeout properly** - call_gem computes `_tool_timeout` but never uses it

**What `call_cc` should adopt from Codex:**

1. **Session management** - Optional session_id, listSessions metadata
2. **Conversational responses** - Plain text/markdown, natural language
3. **Clear defaults** - Safe by default, explicit opt-in to capabilities
4. **Comprehensive help** - Tool description should be clear and complete

**What `call_cc` should adopt from Claude Code:**

1. **Minimal required params** - Just prompt, everything else optional
2. **Natural errors** - Human-readable failure messages
3. **Working directory semantics** - Clear path-based scoping

### 5.4 Implementation Notes

**Execution Strategy:**

**Sync Mode:**
```rust
// Spawn claude code CLI with timeout
// Block until completion or timeout
// Return stdout directly as response
// Log to database for audit trail
```

**Async Mode:**
```rust
// Generate job_id
// Store job in agent_jobs table (status: queued)
// Spawn tokio task to execute
// Return {"job_id": "...", "status": "queued"} immediately
// Task updates status: queued -> running -> completed/failed
// Store full output in job record
```

**Model Selection Mapping:**
```rust
match model {
    "haiku" | "h" => vec!["claude", "code", "-m", "claude-sonnet-4-5"],  // Haiku tier
    "sonnet" | "s" => vec!["claude", "code", "-m", "claude-sonnet-4-5"],
    "opus" | "o" => vec!["claude", "code", "-m", "claude-opus-4-5"],
    _ => vec!["claude", "code"],  // Default
}
```

**Session Handling:**
```rust
// If session_id provided:
//   1. Check agent_sessions table for existing session
//   2. Load conversation file path from session record
//   3. Add --continue flag to CLI invocation
//   4. Increment turn_count, update last_accessed
// If no session_id:
//   Create new temp session file
//   Store in agent_sessions if multi-turn expected
```

**Database Schema Extensions:**
```sql
-- Extend agent_jobs table
ALTER TABLE agent_jobs ADD COLUMN output TEXT;
ALTER TABLE agent_jobs ADD COLUMN working_directory TEXT;
ALTER TABLE agent_jobs ADD COLUMN session_id TEXT;

-- Create agent_sessions table
CREATE TABLE agent_sessions (
    session_id TEXT PRIMARY KEY,
    agent_type TEXT NOT NULL,  -- 'claude_code'
    created_at DATETIME NOT NULL,
    last_accessed DATETIME NOT NULL,
    turn_count INTEGER DEFAULT 0,
    conversation_file TEXT,  -- Path to session file
    working_directory TEXT,
    metadata OBJECT
);
```

**Error Handling Pattern:**
```rust
match result {
    Ok(output) => json!({
        "status": "completed",
        "output": output,
        "duration_ms": duration,
        "model_used": model
    }),
    Err(e) if e.is_timeout() => json!({
        "status": "error",
        "error": "Task exceeded timeout",
        "code": "TIMEOUT",
        "timeout_ms": timeout
    }),
    Err(e) => json!({
        "status": "error",
        "error": e.to_string(),
        "code": "EXECUTION_FAILED"
    })
}
```

**Brainstorm Mode Implementation:**
```rust
// If brainstorm: true
let enhanced_prompt = format!(
    "BRAINSTORMING MODE: Generate creative, diverse ideas for the following:\n\n{}",
    params.prompt
);
// Use enhanced prompt instead of raw prompt
// Rest of execution unchanged
```

---

## Appendix: Tool Schemas

### claude-code-mcp schema
```json
{
  "name": "mcp__claude-code-mcp__claude_code",
  "description": "Claude Code Agent: Your versatile multi-modal assistant...",
  "parameters": {
    "type": "object",
    "properties": {
      "prompt": {
        "description": "The detailed natural language prompt for Claude to execute.",
        "type": "string"
      },
      "workFolder": {
        "description": "Mandatory when using file operations or referencing any file. The working directory for the Claude CLI execution. Must be an absolute path.",
        "type": "string"
      }
    },
    "required": ["prompt"]
  }
}
```

### gemini-cli schema (ask-gemini tool)
```json
{
  "name": "mcp__gemini-cli__ask-gemini",
  "description": "model selection [-m], sandbox [-s], and changeMode:boolean for providing edits",
  "parameters": {
    "type": "object",
    "properties": {
      "prompt": {
        "description": "Analysis request. Use @ syntax to include files...",
        "type": "string",
        "minLength": 1
      },
      "model": {
        "description": "Optional model to use (e.g., 'gemini-2.5-flash'). If not specified, uses the default model (gemini-2.5-pro).",
        "type": "string"
      },
      "sandbox": {
        "description": "Use sandbox mode (-s flag) to safely test code changes...",
        "type": "boolean",
        "default": false
      },
      "changeMode": {
        "description": "Enable structured change mode - formats prompts to prevent tool errors and returns structured edit suggestions...",
        "type": "boolean",
        "default": false
      },
      "chunkIndex": {
        "description": "Which chunk to return (1-based)",
        "type": ["number", "string"]
      },
      "chunkCacheKey": {
        "description": "Optional cache key for continuation",
        "type": "string"
      }
    },
    "required": ["prompt"]
  }
}
```

### gemini-cli schema (brainstorm tool)
```json
{
  "name": "mcp__gemini-cli__brainstorm",
  "description": "Generate novel ideas with dynamic context gathering...",
  "parameters": {
    "type": "object",
    "properties": {
      "prompt": {
        "description": "Primary brainstorming challenge or question to explore",
        "type": "string",
        "minLength": 1
      },
      "methodology": {
        "description": "Brainstorming framework: 'divergent', 'convergent', 'scamper', 'design-thinking', 'lateral', 'auto'",
        "type": "string",
        "enum": ["divergent", "convergent", "scamper", "design-thinking", "lateral", "auto"],
        "default": "auto"
      },
      "ideaCount": {
        "description": "Target number of ideas to generate (default: 10-15)",
        "type": "integer",
        "default": 12,
        "exclusiveMinimum": 0
      },
      "includeAnalysis": {
        "description": "Include feasibility, impact, and implementation analysis for generated ideas",
        "type": "boolean",
        "default": true
      },
      "domain": {
        "description": "Domain context for specialized brainstorming",
        "type": "string"
      },
      "existingContext": {
        "description": "Background information, previous attempts, or current state to build upon",
        "type": "string"
      },
      "constraints": {
        "description": "Known limitations, requirements, or boundaries",
        "type": "string"
      },
      "model": {
        "description": "Optional model to use",
        "type": "string"
      }
    },
    "required": ["prompt"]
  }
}
```

### codex-cli schema (codex tool)
```json
{
  "name": "mcp__codex-cli__codex",
  "description": "Execute Codex CLI in non-interactive mode for AI assistance",
  "parameters": {
    "type": "object",
    "properties": {
      "prompt": {
        "description": "The coding task, question, or analysis request",
        "type": "string"
      },
      "workingDirectory": {
        "description": "Working directory for the agent to use as its root (passed via -C flag)",
        "type": "string"
      },
      "model": {
        "description": "Specify which model to use (defaults to gpt-5.2-codex). Options: gpt-5.2-codex, gpt-5.1-codex, gpt-5.1-codex-max, gpt-5-codex, gpt-4o, gpt-4, o3, o4-mini",
        "type": "string"
      },
      "sandbox": {
        "description": "Sandbox policy for shell command execution. read-only: no writes allowed, workspace-write: writes only in workspace, danger-full-access: full system access",
        "type": "string",
        "enum": ["read-only", "workspace-write", "danger-full-access"]
      },
      "fullAuto": {
        "description": "Enable full-auto mode: sandboxed automatic execution without approval prompts",
        "type": "boolean"
      },
      "sessionId": {
        "description": "Optional session ID for conversational context",
        "type": "string"
      },
      "resetSession": {
        "description": "Reset the session history before processing this request",
        "type": "boolean"
      },
      "reasoningEffort": {
        "description": "Control reasoning depth (minimal < low < medium < high)",
        "type": "string",
        "enum": ["minimal", "low", "medium", "high"]
      }
    },
    "required": ["prompt"]
  }
}
```

### call_gem schema (surreal-mind native)
```json
{
  "name": "mcp__surreal-mind__call_gem",
  "description": "Delegate a task to Gemini CLI with full context and tracking",
  "parameters": {
    "type": "object",
    "properties": {
      "prompt": {
        "type": "string"
      },
      "model": {
        "type": "string"
      },
      "cwd": {
        "type": "string"
      },
      "timeout_ms": {
        "type": "number"
      },
      "tool_timeout_ms": {
        "type": "number"
      },
      "fire_and_forget": {
        "type": "boolean",
        "default": false
      },
      "expose_stream": {
        "type": "boolean"
      },
      "task_name": {
        "type": "string"
      }
    },
    "required": ["prompt"]
  }
}
```

### vibe-cli schema
```
N/A - Not a delegation tool, rules unification system
```
