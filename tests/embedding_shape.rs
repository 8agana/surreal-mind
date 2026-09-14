//! Negative controls for the `think` response-shape assertions (clu
//! fed-77afac, review round 2).
//!
//! DELIBERATELY UNGATED: no `db_integration`, no `test-embedder`, no database,
//! no network. `cargo test` alone runs every case below. That is the whole
//! point -- the hole the reviewer found was a SHAPE regression, and a shape
//! regression is only demonstrable by feeding the degenerate shapes in
//! directly. Breaking the embedder does not exercise it, because the failure
//! path really does emit `embedding_status`; the shapes below are the ones
//! that carry no status at all and used to pass anyway.
//!
//! Every `assert!(... .is_err())` here is a case that PASSED the assertions
//! shipped in c5b8ac9.

mod common;

use common::{
    EmbeddingOutcome, assert_embedding_complete_value, assert_embedding_degraded_value,
    embedding_outcome, thought_id,
};
use serde_json::{Value, json};

/// A structurally valid `delegated_result` payload, matching what
/// `run_convo`/`run_technical` actually build (src/tools/thinking/runners.rs).
fn valid_delegated() -> Value {
    json!({
        "thought_id": "11111111-2222-3333-4444-555555555555",
        "embedding_model": "fake-test-embedder",
        "embedding_dim": 1536,
        "memories_injected": 0,
        "enriched_content": Value::Null,
        "framework_enhanced": Value::Null
    })
}

fn envelope(delegated: Value) -> Value {
    json!({
        "mode_selected": "question",
        "reason": "test",
        "delegated_result": delegated,
        "links": { "session_id": Value::Null },
        "telemetry": {}
    })
}

/// Same payload, with an `embedding_status` of the caller's choosing.
fn envelope_with_status(status: Value) -> Value {
    let mut delegated = valid_delegated();
    delegated["embedding_status"] = status;
    envelope(delegated)
}

// --------------------------------------------------------------------------
// Happy paths -- these must keep working, or the negative controls below are
// just asserting that everything fails.
// --------------------------------------------------------------------------

#[test]
fn complete_when_status_key_absent() {
    let response = envelope(valid_delegated());
    assert_eq!(
        embedding_outcome(&response),
        Ok(EmbeddingOutcome::Complete),
        "the current runner omits embedding_status on success"
    );
    assert_eq!(
        assert_embedding_complete_value(&response),
        "11111111-2222-3333-4444-555555555555"
    );
}

#[test]
fn complete_when_status_explicitly_complete() {
    let response = envelope_with_status(json!("complete"));
    assert_eq!(
        embedding_outcome(&response),
        Ok(EmbeddingOutcome::Complete),
        "an explicit success marker must not read as a shape violation"
    );
}

#[test]
fn degraded_for_each_known_failure_status() {
    for status in ["pending", "failed"] {
        let response = envelope_with_status(json!(status));
        assert_eq!(
            embedding_outcome(&response),
            Ok(EmbeddingOutcome::Degraded(status.to_string())),
            "{status} must classify as degraded"
        );
        assert_eq!(
            assert_embedding_degraded_value(&response),
            "11111111-2222-3333-4444-555555555555"
        );
    }
}

#[test]
fn thought_id_extracted_from_valid_response() {
    let response = envelope(valid_delegated());
    assert_eq!(
        thought_id(&response).as_deref(),
        Ok("11111111-2222-3333-4444-555555555555")
    );
}

// --------------------------------------------------------------------------
// NEGATIVE CONTROLS -- the degenerate `delegated_result` shapes.
//
// `serde_json::Value::get` returns None on ALL of these, so every one of them
// satisfied `delegated.get("embedding_status").is_none()` and passed the
// offline "embedding completed" assertion shipped in c5b8ac9.
// --------------------------------------------------------------------------

#[test]
fn rejects_empty_object_delegated_result() {
    let err = embedding_outcome(&envelope(json!({})))
        .expect_err("`delegated_result: {}` must not read as a completed embedding");
    assert!(
        err.contains("thought_id"),
        "error should name the missing witness key, got: {err}"
    );
}

#[test]
fn rejects_array_delegated_result() {
    let err = embedding_outcome(&envelope(json!([])))
        .expect_err("`delegated_result: []` must not read as a completed embedding");
    assert!(
        err.contains("must be a JSON object"),
        "error should name the type violation, got: {err}"
    );
}

#[test]
fn rejects_null_delegated_result() {
    let err = embedding_outcome(&envelope(Value::Null))
        .expect_err("`delegated_result: null` must not read as a completed embedding");
    assert!(
        err.contains("must be a JSON object"),
        "error should name the type violation, got: {err}"
    );
}

