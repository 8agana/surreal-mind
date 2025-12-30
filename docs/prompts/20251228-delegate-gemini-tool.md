---
date: 2025-12-28
prompt type: Implementation Plan (Tool)
justification: Enabling "The Federation" by exposing Gemini CLI to SurrealMind with persistence.
status: Draft
implementation date: TBD
related_docs:
  - docs/prompts/20251227-gemini-cli-implementation.md
---

# Tool Implementation: `delegate_gemini`

## 1. Objective
Create an MCP tool `delegate_gemini` that allows SurrealMind to spawn non-interactive Gemini tasks.
Crucially, this tool handles the **SubconsciousLink** (DB Persistence) that the raw client does not.

## 2. Architecture

### 2.1 Core Principle: Provenance & Separation
The key architectural insight: **preserve raw logs from internal synthesis**.
- Raw Gemini output flows into `agent_exchanges` (provenance-aware, immutable)
- Internal synthesis (thoughts, reasoning) flows into `thoughts` (mutable, frameworks applied)
- This separation prevents data loss while enabling clean cognitive processing

### 2.2 The Tool Handler
**Location:** `src/tools/delegate_gemini.rs`
**Responsibility:**
1.  Retrieve active session ID from SurrealDB (`tool_sessions` table) - atomic upsert pattern
2.  Invoke `GeminiClient::call()`
3.  Persist raw exchange as `agent_exchange` (immutable provenance record)
4.  Synthesize internal `thought` from the exchange (cognitive processing)
5.  Update `tool_sessions` with latest CLI session ID (atomic, single operation)

### 2.3 Data Schema & SQL

**Input (Request):**
```json
{
  "prompt": "Analyze the log files...",
  "task_name": "log_analysis_2025",     // Optional: Creates a named session
  "model": "gemini-2.5-pro",            // Optional: Override default
  "include_response_in_thought": true   // Optional: Allow synthesis of raw output
}
```

**Output (Response):**
```json
{
  "response": "Analysis complete...",
  "session_id": "cli-session-123",
  "exchange_id": "exchange:abc-123",     // Raw provenance record
  "thought_id": "thought:xyz-789"        // Synthesized cognitive record
}
```

### 2.4 Database Schema (Opus Design)

**New `agent_exchanges` Table:**
Separates raw inter-agent communication from internal synthesis.

```sql
-- Raw agent communication records (immutable provenance)
CREATE TABLE agent_exchanges (
  id TEXT PRIMARY KEY,
  agent_source TEXT NOT NULL,           -- 'gemini', 'claude', 'grok', etc.
  agent_instance TEXT NOT NULL,         -- 'gemini-2.5-pro', 'claude-opus-4-5', etc.
  prompt TEXT NOT NULL,
  response TEXT NOT NULL,
  timestamp TIMESTAMP NOT NULL DEFAULT NOW(),
  tool_name TEXT,                       -- Optional: Associated task/tool
  session_id TEXT,                      -- Optional: Session context
  metadata JSONB,                       -- Extensible: model_version, token_counts, latency, etc.
  created_at TIMESTAMP NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Transient tool session state (atomic upserts only)
CREATE TABLE tool_sessions (
  tool_name TEXT PRIMARY KEY,
  last_agent_session_id TEXT NOT NULL,
  exchange_count INT DEFAULT 0,         -- Normalized: count of exchanges in this session
  last_exchange_id TEXT,                -- Foreign key to agent_exchanges
  last_updated TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Internal synthesis (mutable, frameworks applied)
-- Existing 'thoughts' table maintains structure but now references agent_exchanges
ALTER TABLE thoughts ADD COLUMN (
  source_exchange_id TEXT,              -- Foreign key to agent_exchanges (provenance link)
  agent_source TEXT,                    -- Denormalized from exchange for query efficiency
  synthesis_type TEXT,                  -- 'raw_log', 'summary', 'analysis', 'question_response'
  framework_applied TEXT                -- Optional: framework used in synthesis ('SCAMPER', 'KG_extraction', etc.)
);

-- Atomic session upsert pattern (Codex enhancement)
-- Helper function for single atomic operation
CREATE FUNCTION upsert_tool_session(
  p_tool_name TEXT,
  p_session_id TEXT,
  p_exchange_id TEXT
) RETURNS TEXT AS $$
BEGIN
  INSERT INTO tool_sessions (tool_name, last_agent_session_id, last_exchange_id, exchange_count, last_updated)
  VALUES (p_tool_name, p_session_id, p_exchange_id,
    COALESCE((SELECT exchange_count FROM tool_sessions WHERE tool_name = p_tool_name), 0) + 1,
    NOW())
  ON CONFLICT(tool_name) DO UPDATE SET
    last_agent_session_id = p_session_id,
    last_exchange_id = p_exchange_id,
    exchange_count = tool_sessions.exchange_count + 1,
    last_updated = NOW();
  RETURN p_session_id;
END;
$$ LANGUAGE plpgsql;
```

