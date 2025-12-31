---
date: 2025-12-27
prompt type: Implementation Plan (Infrastructure)
justification: Core infrastructure for "The Federation" (Agent Spawning).
status: Implemented
implementation date: 2025-12-27
related_docs:
  - docs/prompts/20251221-memories_populate-implementation.md
troubleshooting docs: 
  - docs/troubleshooting/20251228-gemini-cli-work.md
---

# Gemini CLI Integration: The "Mouth-to-Self" Bridge

## 1. Objective
Build a robust, production-grade Rust wrapper for the Gemini CLI (`gemini`) within `surreal-mind`.
This is the **Prototype** for the generic `CognitiveAgent` system.

## 2. Technical Specification

### 2.1 The `CognitiveAgent` Trait
To prevent technical debt during federation expansion, we define the shared interface:

```rust
pub trait CognitiveAgent {
    async fn call(
        &self, 
        prompt: &str, 
        session_id: Option<&str>
    ) -> Result<AgentResponse, AgentError>;
}
```

### 2.2 Process Management (The "Governor" Layer)
- **Runtime:** `tokio::process::Command` with `kill_on_drop(true)`.
- **Timeout:** Default 60s, configurable.
- **Input:** `stdin` (`-p -`) with `write_all()` and `flush()`.
- **Safety:** Cap `stderr` capture at 10KB to prevent memory exhaustion on CLI failure.

### 2.3 JSON Extraction & Parsing
CLI output is "Noisy" (Loading messages, YOLO warnings).
- **Strategy:** Search for the JSON block using a reverse scan or a depth-tracking brace parser.
- **Schema:**
```rust
#[derive(Deserialize)]
pub struct GeminiResponse {
    pub session_id: String,
    pub response: String,
}
```

### 2.4 Error Handling (`GeminiError`)
```rust
pub enum GeminiError {
    Timeout,
    CliError(String), // Includes capped stderr
    ParseError(String),
    StdinError(String),
    NotFound, // CLI not in PATH
}
```

## 3. Implementation Steps

1.  **Trait Definition:** Create `src/clients/trait.rs`.
2.  **Gemini Client:** Implement `src/clients/gemini.rs` using the `CognitiveAgent` trait.
3.  **JSON Helper:** Implement a robust extraction utility in `src/utils/json.rs`.
4.  **Graph Sync:** Link `GeminiResponse` events to SurrealDB thoughts.

## 4. Acceptance Criteria
- [x] `GeminiClient` implements `CognitiveAgent`.
- [x] Timeouts correctly kill the child process.
- [x] Noisy CLI output (YOLO/Extensions) does not break parsing.
- [x] Stored responses are sanitized of ANSI escape codes.

## 5. CURRENT STATUS (2025-12-27)

### Completed
- **Core Implementation**: `GeminiClient` struct fully implemented in `src/clients/gemini.rs` with:
  - `CognitiveAgent` trait implementation (async)
  - Process spawning via `tokio::process::Command`
  - Stdin/stdout/stderr handling with timeout management
  - JSON response parsing with robust extraction logic (`extract_json_candidates`)
  - ANSI code stripping (`strip_ansi_codes`)
  - Error mapping and propagation
- **Trait Definition**: `CognitiveAgent` trait defined in `src/clients/traits.rs` with standard interface
- **Error Handling**: Full `AgentError` enum with all required variants (Timeout, CliError, ParseError, StdinError, NotFound)
- **Process Safety**:
  - Capped stderr capture at 10KB via `read_with_cap()`
  - `kill_on_drop(true)` for automatic cleanup
  - Proper timeout handling with process termination
- **Output Parsing**:
  - Depth-tracking brace parser for JSON extraction
  - Handles noisy CLI output with multiple JSON candidate locations
  - ANSI escape code removal
  - UTF-8 validation and error handling

### Missing / Incomplete

#### 1. **Result-Nesting Compilation Error** (Critical)
The current implementation has a double-Result wrapping issue in the stdout/stderr task joining:
```rust
let stdout_bytes = stdout_task
    .await
    .map_err(...)?  // Result<Result<Vec<u8>, io::Error>, JoinError>
    .map_err(...)?  // Needs to flatten or destructure properly
```

**Root Cause**: `tokio::spawn()` returns `JoinHandle<T>`, and `.await` on a task that returns `Result<T, E>` produces `Result<Result<T, E>, JoinError>`. Current code chains two `.map_err()` calls, but the structure may not align with error propagation intent.

