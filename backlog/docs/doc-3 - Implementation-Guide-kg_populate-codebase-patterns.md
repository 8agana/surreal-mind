---
id: doc-3
title: Implementation Guide - kg_populate codebase patterns
type: other
created_date: '2025-12-31 22:50'
updated_date: '2025-12-31 23:23'
---
# Implementation Guide — kg_populate codebase patterns

**Related task**: task-1 (Implement kg_populate orchestrator binary)  
**Related docs**: doc-1 (Codex review), doc-2 (CC feasibility review)  
**Date**: 2025-12-31  
**Source**: Serena codebase investigation

---

## DATABASE SCHEMA (VERIFIED)

### thoughts table extraction fields
```sql
DEFINE FIELD extracted_to_kg ON TABLE thoughts TYPE bool DEFAULT false;
DEFINE FIELD extraction_batch_id ON TABLE thoughts TYPE option<string>;
DEFINE FIELD extracted_at ON TABLE thoughts TYPE option<datetime>;
```

**CRITICAL**: Field is `extracted_to_kg`, NOT `kg_extracted` (doc-1 was correct)

### KG tables extraction metadata
All three tables (kg_entities, kg_edges, kg_observations) have:
```sql
DEFINE FIELD source_thought_ids ON TABLE [table] TYPE option<array<string>>;
DEFINE FIELD extraction_batch_id ON TABLE [table] TYPE option<string>;
DEFINE FIELD extracted_at ON TABLE [table] TYPE option<datetime>;
DEFINE FIELD extraction_confidence ON TABLE [table] TYPE option<float>;
DEFINE FIELD extraction_prompt_version ON TABLE [table] TYPE option<string>;
```

**Location**: `src/server/schema.rs:30-94`

---

## CONFIG LOADING PATTERN

### Standard config initialization
```rust
use surreal_mind::config::Config;

let config = Config::load().map_err(|e| {
    eprintln!("Failed to load configuration: {}", e);
    e
})?;
```

### What Config::load() does
1. Loads `.env` from current dir or parent (smart fallback)
2. Reads `surreal_mind.toml` (or path from SURREAL_MIND_CONFIG env)
3. Applies env overrides: SURR_DB_URL, SURR_DB_NS, SURR_DB_DB
4. Loads runtime config (database_user, database_pass)
5. Validates provider/model/dimensions coherence

**Location**: `src/config.rs:146-314`

---

## DATABASE CONNECTION PATTERN

### Standard connection sequence (from reembed.rs)
```rust
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

let db = Surreal::new::<Ws>(&config.system.database_url).await?;
db.signin(Root {
    username: &config.runtime.database_user,
    password: &config.runtime.database_pass,
}).await?;
db.use_ns(&config.system.database_ns)
  .use_db(&config.system.database_db).await?;
```

**Location**: `src/bin/reembed.rs:28-38`

---

## DELEGATE_GEMINI INTEGRATION

### Sync execution pattern (execute_gemini_call helper)
```rust
// From src/tools/delegate_gemini.rs:430-454
async fn execute_gemini_call(
    db: std::sync::Arc<Surreal<WsClient>>,
    prompt: &str,
    task_name: &str,
    model_override: Option<&str>,
    cwd: Option<&str>,
    timeout: u64,
) -> std::result::Result<AgentResponse, AgentError> {
    let resume_session = fetch_last_session_id(db.as_ref(), task_name.to_string())
        .await
        .map_err(|e| AgentError::CliError(format!("Failed to fetch session: {}", e)))?;

    let model = model_override
        .map(|s| s.to_string())
        .unwrap_or_else(default_model_name);

    let mut gemini = GeminiClient::with_timeout_ms(model.clone(), timeout);
    if let Some(dir) = cwd {
        gemini = gemini.with_cwd(dir);
    }

    let agent = PersistedAgent::new(gemini, db.clone(), "gemini", model, task_name.to_string());

    agent.call(prompt, resume_session.as_deref()).await
}
```

