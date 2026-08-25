#![cfg(feature = "db_integration")]

use anyhow::Result;
use surreal_mind::{config::Config, embeddings::create_embedder};

/// Test that dimension hygiene is maintained in the database
#[tokio::test]
async fn test_dimension_hygiene_check() -> Result<()> {
    // Only run if RUN_DB_TESTS is set
    if std::env::var("RUN_DB_TESTS").is_err() {
        return Ok(());
    }

    let config = Config::load()?;
    let server = surreal_mind::server::SurrealMindServer::new(&config).await?;

    // Validate via check_embedding_dims (behavior assertion)
    // This will return an error if dimensions are mismatched
    // The function returns Ok(()) if all dimensions match, or an error with details if they don't
    match server.check_embedding_dims().await {
        Ok(()) => {
            // All embedding dimensions are consistent
        }
        Err(e) => {
            // This means there are mismatched dimensions
            panic!("Embedding dimension mismatch detected: {}", e);
        }
    }

    Ok(())
}

/// Test vector dimension validation
#[test]
fn test_vector_dimension_validation() {
    // Mock vectors of different sizes
    let v1: Vec<f32> = vec![0.0; 1536]; // OpenAI size
    let v2: Vec<f32> = vec![0.0; 384]; // BGE size
    let v3: Vec<f32> = vec![0.0; 768]; // Wrong size

    // Test dimension validation helper
    fn validate_dims(vec: &[f32], expected: usize) -> bool {
        vec.len() == expected
    }

    // OpenAI dims
    assert!(
        validate_dims(&v1, 1536),
        "1536-dim vector should validate for OpenAI"
    );
    assert!(
        !validate_dims(&v2, 1536),
        "384-dim vector should not validate for OpenAI"
    );
    assert!(
        !validate_dims(&v3, 1536),
        "768-dim vector should not validate for OpenAI"
    );

    // BGE dims
    assert!(
        validate_dims(&v2, 384),
        "384-dim vector should validate for BGE"
    );
    assert!(
        !validate_dims(&v1, 384),
        "1536-dim vector should not validate for BGE"
    );
    assert!(
        !validate_dims(&v3, 384),
        "768-dim vector should not validate for BGE"
    );
}

/// Test that reembed reports dimension mismatches accurately
#[tokio::test]
async fn test_reembed_mismatch_reporting() -> Result<()> {
    // Only run if RUN_DB_TESTS is set
    if std::env::var("RUN_DB_TESTS").is_err() {
        return Ok(());
    }

    let config = Config::load()?;
    let embedder = create_embedder(&config).await?;
    let expected_dims = embedder.dimensions();

    // Mock thought with a dimension-metadata mismatch: `embedding_dim`
    // (metadata) disagrees with the real length of `embedding`. This does
    // NOT set the `embedding` array itself to the wrong length — measured
    // live during this pass, `thoughts_embedding_idx` (HNSW DIMENSION
    // {expected_dims}) rejects any write whose `embedding` array length
    // doesn't match the index's configured dimension outright ("Incorrect
    // vector dimension"), so a genuinely wrong-length vector can never be
    // persisted at all once that index exists — a stronger, DB-enforced
    // backstop than the M-1 application-level fix alone (which is still
    // correct: it avoids the wasted round-trip and doesn't assume the index
    // will always exist in this exact shape). What CAN legitimately drift is
    // the `embedding_dim` metadata field itself, independent of the real
    // array length, which is exactly what this test's SELECT detects.
    //
    // Three incidental fixes discovered while adding db_integration coverage
    // for N-1/N-5 in this same pass (all were previously masking each other
    // via the N-2 pattern: `.query().await?` alone never surfaces a
    // per-statement error, so this CREATE was silently failing on every
    // prior run and the test only ever "passed" against pre-existing
    // pollution in whatever namespace it ran in):
    //   1. `array::range(start, count, step)` (3 args) does not exist in
    //      SurrealDB 3.1.2 — `array::range(start, count)` (2 args) is the
    //      live signature, and (measured live) `array::range(a, b)` returns
    //      `b - a` elements (i.e. `[a, a+1, ..., b-1]`), not `b` elements —
    //      so an exactly-`dims`-length array needs `array::range(0, dims)`,
    //      not `array::range(1, dims)` (which is one short).
    //   2. `thoughts` is SCHEMAFULL with several TYPE (non-`option`) fields
    //      that carry no DEFAULT — injected_memories, injection_scale,
    //      significance, access_count, created_at — so SurrealDB 3.1.2
    //      rejects a CREATE that omits them rather than silently defaulting.
    //   3. `embedding` must actually satisfy the live HNSW index's
    //      DIMENSION (see above) — the original 3-arg call additionally
    //      tried to write a wrong-length array, which would fail this check
    //      even after fixing the arg count.
    let query = format!(
        r#"CREATE thoughts SET
           content = "test content",
           embedding = array::range(0, {}),
           embedding_dim = {},
           embedding_model = "wrong_model",
           created_at = time::now(),
           injected_memories = [],
           injection_scale = 0,
           significance = 0.0,
           access_count = 0
        "#,
        expected_dims,
        expected_dims + 100
    );

    let server = surreal_mind::server::SurrealMindServer::new(&config).await?;
    server.db.query(query).await?.check()?;

    // Run reembed stats query
    let stats: Vec<serde_json::Value> = server
        .db
        .query(
            "SELECT embedding_dim as dim, count() as count
             FROM thoughts
             WHERE embedding_dim != $expected
             GROUP BY embedding_dim",
        )
        .bind(("expected", expected_dims as i64))
        .await?
        .take(0)?;

    // Verify mismatches were detected
    assert!(!stats.is_empty(), "Should detect dimension mismatches");
    let first_mismatch = &stats[0];
    let mismatched_dim = first_mismatch["dim"].as_i64().unwrap();
    assert_ne!(
        mismatched_dim as usize, expected_dims,
        "Should identify wrong dimension"
    );

    // Cleanup test data
    server
        .db
        .query("DELETE thoughts WHERE embedding_dim > $dims")
        .bind(("dims", expected_dims as i64))
        .await?;

    Ok(())
}

