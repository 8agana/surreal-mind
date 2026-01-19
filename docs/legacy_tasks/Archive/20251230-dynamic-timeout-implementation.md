# Implementation Plan: Dynamic Timeout Handling for Gemini Delegation

**Date**: 2025-12-30  
**Status**: DRAFT  
**Objective**: Replace the hardcoded 60-second execution cap with a dynamic, user-configurable timeout to support complex reasoning tasks (Gemini Pro).

## 1. Rationale
The current implementation in `src/clients/gemini.rs` imposes a hard 60,000ms limit on the total duration of the `gemini` CLI subprocess. While sufficient for simple queries, complex architectural reviews and multi-turn chains (especially using Pro models) frequently exceed this threshold, leading to premature process termination and "missing exchange_id" errors (since the middleware never completes the persistence turn).

## 2. Technical Requirements

### 2.1 Schema Updates (`src/schemas.rs`)
- Add an optional `timeout_ms` parameter to:
    - `delegate_gemini_schema`
    - `legacymind_think_schema`
- Type: `integer`, minimum: `1000` (1s), maximum: `1800000` (30m).

### 2.2 Trait Modification (`src/clients/traits.rs`)
- Update `CognitiveAgent::call` signature to accept an optional timeout:
  ```rust
  async fn call(
      &self,
      prompt: &str,
      session_id: Option<&str>,
      timeout_ms: Option<u64>, // Added
  ) -> Result<AgentResponse, AgentError>;
  ```

### 2.3 Client Implementation (`src/clients/gemini.rs`)
- Update `GeminiClient::call` to honor the `timeout_ms` override.
- Increase the global `DEFAULT_TIMEOUT_MS` from `60_000` to `300_000` (5 minutes).
- Ensure the `tokio::time::timeout` wrapper uses the resolved duration.

### 2.4 Middleware Implementation (`src/clients/persisted.rs`)
- Pass the `timeout_ms` through to the inner agent.

### 2.5 Tool Handler (`src/tools/delegate_gemini.rs`)
- Extract `timeout_ms` from `CallToolRequestParam`.
- Pass it to the `agent.call()` method.

## 3. Implementation Steps
1. **Refactor Traits**: Update `AgentResponse` and `CognitiveAgent`.
2. **Update Client**: Modify `GeminiClient` to use dynamic duration and increase default to 300s.
3. **Update Schemas**: Expose `timeout_ms` to the MCP interface.
4. **Wire Handlers**: Update `delegate_gemini.rs` and `thinking.rs` to propagate the parameter.
5. **Verify**: Run a 70-second sleep test via `delegate_gemini` to confirm the process is no longer killed at 60s.

## 4. Success Criteria
- [ ] `delegate_gemini` accepts `timeout_ms` argument.
- [ ] Large prompts using Pro models successfully return after 60s.
- [ ] Persistence middleware correctly logs the turn after an extended execution.