#[test]
fn rejects_scalar_delegated_result() {
    for scalar in [json!(42), json!("nope"), json!(true), json!(1.5)] {
        let err = embedding_outcome(&envelope(scalar.clone())).expect_err(&format!(
            "`delegated_result: {scalar}` must not read as a completed embedding"
        ));
        assert!(
            err.contains("must be a JSON object"),
            "error should name the type violation for {scalar}, got: {err}"
        );
    }
}

#[test]
fn rejects_missing_delegated_result() {
    let response = json!({ "mode_selected": "question", "links": {} });
    let err = embedding_outcome(&response).expect_err("a missing container must not pass");
    assert!(
        err.contains("missing `delegated_result`"),
        "error should name the missing container, got: {err}"
    );
}

#[test]
fn rejects_non_object_envelope() {
    for bogus in [json!([]), json!("{}"), Value::Null, json!(7)] {
        assert!(
            embedding_outcome(&bogus).is_err(),
            "a non-object think response must not pass: {bogus}"
        );
    }
}

#[test]
fn rejects_partial_runner_payload() {
    // A `delegated_result` that is an object but not a runner payload: the
    // object check alone is not enough, which is why the witness keys are
    // required too.
    let cases = [
        json!({ "embedding_model": "m", "embedding_dim": 8 }), // no thought_id
        json!({ "thought_id": "", "embedding_model": "m", "embedding_dim": 8 }), // blank id
        json!({ "thought_id": "t", "embedding_dim": 8 }),      // no model
        json!({ "thought_id": "t", "embedding_model": "m" }),  // no dim
        json!({ "thought_id": "t", "embedding_model": "m", "embedding_dim": 0 }), // zero dim
        json!({ "thought_id": "t", "embedding_model": "m", "embedding_dim": "8" }), // dim as string
        json!({ "thought_id": 8, "embedding_model": "m", "embedding_dim": 8 }), // id not a string
    ];
    for case in cases {
        assert!(
            embedding_outcome(&envelope(case.clone())).is_err(),
            "partial runner payload must not pass: {case}"
        );
    }
}

// --------------------------------------------------------------------------
// NEGATIVE CONTROLS -- the status VALUE, i.e. the network-mode hole.
//
// `{"embedding_status": null}` satisfied `.is_some()` and passed the
// `--allow-network` graceful-degradation assertion shipped in c5b8ac9 while
// carrying no status at all.
// --------------------------------------------------------------------------

#[test]
fn rejects_null_embedding_status() {
    let err = embedding_outcome(&envelope_with_status(Value::Null))
        .expect_err("a null embedding_status must not read as a real degraded status");
    assert!(
        err.contains("embedding_status"),
        "error should name the offending key, got: {err}"
    );
}

#[test]
fn rejects_non_string_embedding_status() {
    for bogus in [json!(0), json!(true), json!([]), json!({})] {
        assert!(
            embedding_outcome(&envelope_with_status(bogus.clone())).is_err(),
            "non-string embedding_status must not pass: {bogus}"
        );
    }
}

#[test]
fn rejects_unknown_embedding_status_string() {
    for bogus in ["", "COMPLETE", "degraded", "ok", "unknown"] {
        assert!(
            embedding_outcome(&envelope_with_status(json!(bogus))).is_err(),
            "unrecognised embedding_status must not pass: {bogus:?}"
        );
    }
}

// --------------------------------------------------------------------------
// The panicking wrappers the DB-backed tests actually call must fail on the
// same inputs -- a correct predicate wired to a wrapper that swallows the
// error would be no better than the hole it replaced.
// --------------------------------------------------------------------------

#[test]
#[should_panic(expected = "shape violation")]
fn complete_wrapper_panics_on_empty_object() {
    assert_embedding_complete_value(&envelope(json!({})));
}

#[test]
#[should_panic(expected = "shape violation")]
fn complete_wrapper_panics_on_null_container() {
    assert_embedding_complete_value(&envelope(Value::Null));
}

#[test]
#[should_panic(expected = "shape violation")]
fn complete_wrapper_panics_on_array_container() {
    assert_embedding_complete_value(&envelope(json!([])));
}

#[test]
#[should_panic(expected = "shape violation")]
fn complete_wrapper_panics_on_scalar_container() {
    assert_embedding_complete_value(&envelope(json!(42)));
}

#[test]
#[should_panic(expected = "embedding_status")]
fn complete_wrapper_panics_on_degraded_status() {
    assert_embedding_complete_value(&envelope_with_status(json!("failed")));
}

#[test]
#[should_panic(expected = "shape violation")]
fn degraded_wrapper_panics_on_null_status() {
    assert_embedding_degraded_value(&envelope_with_status(Value::Null));
}

#[test]
#[should_panic(expected = "shape violation")]
fn degraded_wrapper_panics_on_empty_object() {
    assert_embedding_degraded_value(&envelope(json!({})));
}

#[test]
#[should_panic(expected = "graceful")]
fn degraded_wrapper_panics_when_embedding_completed() {
    assert_embedding_degraded_value(&envelope(valid_delegated()));
}
