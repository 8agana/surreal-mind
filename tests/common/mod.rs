//! Shared, DB-free assertions over the `think` tool's response envelope
//! (clu fed-77afac, review round 2).
//!
//! WHY THIS MODULE EXISTS -- read before editing an assertion here.
//!
//! `handle_legacymind_think` returns a JSON envelope whose interesting part
//! is nested under `delegated_result` (src/tools/thinking.rs). The runners
//! (src/tools/thinking/runners.rs) attach an `embedding_status` key to that
//! object ONLY when the status is not `"complete"`, so the offline
//! (FakeEmbedder) tests were asserting success by the ABSENCE of a failure
//! marker.
//!
//! Absence-as-success is fragile by construction: every way the response can
//! degrade also produces absence. Two previous attempts at this assertion
//! shipped with exactly that hole --
//!
//!   attempt 1: `parsed.get("embedding_status").is_none()` at the TOP level,
//!              where the key never appears at all -> always true.
//!   attempt 2: `delegated_result` was `.expect()`ed to exist, but nothing
//!              checked that it was an OBJECT, and nothing checked the status
//!              VALUE. `serde_json::Value::get` returns `None` for `{}`, `[]`,
//!              `null`, and every scalar alike, so all four degenerate shapes
//!              satisfied `.is_none()` and passed the offline test; and
//!              `{"embedding_status": null}` satisfied `.is_some()` and passed
//!              the network test.
//!
//! So the checks live here as a PURE, TOTAL function over `serde_json::Value`
//! (`embedding_outcome`) rather than as inline `assert!`s duplicated across
//! two test files. That buys three things:
//!   1. the degenerate shapes are directly testable with no database and no
//!      cargo features at all (tests/embedding_shape.rs),
//!   2. `tests/mcp_integration.rs` and `tests/mcp_protocol.rs` share one
//!      implementation instead of two drifting copies,
//!   3. a shape regression produces a named error instead of a silent pass.
//!
//! NOTE ON WHAT THIS CAN AND CANNOT PROVE: nothing in the response envelope
//! is a positive witness that a vector was actually persisted --
//! `embedding_dim` is `self.embedder.dimensions()`, a static property of the
//! configured embedder, not the length of anything stored. The real positive
//! witness is the persisted row, asserted separately against the database in
//! `tests/mcp_integration.rs::assert_persisted_embedding_complete`.

#![allow(dead_code)] // each test binary uses a different subset of these

use serde_json::{Map, Value};

/// Values `embedding_status` may legitimately carry.
///
/// `"pending"` and `"failed"` are what the runners write when embedding did
/// not complete (src/tools/thinking.rs: the builder returns one of
/// `"complete"`, `"pending"`, `"failed"`). `"complete"` is accepted as a
/// recognised value too, even though the runners currently omit the key in
/// that case -- an explicit success marker is a strictly better contract than
/// an absent one, so it must not be treated as a shape violation if it ever
/// starts being emitted.
pub const DEGRADED_STATUSES: &[&str] = &["pending", "failed"];

/// The embedding outcome a `think` response reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddingOutcome {
    /// `embedding_status` absent (current runner behaviour) or explicitly
    /// `"complete"`.
    Complete,
    /// `embedding_status` present and equal to one of [`DEGRADED_STATUSES`].
    Degraded(String),
}

fn json_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn preview(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 240 {
        let head: String = s.chars().take(240).collect();
        format!("{head}...")
    } else {
        s
    }
}

fn require_non_empty_string(map: &Map<String, Value>, key: &str) -> Result<String, String> {
    match map.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.clone()),
        Some(other) => Err(format!(
            "`delegated_result.{key}` must be a non-empty string, got {} ({})",
            json_kind(other),
            preview(other)
        )),
        None => Err(format!(
            "`delegated_result` is missing `{key}`; every runner payload sets it \
             (src/tools/thinking/runners.rs). Keys present: {:?}",
            map.keys().collect::<Vec<_>>()
        )),
    }
}

fn require_positive_u64(map: &Map<String, Value>, key: &str) -> Result<u64, String> {
    match map.get(key) {
        Some(Value::Number(n)) => match n.as_u64() {
            Some(v) if v > 0 => Ok(v),
            _ => Err(format!(
                "`delegated_result.{key}` must be a positive integer, got {n}"
            )),
        },
        Some(other) => Err(format!(
            "`delegated_result.{key}` must be a positive integer, got {} ({})",
            json_kind(other),
            preview(other)
        )),
        None => Err(format!(
            "`delegated_result` is missing `{key}`; every runner payload sets it \
             (src/tools/thinking/runners.rs). Keys present: {:?}",
            map.keys().collect::<Vec<_>>()
        )),
    }
}