### 2.5 Persistence Logic (The "Self" Memory)

**Session Lookup (Atomic Upsert Pattern - Codex):**
```sql
-- Single atomic operation: get or create session
SELECT last_agent_session_id
FROM tool_sessions
WHERE tool_name = $task_name
FOR UPDATE;  -- Lock row to prevent race conditions

-- If NULL, generate new session ID
-- Then upsert atomically via helper function
```

**Exchange Storage (Immutable Provenance):**
```sql
INSERT INTO agent_exchanges (
  id, agent_source, agent_instance, prompt, response,
  tool_name, session_id, metadata, timestamp
) VALUES (
  gen_id('exchange'),
  'gemini',
  $model_version,
  $prompt,
  $response,
  $task_name,
  $session_id,
  jsonb_build_object(
    'tokens_used', $token_count,
    'latency_ms', $latency,
    'model_config', $config
  ),
  NOW()
);
```

**Thought Synthesis (Internal Processing):**
```sql
INSERT INTO thoughts (
  content, source, prompt, session_id,
  source_exchange_id, agent_source, synthesis_type,
  framework_applied, created_at
) VALUES (
  $synthesized_response,
  'gemini_delegate',
  $original_prompt,
  $session_id,
  $exchange_id,
  'gemini',
  'response_synthesis',
  $framework_if_applied,
  NOW()
);
```

**Session Update (Atomic via Helper):**
```sql
SELECT upsert_tool_session($task_name, $session_id, $exchange_id);
```

## 3. Implementation Steps

### 3.1 Database Setup (Prerequisite)
- [ ] Create `agent_exchanges` table (immutable provenance records)
- [ ] Create `tool_sessions` table (atomic session state)
- [ ] Add new columns to `thoughts` table (`source_exchange_id`, `agent_source`, `synthesis_type`, `framework_applied`)
- [ ] Create `upsert_tool_session()` helper function (atomic single operation)
- [ ] Verify indexes on `agent_exchanges(tool_name, session_id)` and `thoughts(source_exchange_id)`

### 3.2 Code Structure
1.  **Dependencies:** Import `GeminiClient` from `crate::clients`
2.  **Centralized Helpers:** Create `src/utils/db.rs` module with:
    - `get_active_session(tool_name)` - retrieves last session ID with row lock
    - `persist_exchange(agent_exchanges_row)` - inserts immutable provenance record
    - `upsert_tool_session(tool_name, session_id, exchange_id)` - atomic DB helper
    - `create_thought_from_exchange(exchange_id, synthesis_type, framework)` - cognitive synthesis
3.  **Handler:** Implement `handle_delegate_gemini` in `src/tools/delegate_gemini.rs`:
    - Get or create session (atomic)
    - Invoke `GeminiClient::call(prompt, model)`
    - Persist raw exchange (immutable)
    - Synthesize thought from exchange (mutable, frameworks applied)
    - Update session atomically
    - Return all IDs to caller
