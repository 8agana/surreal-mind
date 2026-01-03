# Inner Voice NLQ - FINAL Implementation (All Feedback Incorporated)

## Critical Fixes from Warp (MUST DO)
1. **ILIKE doesn't exist in SurrealDB** – Use regex: `content ~ '(?i)excited'` (with proper escaping).
2. **Parameterize everything** – No string concatenated SQL; only whitelist non-bindable clauses (e.g., ORDER BY).
3. **OpenAI 1536 dims PRIMARY** – Use active embedder dimensions (normally 1536). Log/warn on mismatches; do not process mixed dims.

## Quick Implementation (4-6 hours)

### 1. Correct SurrealDB Query Pattern (regex-escaped + whitelisted ORDER)
```rust
// WRONG (ILIKE doesn't exist):
// content ILIKE '%excited%'

use regex::escape as rx_escape;

// Build ORDER BY via whitelist (cannot bind ORDER BY safely)
fn order_clause(order: Option<&str>) -> &'static str {
    match order {
        Some("created_at_asc") => "ORDER BY created_at ASC",
        _ => "ORDER BY created_at DESC",
    }
}

// Regex with case-insensitive flag and escaping per keyword
let escaped: Vec<String> = keywords.iter().map(|k| rx_escape(k)).collect();
let keyword_regex = if escaped.is_empty() {
    String::from(".*") // match-all
} else {
    format!("(?i)({})", escaped.join("|"))
};

let max_limit = self.config.nlq.max_limit.max(1);
let effective_limit = input.limit
    .unwrap_or(self.config.nlq.default_limit)
    .clamp(1, max_limit);

let dim = self.embedder.dimensions() as i64;
let base_sql = format!(
    "SELECT meta::id(id) as id, content, embedding, created_at \
     FROM thoughts \
     WHERE array::len(embedding) = $dim \
       AND created_at >= $from AND created_at < $to \
       AND content ~ $keyword_regex \
     {} \
     LIMIT $limit",
    order_clause(input.order.as_deref())
);

let results: Vec<Row> = self.db
    .query(base_sql)
    .bind(("dim", dim))
    .bind(("from", from))  // UTC Datetime
    .bind(("to", to))      // exclusive upper bound
    .bind(("keyword_regex", keyword_regex))
    .bind(("limit", effective_limit as i64))
    .await?
    .take(0)?;
```

### 2. Parsers with Stopwords
```rust
const STOPWORDS: &[&str] = &["The", "This", "That", "What", "When", "Where"];

fn extract_entities(query: &str) -> Vec<String> {
    let aliases = HashMap::from([
        ("sam", "Sam Atagana"),
        ("cc", "Claude Code"),
        ("codex", "Codex"),
    ]);
    
    query.split_whitespace()
        .filter(|w| w.chars().next().map_or(false, |c| c.is_uppercase()))
        .filter(|w| !STOPWORDS.contains(w))
        .filter_map(|w| aliases.get(w.to_lowercase().as_str()))
        .map(|s| s.to_string())
        .collect()
}
```

### 3. Temporal with Explicit Timezone (config-driven, DST-safe, testable)
```rust
use chrono::{DateTime, Duration, LocalResult, TimeZone, Utc};
use chrono_tz::Tz;

fn parse_temporal<TzNow: Fn() -> DateTime<Utc>>(
    phrase: &str,
    tz: Tz,
    now_utc: TzNow, // injectable clock for tests
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let now_local = now_utc().with_timezone(&tz);

    let day_start = |d: chrono::NaiveDate| -> Option<DateTime<Utc>> {
        match tz.with_ymd_and_hms(d.year(), d.month(), d.day(), 0, 0, 0) {
            LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => Some(dt.with_timezone(&Utc)),
            LocalResult::None => None,
        }
    };

    let (start_local, end_local) = match phrase {
        "yesterday" => {
            let y = now_local.date_naive().pred_opt()?;
            (y, y.succ_opt()?)
        }
        "two weeks ago" => {
            let target = now_local - Duration::weeks(2);
            let d = target.date_naive();
            (d, d.succ_opt()?)
        }
        _ => return None,
    };

    Some((day_start(start_local)?, day_start(end_local)?)) // [start, end)
}
```