/// Structural gate: `response` must be an object carrying a `delegated_result`
/// OBJECT that looks like an actual runner payload.
///
/// The three required siblings (`thought_id`, `embedding_model`,
/// `embedding_dim`) are unconditionally set by both `run_convo` and
/// `run_technical`, so requiring them is what makes `delegated_result == {}`
/// -- the shape that quietly satisfied every previous version of these
/// assertions -- a hard failure rather than a pass.
pub fn delegated_result(response: &Value) -> Result<&Map<String, Value>, String> {
    let envelope = response.as_object().ok_or_else(|| {
        format!(
            "think response must be a JSON object, got {} ({})",
            json_kind(response),
            preview(response)
        )
    })?;

    let delegated = envelope.get("delegated_result").ok_or_else(|| {
        format!(
            "think response is missing `delegated_result`. Keys present: {:?}",
            envelope.keys().collect::<Vec<_>>()
        )
    })?;

    let map = delegated.as_object().ok_or_else(|| {
        format!(
            "`delegated_result` must be a JSON object, got {} ({}). \
             serde_json's `Value::get` returns None for arrays, nulls and scalars \
             alike, so an `.is_none()` check on a child key passes vacuously here -- \
             that is the fed-77afac review finding this function exists to close.",
            json_kind(delegated),
            preview(delegated)
        )
    })?;

    require_non_empty_string(map, "thought_id")?;
    require_non_empty_string(map, "embedding_model")?;
    require_positive_u64(map, "embedding_dim")?;

    Ok(map)
}

/// The persisted thought's id, from a structurally valid response.
pub fn thought_id(response: &Value) -> Result<String, String> {
    let map = delegated_result(response)?;
    require_non_empty_string(map, "thought_id")
}

/// Pure, total classification of a `think` response's embedding outcome.
///
/// `Err` always means a SHAPE violation (the response does not match the
/// documented contract); `Ok` means the shape held and carries the semantic
/// outcome. Callers must never collapse the two.
pub fn embedding_outcome(response: &Value) -> Result<EmbeddingOutcome, String> {
    let map = delegated_result(response)?;

    match map.get("embedding_status") {
        // Current runner behaviour: the key is written only when the status is
        // not "complete" (src/tools/thinking/runners.rs).
        None => Ok(EmbeddingOutcome::Complete),
        Some(Value::String(s)) if s == "complete" => Ok(EmbeddingOutcome::Complete),
        Some(Value::String(s)) if DEGRADED_STATUSES.contains(&s.as_str()) => {
            Ok(EmbeddingOutcome::Degraded(s.clone()))
        }
        Some(other) => Err(format!(
            "`delegated_result.embedding_status` is present but is not one of \
             [\"complete\", \"pending\", \"failed\"]: got {} ({}). A null here is the \
             specific shape that satisfied the old `.is_some()` network-mode \
             assertion while carrying no status at all.",
            json_kind(other),
            preview(other)
        )),
    }
}

/// Offline (FakeEmbedder) expectation: the embedding completed.
///
/// Returns the persisted `thought_id` so the caller can go on to assert the
/// genuine positive witness against the database.
///
/// # Panics
/// On any shape violation, or if the response reports a degraded embedding.
pub fn assert_embedding_complete_value(response: &Value) -> String {
    match embedding_outcome(response) {
        Ok(EmbeddingOutcome::Complete) => {
            thought_id(response).expect("shape already validated by embedding_outcome")
        }
        Ok(EmbeddingOutcome::Degraded(status)) => panic!(
            "expected the offline embedder to complete, but embedding_status={status:?} \
             (full response: {response})"
        ),
        Err(shape) => panic!("think response shape violation: {shape}\nfull response: {response}"),
    }
}

/// `--allow-network` expectation: that path deliberately uses an invalid API
/// key (`OPENAI_API_KEY=sk-fake-testdb`), so the point is proving the handler
/// degrades gracefully -- `embedding_status` must be present AND carry a real
/// degraded value, not merely exist.
///
/// # Panics
/// On any shape violation, or if the response reports a completed embedding.
pub fn assert_embedding_degraded_value(response: &Value) -> String {
    match embedding_outcome(response) {
        Ok(EmbeddingOutcome::Degraded(_)) => {
            thought_id(response).expect("shape already validated by embedding_outcome")
        }
        Ok(EmbeddingOutcome::Complete) => panic!(
            "--allow-network uses an intentionally invalid API key and expects graceful \
             degradation (embedding_status one of {DEGRADED_STATUSES:?} under \
             delegated_result), but the response reported completion -- did this somehow \
             reach a VALID OpenAI key? (full response: {response})"
        ),
        Err(shape) => panic!("think response shape violation: {shape}\nfull response: {response}"),
    }
}

/// Convenience for the two DB-backed test files: pick the right expectation
/// for the mode under test.
pub fn assert_embedding_for_mode(response: &Value, network_mode: bool) -> String {
    if network_mode {
        assert_embedding_degraded_value(response)
    } else {
        assert_embedding_complete_value(response)
    }
}

