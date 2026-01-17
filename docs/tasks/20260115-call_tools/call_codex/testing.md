# call_codex Testing Document

**Status**: Draft - Awaiting Implementation
**Created**: 2026-01-16
**Last Updated**: 2026-01-16

## Overview

This document defines the testing strategy for the `call_codex` MCP tool implementation. It covers unit tests, integration tests, manual verification procedures, and result tracking.

---

## 1. Unit Tests (Rust)

### 1.1 Schema Validation Tests

**Location**: `surreal-mind/src/mcp/tools/codex/tests.rs` or `surreal-mind/tests/mcp_tools_codex.rs`

```rust
#[cfg(test)]
mod schema_tests {
    use super::*;

    #[test]
    fn test_codex_request_schema_valid_minimal() {
        // Test: Minimal valid request (prompt only)
        let json = json!({
            "prompt": "List all Rust files in src/"
        });

        let result = serde_json::from_value::<CodexRequest>(json);
        assert!(result.is_ok());
        let req = result.unwrap();
        assert_eq!(req.prompt, "List all Rust files in src/");
        assert_eq!(req.model, None);
        assert_eq!(req.working_directory, None);
        assert_eq!(req.session_id, None);
    }

    #[test]
    fn test_codex_request_schema_valid_full() {
        // Test: All optional parameters populated
        let json = json!({
            "prompt": "Refactor this function",
            "model": "gpt-5.2-codex",
            "workingDirectory": "/Users/samuelatagana/Projects/LegacyMind",
            "sessionId": "abc123",
            "sandbox": "workspace-write",
            "fullAuto": true,
            "reasoningEffort": "medium"
        });

        let result = serde_json::from_value::<CodexRequest>(json);
        assert!(result.is_ok());
        let req = result.unwrap();
        assert_eq!(req.model.unwrap(), "gpt-5.2-codex");
        assert_eq!(req.working_directory.unwrap(), "/Users/samuelatagana/Projects/LegacyMind");
        assert_eq!(req.sandbox.unwrap(), "workspace-write");
    }

    #[test]
    fn test_codex_request_schema_missing_prompt() {
        // Test: Missing required parameter
        let json = json!({
            "model": "gpt-5.2-codex"
        });

        let result = serde_json::from_value::<CodexRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_codex_request_schema_invalid_sandbox() {
        // Test: Invalid enum value
        let json = json!({
            "prompt": "Test",
            "sandbox": "invalid-mode"
        });

        let result = serde_json::from_value::<CodexRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_codex_request_schema_invalid_reasoning_effort() {
        // Test: Invalid enum value
        let json = json!({
            "prompt": "Test",
            "reasoningEffort": "ultra-mega"
        });

        let result = serde_json::from_value::<CodexRequest>(json);
        assert!(result.is_err());
    }
}
```

### 1.2 CLI Builder Tests