/// N-1 regression guard: SurrealDB 3.x's zero-arg `count()` returns a
/// hardcoded 1 per matched row unless the query carries `GROUP ALL`. Before
/// the fix, `maintenance.rs`'s embed_pending "remaining" count and
/// `admin.rs`'s dimension-fix verification both used the no-GROUP-ALL form,
/// so N matching rows reported as N separate `{cnt:1}` rows instead of one
/// `{cnt:N}` — reading `.first()` off that array (as both call sites do)
/// silently truncates the true count to 1 whenever more than one row
/// matches. This test reproduces the exact predicate from
/// maintenance.rs's embed_pending remaining-count query against 3 synthetic
/// scratch rows and asserts the documented asymmetry directly, self-cleaning
/// via a marker unlikely to collide with real content.
#[tokio::test]
async fn test_group_all_required_for_accurate_count() -> Result<()> {
    if std::env::var("RUN_DB_TESTS").is_err() {
        return Ok(());
    }

    let config = Config::load()?;
    let server = surreal_mind::server::SurrealMindServer::new(&config).await?;
    let marker = "__rmcp-followup-N1-count-regression__";

    // Scratch rows: 3 thoughts with embedding_status in ['pending','failed'],
    // scoped by a synthetic content marker so cleanup can never touch real data.
    for i in 0..3 {
        let status = if i == 0 { "pending" } else { "failed" };
        server
            .db
            .query(
                "CREATE thoughts SET content = $content, embedding_status = $status, \
                 created_at = time::now(), injected_memories = [], injection_scale = 0, \
                 significance = 0.0, access_count = 0",
            )
            .bind(("content", format!("{} row {}", marker, i)))
            .bind(("status", status))
            .await?
            .check()?;
    }

    // Pre-fix shape: no GROUP ALL. Must reproduce the documented bug —
    // 3 separate {cnt:1} rows, not one {cnt:3}.
    let no_group_all: Vec<serde_json::Value> = server
        .db
        .query("SELECT count() AS cnt FROM thoughts WHERE content CONTAINS $marker")
        .bind(("marker", marker))
        .await?
        .take(0)?;
    assert_eq!(
        no_group_all.len(),
        3,
        "without GROUP ALL, count() must return one row per match (the bug being guarded against)"
    );
    for row in &no_group_all {
        assert_eq!(row.get("cnt").and_then(|v| v.as_i64()), Some(1));
    }

    // Post-fix shape: with GROUP ALL, exactly one row carrying the true total.
    let with_group_all: Vec<serde_json::Value> = server
        .db
        .query("SELECT count() AS cnt FROM thoughts WHERE content CONTAINS $marker GROUP ALL")
        .bind(("marker", marker))
        .await?
        .take(0)?;
    assert_eq!(
        with_group_all.len(),
        1,
        "with GROUP ALL, count() must collapse to a single row"
    );
    assert_eq!(
        with_group_all
            .first()
            .and_then(|r| r.get("cnt"))
            .and_then(|v| v.as_i64()),
        Some(3),
        "GROUP ALL must report the true total across all matching rows"
    );

    // Cleanup: only the synthetically marked scratch rows.
    server
        .db
        .query("DELETE thoughts WHERE content CONTAINS $marker")
        .bind(("marker", marker))
        .await?
        .check()?;

    Ok(())
}