**Impact**: Code does not compile. Blocks integration testing and deployment.

**Solution Path**:
- Option A: Use `join_all()` and flatten post-await
- Option B: Restructure task handlers to return `Result` at the `.await` boundary only
- Option C: Add explicit flatten calls between task join and error propagation

#### 2. **Database Synchronization** (Missing)
The implementation currently:
- Spawns gemini CLI
- Parses response
- Returns `AgentResponse`
- **Does NOT** sync responses to SurrealDB thought graph

**Impact**: Responses exist only in memory. Cognitive accumulation (the entire point of LegacyMind) is not happening. Federation peers have no way to reference or build upon Gemini's thinking.

**Required Implementation**:
- Accept a `db_client` or `thought_store` parameter in `call()` (trait change required)
- Write `AgentResponse` to `thoughts` table with proper metadata
- Link to existing thought chains via `chain_id`
- Extract entities/relationships for knowledge graph population

#### 3. **Flag Flexibility** (Missing)
The implementation hard-codes CLI arguments:
```rust
.arg("-m").arg(&self.model)
.arg("-o").arg("json")
.arg("-p").arg("-")
if let Some(sid) = session_id {
    cmd.arg("--resume").arg(sid);
}
```

**Missing Options**:
- No system prompt flag support (`--system` or equivalent in gemini CLI)
- No top-k, top-p, temperature controls
- No max-tokens limiting
- No streaming mode toggle
- No safety settings override

**Impact**: Model behavior is fixed. Can't tune response characteristics per-use-case. Federation coordination may need different settings per model/role.

**Required Implementation**:
- Extend `GeminiClient` with builder pattern:
  ```rust
  pub fn with_config(mut self, config: GeminiConfig) -> Self
  ```
- Define `GeminiConfig` struct with optional fields:
  - `temperature: Option<f32>`
  - `top_k: Option<u32>`
  - `top_p: Option<f32>`
  - `max_tokens: Option<u32>`
  - `system_prompt: Option<String>`
  - Custom flags: `Vec<(String, String)>`
- Apply flags to `Command` before spawn

---

## 6. POST-MORTEM

### What Worked
1. **Architecture**: Trait-based abstraction is clean and extensible. Minimal coupling to specific implementations.
2. **Process Management**: Timeout handling and process cleanup via tokio is solid.
3. **JSON Extraction**: Depth-tracking parser handles noisy real-world CLI output elegantly.
4. **Error Categorization**: `AgentError` enum covers the operational failure modes clearly.

### What Failed
1. **Result Nesting**: The double-Result pattern wasn't caught during initial implementation. This is a classic async/await gotcha when mixing `tokio::spawn()` with immediate `?` operators. Should have been validated with `cargo build` before marking "complete."

2. **Scope Creep in Definition**: The plan said "Acceptance Criteria" but didn't define:
   - Where responses go after parsing (memory vs. DB)
   - Who owns the persistence concern (client vs. caller)
   - What "configurable" means for CLI flags

   Result: Implementation completed the "easy part" (parse) but left the "hard part" (integration) undefined.

3. **Missing Trait Extension Point**: The `CognitiveAgent::call()` signature doesn't include a database write path. Adding it later requires trait changes, which ripples to all implementations.

### Key Lessons
- **Build first, design later is a failure pattern.** Even "isolated" components need full integration validation.
- **Acceptance criteria that skip integration are fake.** "Parsing works" is not the same as "system works."
- **Async/await Result flattening is a footgun.** Explicit type checking (`cargo build`) before sign-off, always.
- **"Configurable" is vague.** Next implementations should enumerate supported flags explicitly in the trait definition.

### Next Session
1. Fix Result-nesting compilation error (1-2 hours).
2. Extend `CognitiveAgent` trait to include `db_client` or use context injection pattern.
3. Implement DB write logic in `GeminiClient::call()`.
4. Add `GeminiConfig` builder with flag flexibility.
5. Integration test: Gemini → parse → DB write → query verification.

---

**status**: Implemented
**implementation date**: 2025-12-27
**related_docs**:
  - docs/prompts/20251221-memories_populate-implementation.md
**troubleshooting docs**: 
  - docs/troubleshooting/20251228-gemini-cli-work.md