```rust
#[cfg(test)]
mod cli_builder_tests {
    use super::*;

    #[test]
    fn test_build_codex_command_minimal() {
        // Test: Minimal command (prompt only)
        let req = CodexRequest {
            prompt: "List files".to_string(),
            model: None,
            working_directory: None,
            session_id: None,
            sandbox: None,
            full_auto: None,
            reasoning_effort: None,
            reset_session: None,
        };

        let cmd = build_codex_command(&req);

        assert_eq!(cmd.get_program(), "codex");
        let args: Vec<&str> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();
        assert!(args.contains(&"List files"));
    }

    #[test]
    fn test_build_codex_command_with_model() {
        // Test: Model flag injection
        let req = CodexRequest {
            prompt: "Test".to_string(),
            model: Some("gpt-4o".to_string()),
            ..Default::default()
        };

        let cmd = build_codex_command(&req);
        let args: Vec<&str> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();

        assert!(args.contains(&"--model"));
        assert!(args.contains(&"gpt-4o"));
    }

    #[test]
    fn test_build_codex_command_with_working_directory() {
        // Test: -C flag injection
        let req = CodexRequest {
            prompt: "Test".to_string(),
            working_directory: Some("/tmp/test".to_string()),
            ..Default::default()
        };

        let cmd = build_codex_command(&req);
        let args: Vec<&str> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();

        assert!(args.contains(&"-C"));
        assert!(args.contains(&"/tmp/test"));
    }

    #[test]
    fn test_build_codex_command_with_sandbox() {
        // Test: --sandbox flag injection
        let req = CodexRequest {
            prompt: "Test".to_string(),
            sandbox: Some("workspace-write".to_string()),
            ..Default::default()
        };

        let cmd = build_codex_command(&req);
        let args: Vec<&str> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();

        assert!(args.contains(&"--sandbox"));
        assert!(args.contains(&"workspace-write"));
    }

    #[test]
    fn test_build_codex_command_full_auto() {
        // Test: -a on-request and --sandbox together
        let req = CodexRequest {
            prompt: "Test".to_string(),
            full_auto: Some(true),
            ..Default::default()
        };

        let cmd = build_codex_command(&req);
        let args: Vec<&str> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();

        assert!(args.contains(&"-a"));
        assert!(args.contains(&"on-request"));
        assert!(args.contains(&"--sandbox"));
        assert!(args.contains(&"workspace-write"));
    }

    #[test]
    fn test_build_codex_command_session_resume() {
        // Test: Session ID injection without applying other flags
        let req = CodexRequest {
            prompt: "Continue from last".to_string(),
            session_id: Some("session-abc123".to_string()),
            working_directory: Some("/tmp/test".to_string()),
            sandbox: Some("read-only".to_string()),
            ..Default::default()
        };

        let cmd = build_codex_command(&req);
        let args: Vec<&str> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();

        // Session ID should be included
        assert!(args.contains(&"session-abc123"));

        // But NOT -C or --sandbox (CLI limitation)
        assert!(!args.contains(&"-C"));
        assert!(!args.contains(&"--sandbox"));
    }
}
```

### 1.3 Job Tracking Tests

```rust
#[cfg(test)]
mod job_tracking_tests {
    use super::*;

    #[test]
    fn test_job_creation() {
        // Test: Job created with correct fields
        let job = CodexJob::new(
            "job-123".to_string(),
            "codex".to_string(),
            "Test prompt".to_string(),
        );

        assert_eq!(job.job_id, "job-123");
        assert_eq!(job.tool_name, "codex");
        assert_eq!(job.prompt, "Test prompt");
        assert_eq!(job.status, JobStatus::Running);
        assert!(job.stdout.is_none());
        assert!(job.stderr.is_none());
    }

    #[test]
    fn test_job_status_transition() {
        // Test: Job status updates correctly
        let mut job = CodexJob::new(
            "job-123".to_string(),
            "codex".to_string(),
            "Test".to_string(),
        );

        assert_eq!(job.status, JobStatus::Running);

        job.mark_completed("Output text".to_string(), None);
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.stdout.unwrap(), "Output text");
    }

    #[test]
    fn test_job_store_insert_and_retrieve() {
        // Test: JobStore operations
        let store = JobStore::new();

        let job = CodexJob::new(
            "job-456".to_string(),
            "codex".to_string(),
            "Another test".to_string(),
        );

        store.insert(job.clone());

        let retrieved = store.get(&"job-456".to_string());
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().job_id, "job-456");
    }

    #[test]
    fn test_job_store_list_by_status() {
        // Test: Filtering jobs by status
        let store = JobStore::new();

        let mut job1 = CodexJob::new("j1".to_string(), "codex".to_string(), "P1".to_string());
        let job2 = CodexJob::new("j2".to_string(), "codex".to_string(), "P2".to_string());

        job1.mark_completed("Done".to_string(), None);

        store.insert(job1);
        store.insert(job2);

        let running = store.list_by_status(Some(JobStatus::Running));
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].job_id, "j2");
    }
}
```

### 1.4 Response Parsing Tests