/// N-5 regression guard, end-to-end: before the fix, `run_reembed`'s per-row
/// UPDATE compared a bare string against a typed record id
/// (`WHERE id = '<key>'`) and never matched a real row (measured live: 0 of 1
/// affected). This creates one synthetic scratch thought carrying a marker
/// `embedding_model` value, runs the real `run_reembed` end-to-end (not a
/// query poke, and forced via `missing_only=false` so the UPDATE is
/// attempted regardless of the embedding's current length), and asserts the
/// row's `embedding_model` actually changed away from the marker afterward —
/// which fails against the pre-fix `WHERE id = '{}'` clause (the row is
/// never touched, so the marker survives) and passes against the
/// `type::record('thoughts', ...)` fix. Uses a real, index-valid-length
/// embedding for the CREATE (see the comment on
/// `test_reembed_mismatch_reporting` above: `thoughts_embedding_idx`'s live
/// HNSW DIMENSION rejects any write of a genuinely wrong-length vector, so
/// this test cannot and does not attempt one) — the identity bug under test
/// lives entirely in the UPDATE's WHERE clause, not in dimension handling.
#[tokio::test]
async fn test_run_reembed_actually_updates_matched_row() -> Result<()> {
    if std::env::var("RUN_DB_TESTS").is_err() {
        return Ok(());
    }

    let config = Config::load()?;

    // Hard safety gate: run_reembed operates over the WHOLE `thoughts` table
    // in its configured namespace/database, not just this test's scratch
    // row — unlike every other test in this file, it is not scoped by a
    // marker. Running it against the shared production namespace would
    // write to real data, which this task's hard boundary forbids even
    // under RUN_DB_TESTS. Require a second, explicit opt-in plus a
    // namespace name that cannot be the known production namespace.
    let confirmed_disposable = std::env::var("REEMBED_TEST_CONFIRM_DISPOSABLE_NS").is_ok();
    let looks_like_production_ns = matches!(
        config.system.database_ns.as_str(),
        "surreal_mind" | "surreal-mind"
    );
    if !confirmed_disposable || looks_like_production_ns {
        eprintln!(
            "Skipping test_run_reembed_actually_updates_matched_row: set \
             REEMBED_TEST_CONFIRM_DISPOSABLE_NS=1 and point SURR_DB_NS/SURR_DB_DB at a \
             genuinely disposable namespace (current ns={:?}). This test calls run_reembed, \
             which walks the WHOLE thoughts table in the configured namespace, not just its \
             own scratch row.",
            config.system.database_ns
        );
        return Ok(());
    }
    let embedder = create_embedder(&config).await?;
    let expected_dims = embedder.dimensions();
    let marker = "__rmcp-followup-N5-reembed-regression__";
    let stale_model_marker = "__rmcp-followup-N5-stale-model-marker__";

    let server = surreal_mind::server::SurrealMindServer::new(&config).await?;
    server
        .db
        .query(
            r#"CREATE thoughts SET
               content = $content,
               embedding = array::range(0, $dims),
               embedding_dim = $dims,
               embedding_model = $stale_model,
               embedding_status = "complete",
               created_at = time::now(),
               injected_memories = [],
               injection_scale = 0,
               significance = 0.0,
               access_count = 0
            "#,
        )
        .bind(("content", marker.to_string()))
        .bind(("dims", expected_dims as i64))
        .bind(("stale_model", stale_model_marker))
        .await?
        .check()?;

    // Run the real function under test. missing_only=false forces the UPDATE
    // to be attempted for every row `run_reembed` sees regardless of current
    // embedding length, which is what actually exercises the WHERE clause
    // under test here (the point isn't dimension correction, it's whether
    // the UPDATE matches the row at all).
    let stats = surreal_mind::run_reembed(10, Some(50), false, false).await?;
    assert!(
        stats.updated >= 1,
        "run_reembed must report at least one row updated; got {:?}",
        stats
    );

    let after: Vec<serde_json::Value> = server
        .db
        .query("SELECT embedding_model AS model FROM thoughts WHERE content = $content")
        .bind(("content", marker.to_string()))
        .await?
        .take(0)?;
    let model_after = after
        .first()
        .and_then(|r| r.get("model"))
        .and_then(|v| v.as_str());
    assert_ne!(
        model_after,
        Some(stale_model_marker),
        "the scratch row's embedding_model must no longer be the stale marker after \
         run_reembed; it still being {:?} means the UPDATE's WHERE clause matched 0 rows \
         (the exact N-5 bug: a bare string never equals a typed record id)",
        stale_model_marker
    );

    // Cleanup: only the synthetically marked scratch row.
    server
        .db
        .query("DELETE thoughts WHERE content = $content")
        .bind(("content", marker.to_string()))
        .await?
        .check()?;

    Ok(())
}
