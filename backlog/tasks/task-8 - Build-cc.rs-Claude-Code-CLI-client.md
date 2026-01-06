---
id: task-8
title: Build cc.rs - Claude Code CLI client
status: To Do
assignee: []
created_date: '2026-01-02 23:14'
updated_date: '2026-01-05 21:55'
labels:
  - surreal-mind
  - rust
  - federation
  - delegate-tools
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create the CcClient - a Rust wrapper around Claude Code CLI that implements the CognitiveAgent trait.

This is the low-level client that handles spawning `claude` commands, parsing JSON output, and managing session lifecycle. It mirrors the architecture of `src/clients/gemini.rs` but targets Claude Code instead of Gemini.

## Goals
- Non-interactive CLI calls (headless execution via `claude -p`)
- Parse JSON output from Claude Code (`--output-format json`)
- Support session continuation (`-c` for most recent, `-r session_id` for specific)
- Set working directory via `Command::current_dir(cwd)`
- Timeout handling (`CC_TIMEOUT_MS`)
- Implement CognitiveAgent trait for consistency

## Architecture

**Pattern:** `CcClient → spawn "claude -p" → parse JSON → return { response, session_id }`

### Key CLI Reference
1. **Non-interactive:** `claude -p "prompt"` or stdin
2. **Continue recent:** `claude -c -p "follow up"` (most recent in current dir)
3. **Resume specific:** `claude -r <session_id> -p "follow up"`
4. **CWD:** No --cwd flag - must use `Command::current_dir()`
5. **JSON output:** `--output-format json` (returns session_id, response, cost)
6. **Rate limiting:** `--max-turns N` for safety

### Components

| Item | Source | Purpose |
|------|--------|---------|
| CognitiveAgent trait | `src/clients/mod.rs` | Interface to implement |
| Gemini reference | `src/clients/gemini.rs` (385 lines) | Architecture pattern |
| Test structure | `tests/clients/` | Testing patterns |

### Data Flow

```
CcClient::call(prompt, cwd, session_id?)
    |
    v
Build command: "claude -p <prompt> --output-format json [options]"
    |
    v
Set working directory: Command::current_dir(cwd)
    |
    v
Spawn process with timeout
    |
    v
Parse JSON output: { session_id, response, cost?, ... }
    |
    v
Return { response, session_id } to caller
```

### Environment Config

| Variable | Default | Purpose |
|----------|---------|---------|
| `CC_TIMEOUT_MS` | `60000` | Inactivity timeout |
| `CC_CLI_PATH` | `claude` | Path to claude binary |

## Implementation Steps

1. Create `src/clients/cc.rs`
2. Implement CcClient struct with fields: timeout_ms, cli_path
3. Implement CognitiveAgent trait with call() method
4. Handle session IDs (-c, -r flags)
5. Parse JSON output and error cases
6. Add tests
7. Update `src/clients/mod.rs` to export CcClient

Acceptance Criteria:
--------------------------------------------------
- [ ] CcClient created in src/clients/cc.rs
- [ ] CognitiveAgent trait implemented
- [ ] Non-interactive mode works: spawns claude -p with JSON
- [ ] Session continuation works (-c flag)
- [ ] Session resume works (-r session_id)
- [ ] CWD parameter sets working directory correctly
- [ ] Timeout handling works
- [ ] Error cases handled (CLI not found, JSON parse failure, timeout)
- [ ] Unit tests pass
- [ ] Exported from src/clients/mod.rs
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 CcClient created in src/clients/cc.rs
- [ ] #2 CognitiveAgent trait implemented
- [ ] #3 Non-interactive mode works: spawns claude -p with JSON
- [ ] #4 Session continuation works (-c flag)
- [ ] #5 Session resume works (-r session_id)
- [ ] #6 CWD parameter sets working directory correctly
- [ ] #7 Timeout handling works
- [ ] #8 Error cases handled (CLI not found, JSON parse failure, timeout)
- [ ] #9 Unit tests pass

- [ ] #10 Exported from src/clients/mod.rs
<!-- AC:END -->