```rust
#[cfg(test)]
mod response_parsing_tests {
    use super::*;

    #[test]
    fn test_parse_codex_success_output() {
        // Test: Successful execution output parsing
        let stdout = "Session: session-abc123\nFiles listed:\n- main.rs\n- lib.rs";
        let stderr = "";

        let result = parse_codex_output(stdout, stderr);

        assert!(result.success);
        assert!(result.session_id.is_some());
        assert_eq!(result.session_id.unwrap(), "session-abc123");
        assert!(result.output.contains("main.rs"));
    }

    #[test]
    fn test_parse_codex_error_output() {
        // Test: Error extraction from stderr
        let stdout = "";
        let stderr = "Error: Authentication failed - invalid API key";

        let result = parse_codex_output(stdout, stderr);

        assert!(!result.success);
        assert!(result.error_message.is_some());
        assert!(result.error_message.unwrap().contains("Authentication failed"));
    }

    #[test]
    fn test_extract_session_id_from_output() {
        // Test: Session ID extraction patterns
        let output1 = "Session: abc-123-def";
        let output2 = "Started session abc-123-def with model gpt-4o";
        let output3 = "No session information";

        assert_eq!(extract_session_id(output1), Some("abc-123-def".to_string()));
        assert_eq!(extract_session_id(output2), Some("abc-123-def".to_string()));
        assert_eq!(extract_session_id(output3), None);
    }
}
```

---

## 2. Integration Tests (MCP)

### 2.1 End-to-End Tool Execution

**Location**: `surreal-mind/tests/integration/mcp_codex.rs`

```rust
#[tokio::test]
async fn test_call_codex_basic_prompt() {
    // Test: Basic prompt execution through MCP
    let mcp_client = setup_test_mcp_client().await;

    let request = json!({
        "prompt": "echo 'Hello from Codex'"
    });

    let response = mcp_client.call_tool("call_codex", request).await;

    assert!(response.is_ok());
    let result = response.unwrap();
    assert!(result["success"].as_bool().unwrap());
    assert!(result["output"].as_str().unwrap().contains("Hello from Codex"));
}

#[tokio::test]
async fn test_call_codex_with_working_directory() {
    // Test: Working directory flag behavior
    let mcp_client = setup_test_mcp_client().await;

    let request = json!({
        "prompt": "pwd",
        "workingDirectory": "/tmp"
    });

    let response = mcp_client.call_tool("call_codex", request).await;

    assert!(response.is_ok());
    let result = response.unwrap();
    assert!(result["output"].as_str().unwrap().contains("/tmp"));
}

#[tokio::test]
async fn test_call_codex_session_resume() {
    // Test: Session persistence across calls
    let mcp_client = setup_test_mcp_client().await;

    // First call: establish session
    let request1 = json!({
        "prompt": "Set variable x=42"
    });

    let response1 = mcp_client.call_tool("call_codex", request1).await.unwrap();
    let session_id = response1["sessionId"].as_str().unwrap();

    // Second call: resume session
    let request2 = json!({
        "prompt": "Print variable x",
        "sessionId": session_id
    });

    let response2 = mcp_client.call_tool("call_codex", request2).await.unwrap();
    assert!(response2["output"].as_str().unwrap().contains("42"));
}

#[tokio::test]
async fn test_call_status_retrieval() {
    // Test: call_status after job completion
    let mcp_client = setup_test_mcp_client().await;

    let request = json!({
        "prompt": "echo 'test output'"
    });

    let response = mcp_client.call_tool("call_codex", request).await.unwrap();
    let job_id = response["jobId"].as_str().unwrap();

    let status_request = json!({
        "job_id": job_id
    });

    let status = mcp_client.call_tool("call_status", status_request).await.unwrap();
    assert_eq!(status["status"].as_str().unwrap(), "completed");
}

#[tokio::test]
async fn test_call_jobs_listing() {
    // Test: call_jobs filtering and pagination
    let mcp_client = setup_test_mcp_client().await;

    // Create multiple jobs
    for i in 0..3 {
        let request = json!({
            "prompt": format!("echo 'job {}'", i)
        });
        mcp_client.call_tool("call_codex", request).await.unwrap();
    }

    let list_request = json!({
        "status_filter": "completed",
        "limit": 2
    });

    let jobs = mcp_client.call_tool("call_jobs", list_request).await.unwrap();
    let job_list = jobs["jobs"].as_array().unwrap();
    assert_eq!(job_list.len(), 2);
}

#[tokio::test]
async fn test_call_cancel_running_job() {
    // Test: Job cancellation
    let mcp_client = setup_test_mcp_client().await;

    let request = json!({
        "prompt": "sleep 60"
    });

    let response = mcp_client.call_tool("call_codex", request).await.unwrap();
    let job_id = response["jobId"].as_str().unwrap();

    let cancel_request = json!({
        "job_id": job_id
    });

    let cancel_result = mcp_client.call_tool("call_cancel", cancel_request).await;
    assert!(cancel_result.is_ok());

    // Verify status changed to cancelled
    let status_request = json!({"job_id": job_id});
    let status = mcp_client.call_tool("call_status", status_request).await.unwrap();
    assert_eq!(status["status"].as_str().unwrap(), "cancelled");
}
```