4.  **Registration:** Register the tool in `src/server/router.rs` and `src/tools/mod.rs`

### 3.3 Codex Technical Fixes (Atomic Operations)
- **Single Atomic Operation:** Use `upsert_tool_session()` helper to prevent race conditions
- **No Multi-Step Updates:** Never split session get + update into separate queries
- **Field Normalization:** Store `exchange_count` in `tool_sessions` for efficient queries (denormalized from `agent_exchanges` count)
- **Error Handling:** If Gemini call fails, do NOT persist exchange or update session. Return error with context for retry.

### 3.4 Opus Architectural Vision (Provenance & Synthesis)
- **Raw Exchange:** Every Gemini interaction flows into `agent_exchanges` immediately (immutable)
  - Preserves complete context for audit, debugging, and historical analysis
  - Allows reconstruction of entire interaction history without data loss
- **Synthesized Thought:** Internal processing creates linked `thought` record
  - Connects via `source_exchange_id` (provenance link)
  - Marks `synthesis_type` ('response_synthesis', 'summary', 'analysis', etc.)
  - Documents `framework_applied` for reproducibility
  - Mutable because reasoning may improve over time
- **Separation Benefits:**
  - Raw logs never corrupted by cleanup operations
  - Cognitive frameworks don't degrade base data
  - Complete audit trail for agent exchanges
  - Can replay synthesis with new frameworks without re-querying Gemini

## 4. Verification Plan
- [ ] **Schema Creation:** Verify all tables and functions created without errors
- [ ] **Atomic Session:** Run tool twice with same `task_name`. Verify second run retrieves first session's ID
- [ ] **Provenance Storage:** Query `SELECT * FROM agent_exchanges WHERE tool_name = $task_name` - verify raw output is complete and immutable
- [ ] **Thought Synthesis:** Query `SELECT * FROM thoughts WHERE source_exchange_id = $exchange_id` - verify synthesis records exist with proper framework links
- [ ] **Session State:** Query `SELECT exchange_count FROM tool_sessions WHERE tool_name = $task_name` - verify count increments atomically
- [ ] **Error Handling:** Manually kill Gemini process mid-response. Verify DB session is not partially corrupted and tool can retry cleanly
- [ ] **Field Normalization:** Verify denormalized `exchange_count` stays in sync with actual `agent_exchanges` count for that session

## 5. Technical Coherence & Design Principles

### 5.1 Codex's Contribution (Operational Reliability)
The atomic upsert pattern eliminates entire classes of bugs:
- **No Race Conditions:** Single SQL operation prevents partial state corruption
- **Transactional Integrity:** Helper function guarantees exchange is persisted before session updates
- **Idempotency:** Retrying a failed operation doesn't double-count exchanges
- **Field Normalization:** Denormalized `exchange_count` prevents sum() queries while maintaining consistency

### 5.2 Opus's Contribution (Architectural Clarity)
The `agent_exchanges` separation embodies a core principle:
- **Provenance is Sacred:** Raw interaction logs are immutable audit trail, never contaminated by synthesis
- **Cognitive Processing is Mutable:** Thoughts can improve over time without losing base data
- **Composability:** New frameworks can be applied to existing exchanges without requerying Gemini
- **Distributed Agency:** Each agent's contribution is preserved with source attribution

### 5.3 Integrated Vision
This tool realizes the **SubconsciousLink** concept:
- Gemini CLI runs non-interactively (no terminal blocking)
- Results automatically persist as both raw exchange and synthesized thought
- Future delegations to Gemini, Codex, or other agents follow identical pattern
- Federation agents can reason about each other's raw exchanges (provenance) and synthesized thoughts (reasoning)
- Each agent's work is preserved, attributed, and available for future collaboration

The `delegate_gemini` tool is the first concrete instantiation of this architecture. Subsequent tools (`delegate_codex`, `delegate_grok`) will follow the same provenance + synthesis pattern, creating a unified federation communication substrate where all inter-agent exchanges are logged, attributed, and synthesized according to cognitive frameworks.