### Return type: AgentResponse
```rust
struct AgentResponse {
    response: String,      // Raw Gemini output (may contain ```json fences)
    session_id: String,    // Gemini session ID
    exchange_id: String,   // Individual exchange ID
}
```

**Key detail**: Response may contain markdown code fences - strip before JSON parse

---

## KG UPSERT PATTERNS

### Entities (name uniqueness)
```rust
// Check existing by name
let sql = "SELECT meta::id(id) as id FROM kg_entities WHERE name = $name LIMIT 1";
let found: Vec<serde_json::Value> = db.query(sql).bind(("name", name)).await?.take(0)?;

if let Some(existing) = found.first() {
    // Entity exists, skip create
} else {
    // Create new entity
    db.query("CREATE kg_entities SET created_at = time::now(), name = $name, entity_type = $etype, data = $data RETURN meta::id(id) as id")
      .bind(("name", name))
      .bind(("etype", entity_type))
      .bind(("data", data))
      .await?.take(0)?;
}
```

### Edges (source, target, rel_type uniqueness)
```rust
let sql = "SELECT meta::id(id) as id FROM kg_edges WHERE source = $src AND target = $dst AND rel_type = $rel LIMIT 1";
let found: Vec<serde_json::Value> = db.query(sql)
    .bind(("src", source_thing))
    .bind(("dst", target_thing))
    .bind(("rel", rel_type))
    .await?.take(0)?;
```

### Observations (name, source_thought_id uniqueness)
```rust
let sql = "SELECT meta::id(id) as id FROM kg_observations WHERE name = $name AND data.source_thought_id = $src LIMIT 1";
let found: Vec<serde_json::Value> = db.query(sql)
    .bind(("name", name))
    .bind(("src", source_thought_id))
    .await?.take(0)?;
```

**Location**: `src/tools/knowledge_graph.rs:9-283`

---

## BINARY STRUCTURE PATTERN

### Main function template (from reembed.rs)
```rust
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load .env (optional, Config::load() does this too)
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Warning: Could not load .env file: {}", e);
    }

    // 2. Load config
    let config = surreal_mind::config::Config::load()?;

    // 3. Initialize components (embedder, clients, etc.)
    println!("✅ Component initialized");

    // 4. Connect to database
    let db = Surreal::new::<Ws>(&config.system.database_url).await?;
    // ... signin, use_ns, use_db ...

    // 5. Main processing loop
    println!("🔄 Processing...");
    let mut success_count = 0;
    let mut error_count = 0;

    for item in items {
        match process_item(item).await {
            Ok(_) => success_count += 1,
            Err(e) => {
                eprintln!("Error: {}", e);
                error_count += 1;
            }
        }
    }

    // 6. Summary
    println!("✅ Complete: {} success, {} errors", success_count, error_count);
    Ok(())
}
```

**Location**: `src/bin/reembed.rs:7-200` (pattern reference)

---

## ERROR HANDLING

### Standard error type: anyhow::Result
All binaries use `anyhow::Result<()>` for main, `anyhow::Result<T>` for helpers.

### Propagation pattern
```rust
let result: Vec<Value> = db.query(sql).await?.take(0)?;
```

### Custom errors: SurrealMindError
For library code (not binaries), use `surreal_mind::error::SurrealMindError`:
- `Mcp { message: String }`
- `InvalidParams { message: String }`
- `Database { message: String }`
- etc.

**Location**: `src/error.rs`

---

## LOGGING

### Print pattern (not tracing in binaries)
```rust
println!("🚀 Starting process...");
println!("✅ Component initialized");
println!("📊 Stats: {} items processed", count);
eprintln!("⚠️  Warning: {}", msg);
```

### Progress during loops
```rust
if i % 100 == 0 {
    println!("  Processed {}/{} items...", i, total);
}
```

---

## JSON PARSING WITH FENCE STRIPPING

### Pattern for Gemini responses
```rust
let raw_response = agent.call(prompt, session_id).await?.response;