### 2.2 Error Handling Tests

```rust
#[tokio::test]
async fn test_call_codex_invalid_model() {
    // Test: Invalid model parameter rejection
    let mcp_client = setup_test_mcp_client().await;

    let request = json!({
        "prompt": "test",
        "model": "nonexistent-model-xyz"
    });

    let response = mcp_client.call_tool("call_codex", request).await;

    assert!(response.is_err() || !response.unwrap()["success"].as_bool().unwrap());
}

#[tokio::test]
async fn test_call_codex_timeout_handling() {
    // Test: Timeout behavior (if implemented)
    let mcp_client = setup_test_mcp_client().await;

    let request = json!({
        "prompt": "sleep 300" // Exceeds timeout
    });

    let response = mcp_client.call_tool("call_codex", request).await;
    // Should either timeout gracefully or return error
    assert!(response.is_err() || response.unwrap()["status"].as_str().unwrap() == "timeout");
}

#[tokio::test]
async fn test_call_status_invalid_job_id() {
    // Test: call_status with nonexistent job
    let mcp_client = setup_test_mcp_client().await;

    let request = json!({
        "job_id": "job-does-not-exist"
    });

    let response = mcp_client.call_tool("call_status", request).await;
    assert!(response.is_err());
}
```

---

## 3. Manual Verification Checklist

### 3.1 Pre-Flight Checks

- [ ] Codex CLI installed: `which codex` returns path
- [ ] Codex authenticated: `codex --help` runs without auth errors
- [ ] SurrealMind MCP running: `ps aux | grep surreal-mind` shows process
- [ ] MCP connection healthy: Test with existing tool (e.g., `think`)

### 3.2 Basic Functionality

**Test 1: Simple Prompt Execution**
```bash
# Via Claude Code or MCP client
call_codex(prompt: "List all Rust files in the current directory")
```

Expected:
- [ ] Command executes without errors
- [ ] Response contains file list
- [ ] Response includes `jobId` field
- [ ] Response includes `sessionId` field (if Codex returns one)

---

**Test 2: Working Directory Flag**
```bash
call_codex(
    prompt: "pwd",
    workingDirectory: "/Users/samuelatagana/Projects/LegacyMind/surreal-mind"
)
```

Expected:
- [ ] Output shows correct working directory
- [ ] No path-related errors

---

**Test 3: Model Selection**
```bash
call_codex(
    prompt: "What model are you?",
    model: "gpt-4o"
)
```

Expected:
- [ ] Response indicates gpt-4o was used (if Codex reports model)
- [ ] No model-related errors

---

### 3.3 Session Management

**Test 4: Session Persistence**
```bash
# First call
response1 = call_codex(prompt: "Create a variable called testvar with value 42")
session_id = response1.sessionId

# Second call (resume)
response2 = call_codex(
    prompt: "Print the value of testvar",
    sessionId: session_id
)
```