### 4. Exclude Synthesis from Queries (aligns with existing schema)
```rust
// When saving inner_voice synthesis, we already stamp:
// is_summary = true, summary_of = [...], pipeline = 'inner_voice'

// Exclude synthesized rows in retrieval without adding a new 'kind' field:
let sql = "SELECT * FROM thoughts \
           WHERE (is_summary != true) \
             AND (pipeline IS NONE OR pipeline != 'inner_voice') \
             AND array::len(embedding) = $dim \
             AND ...";
```

### 5. NLQ Configuration (config-first, env only at startup)
```rust
// Extend main Config with an nlq section
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NlqConfig {
    pub timezone: String,      // IANA TZ, e.g., "America/Chicago"
    pub default_limit: usize,  // e.g., 25
    pub max_limit: usize,      // e.g., 100
    pub max_keywords: usize,   // cap keyword count for regex
    pub enable_keyword_filter: bool,
}

// At startup, read env overrides once and compute an effective runtime config
// Handlers should NOT read env per call.
```

### 6. Log Dimension Mismatches (no hardcoded 1536)
```rust
for row in candidates {
    if row.embedding.len() as i64 != self.embedder.dimensions() as i64 {
        tracing::warn!(
            thought_id = %row.id,
            actual_dim = row.embedding.len(),
            "Dimension mismatch - needs reembedding"
        );
        // Queue for maintenance_ops::reembed
        continue;
    }
    // Process normally
}
```

### 7. Retrieval Flow and Grounded Synthesis
- Stage A (prefilter): apply temporal window, dimension filter, and optional keyword regex.
- Stage B (rank): compute cosine similarity on the prefiltered set; take top N.
- Synthesis: return `{ answer, sources[] }`; refuse if snippet set is empty or coverage is too low. Keep temp ~0.2.

### 8. Logging & Metrics
- DEBUG log per request: `{ window_from, window_to, limit, keywords_count, candidates_scanned, regex_filtered, sim_evaluated, returned }`.
- WARN on env overrides (if any) and on dimension mismatches.

### 9. Performance Guards
- Use `config.retrieval.candidates` as an upper bound within the time window.
- Respect a local NLQ timeout; degrade gracefully (return partial/top-by-score) on timeout.

### 10. API Surface
- Params: `{ query: string, when?: string, limit?: number, order?: "created_at_desc"|"created_at_asc" }`.
- Response: `{ answer: string, sources: Array<{ id: string, created_at: string, score: number }> }`.

## Test These Queries First
1. "What are three accomplishments that Sam got excited about two weeks ago?"
2. "Show me frustrated thoughts from yesterday"
3. "Find mentions of surreal-mind this week"
4. "Last month’s proud moments"
5. "What did Codex say on Friday?"

## What NOT to Do
- Don't use ILIKE (doesn't exist in SurrealDB).
- Don't concatenate SQL; only generate whitelisted ORDER BY; everything else must be bound.
- Don't hardcode 1536; use the active embedder dimensions.
- Don't include synthesized rows (`is_summary=true` or `pipeline='inner_voice'`).
- Don't read env per call; read overrides once at startup.
- Don't assume BGE 384 is primary — it is dev/fallback only.

## Tests (Must Have)
- Deterministic windows for "yesterday", "two weeks ago", "this week", "last month" via injected clock.
- Regex escaping: inputs like `(excited)+?` compile safely and filter correctly.
- Exclusion: synthesized rows never appear in results.
- E2E: NLQ returns `{ answer, sources[] }` and refuses when ungrounded.