// Strip markdown code fences
let json_str = raw_response
    .trim()
    .strip_prefix("```json")
    .unwrap_or(&raw_response)
    .strip_suffix("```")
    .unwrap_or(&raw_response)
    .trim();

let parsed: ExtractionResult = serde_json::from_str(json_str)?;
```

---

## RECOMMENDED DEPENDENCIES (for kg_populate binary)

```toml
[dependencies]
surreal-mind = { path = ".." }  # Access to config, clients, error types
surrealdb = "2.0"
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
dotenvy = "0.15"
uuid = { version = "1.0", features = ["v4"] }
```

---

## IMPLEMENTATION CHECKLIST

Based on Serena investigation, the following are **verified and ready**:

- [x] Database schema for `extracted_to_kg`, `extraction_batch_id`, `extracted_at`
- [x] Config loading pattern (Config::load())
- [x] Database connection pattern (Surreal::new, signin, use_ns/use_db)
- [x] delegate_gemini execution pattern (via execute_gemini_call or direct PersistedAgent)
- [x] KG upsert patterns (entities, edges, observations)
- [x] Binary structure template (main, error handling, logging)
- [x] JSON fence stripping pattern

**Still need decisions on** (from doc-2):
- [x] Batch error handling strategy (per-thought vs all-or-nothing) - **IMPLEMENTED: per-thought with logging**
- [x] Boundaries storage approach (dedicated table vs kg_observations mapping) - **IMPLEMENTED: dedicated kg_boundaries table**
- [x] Batch size default and env override - **IMPLEMENTED: 25 default, KG_POPULATE_BATCH_SIZE env var**
- [x] Extraction prompt location/creation - **IMPLEMENTED: src/prompts/kg_extraction_v1.md**

---

## IMPLEMENTATION COMPLETED

**Date**: 2025-12-31
**Implementer**: rust-builder (CC subagent)

### Files Created/Modified

1. **`src/prompts/kg_extraction_v1.md`** - Extraction prompt with JSON schema for entities, relationships, observations, boundaries
2. **`src/server/schema.rs`** - Added `kg_boundaries` table definition with extraction metadata fields
3. **`src/bin/kg_populate.rs`** - Complete orchestrator binary (~650 lines)

### Key Implementation Decisions

1. **Ownership for SurrealDB bindings**: All helper functions take owned `String` values instead of `&str` references because SurrealDB's `bind()` method requires `'static` lifetime. This required adding `Clone` derives to extraction structs.

2. **Gemini integration**: Used `PersistedAgent` wrapper directly (not `execute_gemini_call` helper) since we don't need session resume for independent extraction batches.

3. **Edge resolution**: Edges are only created if both source and target entities exist. If Gemini references an entity that wasn't extracted in the same batch, the edge is skipped (returns `false` for "not created").

4. **Observation naming**: Observations are named by truncating content to 50 chars + "..." to provide a human-readable identifier while maintaining uniqueness with `source_thought_id`.

5. **Thought marking**: Even thoughts that Gemini skips in its response are marked as extracted to prevent infinite retry loops. The batch continues processing regardless of individual thought failures.

### Issues Encountered and Fixed

1. **Clippy warning**: `existing.first().is_some()` flagged as unnecessary - changed to `!existing.is_empty()`

2. **Lifetime issues**: Initial implementation used `&str` references which don't satisfy SurrealDB's `'static` requirement for `bind()`. Fixed by converting all DB-bound values to owned `String` types.

### Build Validation Results

```
cargo build --bin kg_populate     # SUCCESS
cargo clippy --bin kg_populate    # SUCCESS (with -A clippy::too-many-arguments for pre-existing issue in delegate_gemini.rs)  
cargo fmt --check                 # SUCCESS
```

### Environment Variables

- `KG_POPULATE_BATCH_SIZE` - Number of thoughts per batch (default: 25)
- `KG_POPULATE_MODEL` - Gemini model to use (default: gemini-2.5-flash)
- `KG_POPULATE_TIMEOUT_MS` - Timeout for Gemini calls (default: 120000)