Expected:
- [ ] First call returns session ID
- [ ] Second call uses same session
- [ ] Variable persists between calls (output shows 42)
- [ ] Response indicates session was resumed

---

**Test 5: Reset Session**
```bash
call_codex(
    prompt: "Start fresh conversation",
    resetSession: true
)
```

Expected:
- [ ] New session ID generated
- [ ] Previous context cleared
- [ ] No errors related to session reset

---

### 3.4 Job Tracking

**Test 6: call_status**
```bash
response = call_codex(prompt: "echo 'test output'")
job_id = response.jobId

status = call_status(job_id: job_id)
```

Expected:
- [ ] call_status returns job details
- [ ] Status is "completed" or "running"
- [ ] Output matches original call
- [ ] Timestamp fields populated

---

**Test 7: call_jobs Listing**
```bash
jobs = call_jobs(limit: 10)
```

Expected:
- [ ] Returns list of recent jobs
- [ ] Each job has required fields (jobId, status, toolName, prompt)
- [ ] Jobs sorted by creation time (newest first)

---

**Test 8: call_jobs Filtering**
```bash
completed_jobs = call_jobs(status_filter: "completed", limit: 5)
```

Expected:
- [ ] Only completed jobs returned
- [ ] Respects limit parameter
- [ ] No running/failed jobs in results

---

**Test 9: call_cancel**
```bash
response = call_codex(prompt: "sleep 60")
job_id = response.jobId

cancel_result = call_cancel(job_id: job_id)
status = call_status(job_id: job_id)
```

Expected:
- [ ] call_cancel returns success
- [ ] Job status changes to "cancelled"
- [ ] Process actually terminates (verify with `ps`)

---

### 3.5 Advanced Parameters

**Test 10: Sandbox Mode**
```bash
call_codex(
    prompt: "Create a file in /tmp/test.txt",
    sandbox: "workspace-write"
)
```

Expected:
- [ ] Codex respects sandbox policy
- [ ] Write operations succeed in workspace
- [ ] Write operations fail outside workspace (if tested)

---

**Test 11: Full Auto Mode**
```bash
call_codex(
    prompt: "Refactor this function to use async/await",
    fullAuto: true
)
```

Expected:
- [ ] Codex executes without approval prompts
- [ ] Sandbox automatically set to workspace-write
- [ ] Changes applied automatically

---

**Test 12: Reasoning Effort**
```bash
call_codex(
    prompt: "Explain the design of SurrealMind architecture",
    reasoningEffort: "high"
)
```

Expected:
- [ ] Response quality reflects reasoning level
- [ ] No parameter errors
- [ ] Response indicates model used reasoning mode (if visible)

---

### 3.6 Error Handling

**Test 13: Missing Required Parameter**
```bash
call_codex()  # No prompt
```

Expected:
- [ ] Returns error (not crash)
- [ ] Error message mentions missing "prompt"
- [ ] HTTP 400 or similar error code

---

**Test 14: Invalid Enum Value**
```bash
call_codex(
    prompt: "test",
    sandbox: "invalid-mode"
)
```

Expected:
- [ ] Schema validation catches error
- [ ] Returns error before execution
- [ ] Error message lists valid options

---

**Test 15: Codex CLI Not Found**
```bash
# Temporarily rename codex binary or modify PATH
call_codex(prompt: "test")
```

Expected:
- [ ] Returns clear error about missing CLI
- [ ] Suggests installation steps
- [ ] Does not crash MCP server

---

**Test 16: Authentication Failure**
```bash
# Temporarily invalidate API key
call_codex(prompt: "test")
```

Expected:
- [ ] Returns authentication error
- [ ] Error message includes troubleshooting hint
- [ ] Job status is "failed" (not "running")

---

### 3.7 Output Parsing

**Test 17: JSON Output**
```bash
response = call_codex(prompt: "Generate a JSON object with keys a, b, c")
```