// ==========================================================================
// DB-backed section: the genuine POSITIVE witness.
//
// Everything above is a pure function over the response envelope. Nothing in
// that envelope proves a vector was actually stored -- `embedding_dim` is
// `self.embedder.dimensions()`, a static property of the configured embedder,
// and success is signalled by the ABSENCE of `embedding_status`. So the
// offline tests also read the persisted row and assert a positive fact.
//
// These live here rather than in one of the two DB-backed test files so there
// is exactly one implementation. `tests/embedding_shape.rs` includes this
// module too but never calls into this section (hence the module-level
// `allow(dead_code)`); it needs no database to exercise the pure half.
// ==========================================================================

use rmcp::model::{CallToolResult, ContentBlock};
use surreal_mind::server::SurrealMindServer;

/// Decode the single JSON payload a `think` call returns.
///
/// Hard assertions throughout: this response is produced deterministically, so
/// a missing content block or unparseable text is a real regression, never an
/// expected condition to tolerate.
pub fn think_response_json(result: &CallToolResult) -> Value {
    let first = result
        .content
        .first()
        .expect("think response should have at least one content block");
    let ContentBlock::Text(text_content) = first else {
        panic!("expected a text content block in think response, got {first:?}");
    };
    serde_json::from_str(&text_content.text).expect("think response text should be valid JSON")
}

/// POSITIVE WITNESS of a completed embedding: the persisted row must carry
/// `embedding_status = 'complete'` AND a stored vector of exactly the
/// embedder's dimensionality.
///
/// Same shape of check the write path itself performs
/// (`src/tools/thinking.rs`'s `write_verified`), applied independently from
/// the test side, against the row rather than against the response.
pub async fn assert_persisted_embedding_complete(server: &SurrealMindServer, thought_id: &str) {
    let expected_dims = server.get_embedding_metadata().2;
    let mut response = server
        .db
        .query(
            "SELECT embedding_status, array::len(embedding) AS embedding_len \
             FROM type::record('thoughts', $id);",
        )
        .bind(("id", thought_id.to_string()))
        .await
        .expect("querying the persisted thought should succeed");
    // take(0), not just `query().await`, so per-statement errors surface.
    let rows: Vec<Value> = response
        .take(0)
        .expect("SELECT on the persisted thought should decode");
    let row = rows.first().unwrap_or_else(|| {
        panic!("think reported thought_id {thought_id} but no such row was persisted")
    });

    assert_eq!(
        row.get("embedding_status").and_then(|v| v.as_str()),
        Some("complete"),
        "persisted thought {thought_id} should carry embedding_status='complete' (row: {row})"
    );
    assert_eq!(
        row.get("embedding_len").and_then(|v| v.as_i64()),
        Some(expected_dims),
        "persisted thought {thought_id} should carry a stored vector of exactly \
         {expected_dims} dimensions (row: {row})"
    );
}

/// Degraded-path counterpart: graceful degradation means the thought is still
/// SAVED, so the row must exist and its status must agree with the response.
pub async fn assert_persisted_embedding_degraded(server: &SurrealMindServer, thought_id: &str) {
    let mut response = server
        .db
        .query("SELECT embedding_status FROM type::record('thoughts', $id);")
        .bind(("id", thought_id.to_string()))
        .await
        .expect("querying the persisted thought should succeed");
    let rows: Vec<Value> = response
        .take(0)
        .expect("SELECT on the persisted thought should decode");
    let row = rows.first().unwrap_or_else(|| {
        panic!(
            "graceful degradation means the thought is still SAVED, but no row was \
             persisted for thought_id {thought_id}"
        )
    });
    let status = row.get("embedding_status").and_then(|v| v.as_str());
    assert!(
        matches!(status, Some("pending") | Some("failed")),
        "persisted thought {thought_id} should carry a degraded embedding_status \
         (pending/failed), got {status:?} (row: {row})"
    );
}

/// Response-shape assertion plus the persisted-row witness, for whichever mode
/// is under test. Returns the persisted `thought_id`.
pub async fn assert_embedding_outcome_persisted(
    server: &SurrealMindServer,
    response: &Value,
    network_mode: bool,
) -> String {
    let thought_id = assert_embedding_for_mode(response, network_mode);
    if network_mode {
        assert_persisted_embedding_degraded(server, &thought_id).await;
    } else {
        assert_persisted_embedding_complete(server, &thought_id).await;
    }
    thought_id
}

/// Convenience for `tests/mcp_integration.rs`, which holds a `CallToolResult`
/// rather than an already-decoded `Value`.
pub async fn assert_think_result(
    server: &SurrealMindServer,
    result: &CallToolResult,
    network_mode: bool,
) -> String {
    let parsed = think_response_json(result);
    assert_embedding_outcome_persisted(server, &parsed, network_mode).await
}
