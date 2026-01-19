---
date: 2025-12-30
prompt type: Implementation Plan (Database)
justification: Implementing the "SubconsciousLink" persistence layer for Federation Agents.
status: Draft
implementation date: TBD
related_docs:
  - docs/prompts/20251228-delegate-gemini-tool.md
---

# Database Implementation: `agent_exchanges` & `tool_sessions`

## 1. Objective
Implement the SurrealDB schema required to support the `PersistedAgent` middleware.
This enables the "Provenance & Synthesis" architecture defined by Opus.

## 2. Schema Definition

### 2.1 `agent_exchanges` (The Raw Log)
Immutable record of every interaction with an external agent.

```sql
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

DEFINE INDEX idx_exchanges_session ON agent_exchanges FIELDS session_id;
DEFINE INDEX idx_exchanges_tool ON agent_exchanges FIELDS tool_name;
```

### 2.2 `tool_sessions` (The State)
Tracks the latest session ID for a given tool/task to enable continuity.

```sql
DEFINE TABLE tool_sessions SCHEMAFULL;
DEFINE FIELD tool_name ON tool_sessions TYPE string;            -- Primary Key (unique)
DEFINE FIELD last_agent_session_id ON tool_sessions TYPE string;
DEFINE FIELD last_exchange_id ON tool_sessions TYPE record<agent_exchanges>;
DEFINE FIELD exchange_count ON tool_sessions TYPE int DEFAULT 0;
DEFINE FIELD last_updated ON tool_sessions TYPE datetime DEFAULT time::now();

DEFINE INDEX idx_sessions_tool ON tool_sessions FIELDS tool_name UNIQUE;
```

### 2.3 `thoughts` (Updates)
Linking synthesized thoughts back to their raw origin.

```sql
DEFINE FIELD source_exchange_id ON thoughts TYPE option<record<agent_exchanges>>;
DEFINE FIELD synthesis_type ON thoughts TYPE option<string>;
```

## 3. Rust Implementation Strategy

### 3.1 `src/server/schema.rs`
Update `initialize_schema` to include these `DEFINE` statements.

### 3.2 Atomic Upsert Logic
Instead of a complex `DEFINE FUNCTION` (which can be brittle in SurrealQL), we will implement the **Atomic Upsert** in Rust using a Transaction pattern in `src/utils/db.rs`.

**Logic:**
```rust
// In src/utils/db.rs
pub async fn upsert_tool_session(db: &Surreal<Client>, tool: &str, session: &str, exchange: &str) -> Result<()> {
    let sql = "
        BEGIN TRANSACTION;
        LET $current = SELECT * FROM tool_sessions WHERE tool_name = $tool LIMIT 1;
        IF array::is_empty($current) {
            CREATE tool_sessions CONTENT {
                tool_name: $tool,
                last_agent_session_id: $session,
                last_exchange_id: $exchange,
                exchange_count: 1,
                last_updated: time::now()
            };
        } ELSE {
            UPDATE tool_sessions SET
                last_agent_session_id = $session,
                last_exchange_id = $exchange,
                exchange_count += 1,
                last_updated = time::now()
            WHERE tool_name = $tool;
        };
        COMMIT TRANSACTION;
    ";
    // ... bind and execute
}
```

## 4. Verification Plan
- [ ] **Schema Init:** Run `cargo run` and verify no schema errors on startup.
- [ ] **Transaction Test:** Create a unit test in `src/utils/db.rs` that runs the upsert logic concurrently to prove atomicity.