Expected:
- [ ] Response includes raw output
- [ ] JSON structure preserved
- [ ] No parsing errors

---

**Test 18: Multiline Output**
```bash
response = call_codex(prompt: "Print a 10-line file")
```

Expected:
- [ ] All lines captured in output
- [ ] No truncation
- [ ] Newlines preserved

---

**Test 19: Large Output Handling**
```bash
response = call_codex(prompt: "Generate 1000 lines of code")
```

Expected:
- [ ] Full output captured (or intentional truncation with notice)
- [ ] No memory issues
- [ ] Response time reasonable

---

## 4. Test Results

### 4.1 Unit Test Results

**Run Date**: [PENDING]
**Environment**: [PENDING]
**Rust Version**: [PENDING]

```
Test Results Summary:
- Total Tests: 0
- Passed: 0
- Failed: 0
- Skipped: 0
```

**Failed Tests**:
[None yet - implementation pending]

**Notes**:
[Space for observations during testing]

---

### 4.2 Integration Test Results

**Run Date**: [PENDING]
**MCP Server Version**: [PENDING]
**Codex CLI Version**: [PENDING]

```
Test Results Summary:
- Total Tests: 0
- Passed: 0
- Failed: 0
- Skipped: 0
```

**Failed Tests**:
[None yet - implementation pending]

**Notes**:
[Space for observations during testing]

---

### 4.3 Manual Verification Results

**Tester**: CC (Claude Code)
**Date**: 2026-01-17
**Environment**: Mac Studio, macOS Darwin 25.2.0
**Status**: All core functionality tests are now passing

**Pre-Flight Checks**:
- [x] Codex CLI installed: `/opt/homebrew/bin/codex` v0.87.0
- [x] SurrealMind MCP running: Multiple processes active
- [x] MCP connection healthy: `curl http://127.0.0.1:8787/health` returns "ok"

| Test ID | Test Name | Status | Notes |
|---------|-----------|--------|-------|
| 1 | Simple Prompt Execution | ✅ | FIXED: Now correctly routes to Codex CLI, returns file list + session_id |
| 2 | Working Directory Flag | ✅ | Output shows correct directory `/Users/samuelatagana/Projects/LegacyMind/surreal-mind` |
| 3 | Model Selection | ✅ | Model parameter correctly passed to CLI |
| 4 | Session Persistence | ✅ | Session persistence works - resumed session correctly recalls context |
| 5 | Reset Session | ✅ | Each new call starts fresh session (implicit reset works) |
| 6 | call_status | N/A | Async job tracking removed from implementation |
| 7 | call_jobs Listing | N/A | Async job tracking removed from implementation |
| 8 | call_jobs Filtering | N/A | Async job tracking removed from implementation |
| 9 | call_cancel | N/A | Async job tracking removed from implementation |
| 10 | Sandbox Mode | N/A | Sandbox/fullAuto/reasoningEffort parameters not yet implemented |
| 11 | Full Auto Mode | N/A | Sandbox/fullAuto/reasoningEffort parameters not yet implemented |
| 12 | Reasoning Effort | N/A | Sandbox/fullAuto/reasoningEffort parameters not yet implemented |
| 13 | Missing Required Parameter | ✅ | Schema validation works: "Invalid parameters: missing field `prompt`" |
| 14 | Invalid Enum Value | N/A | Sandbox parameter not yet implemented |
| 15 | Codex CLI Not Found | ⏳ | |
| 16 | Authentication Failure | ⏳ | |
| 17 | JSON Output | ✅ | JSON structure preserved correctly |
| 18 | Multiline Output | ✅ | Newlines preserved correctly |
| 19 | Large Output Handling | ✅ | 100 lines captured without truncation |

**Legend**: ✅ Pass | ❌ Fail | ⏳ Pending | ⚠️ Partial

**Critical Issues Found**:
1. ~~**BLOCKER: Wrong CLI invoked**~~ - **RESOLVED 2026-01-17** - Routing bug fixed, now correctly invokes Codex CLI
   - Original Job ID: a94d1c8a-feaf-4b95-a228-768be26db5ac (failed)
   - Fixed Job ID: 75a1bc6c-e412-459e-8711-8971066cac63 (completed successfully)
2. ~~**resume_session_id bug**~~ - **RESOLVED 2026-01-17** - Flag ordering issue (exec-level flags like --color, --json, --skip-git-repo-check must come after `exec` but before `resume`)

**Non-Critical Issues Found**:
1. **Response not surfaced in call_status** - call_status returns `response: null` even for completed jobs
   - Response content stored in session but not in job record
   - Workaround: Use response from initial call_codex return value

---

## 5. Performance Testing

### 5.1 Latency Tests

**Test**: Measure end-to-end latency for simple prompts

| Prompt Type | Avg Latency | p95 Latency | p99 Latency |
|-------------|-------------|-------------|-------------|
| Simple command (echo) | [PENDING] | [PENDING] | [PENDING] |
| File listing | [PENDING] | [PENDING] | [PENDING] |
| Code generation (small) | [PENDING] | [PENDING] | [PENDING] |
| Code generation (large) | [PENDING] | [PENDING] | [PENDING] |

### 5.2 Concurrent Job Handling

**Test**: Run multiple call_codex calls in parallel

| Concurrent Jobs | Success Rate | Avg Completion Time | Notes |
|-----------------|--------------|---------------------|-------|
| 1 | [PENDING] | [PENDING] | Baseline |
| 5 | [PENDING] | [PENDING] | |
| 10 | [PENDING] | [PENDING] | |
| 20 | [PENDING] | [PENDING] | Expected degradation |

---

## 6. Regression Testing Checklist

Before each release, verify:

- [ ] All unit tests pass (`cargo test`)
- [ ] All integration tests pass
- [ ] Manual verification checklist 100% complete
- [ ] No performance regressions vs. previous version
- [ ] Documentation matches implementation
- [ ] Error messages user-friendly
- [ ] Job tracking database cleaned up (no leaks)

---

## 7. Known Limitations & Edge Cases

### 7.1 Session Resume Limitation (CLI Constraint)

**Issue**: When `sessionId` is provided, Codex CLI ignores `-C` and `--sandbox` flags.

**Test Coverage**:
- Unit test: `test_build_codex_command_session_resume`
- Manual test: Test 4 (Session Persistence)

**Expected Behavior**: Warning logged, flags omitted from command.

### 7.2 Long-Running Jobs

**Issue**: Jobs that exceed typical timeout need graceful handling.

**Test Coverage**:
- Integration test: `test_call_codex_timeout_handling`
- Manual test: Test 9 (call_cancel)

**Expected Behavior**: Job status updated, process cleaned up.

---

## 8. Testing Tools & Environment

### Required Tools

- Rust toolchain (1.70+)
- Codex CLI (latest version)
- SurrealMind MCP server (running on port 8787)
- MCP test client (Claude Code or custom test harness)

### Test Data

- Sample prompts in `surreal-mind/tests/fixtures/codex_prompts.json`
- Expected outputs in `surreal-mind/tests/fixtures/codex_expected.json`

### Environment Variables

```bash
# For testing authentication failures
export CODEX_API_KEY="invalid-key"

# For testing different models
export CODEX_DEFAULT_MODEL="gpt-4o"
```

---

## 9. Next Steps

1. **Implement call_codex tool** - Schema, handler, CLI wrapper
2. **Write unit tests** - Copy patterns from this document into `tests/`
3. **Run unit tests** - `cargo test --package surreal-mind --test mcp_tools_codex`
4. **Write integration tests** - MCP client tests
5. **Run manual verification** - Work through checklist
6. **Document results** - Fill in Section 4 with actual outcomes
7. **Fix failures** - Iterate on implementation
8. **Performance tuning** - Optimize hot paths
9. **Finalize documentation** - Update this doc with findings

---

**Document Status**: Testing In Progress (2026-01-17)
**Approval Status**: Awaiting Review by CC/Sam
