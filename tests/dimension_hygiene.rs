#![cfg(feature = "db_integration")]

use anyhow::Result;
use surreal_mind::{
    config::Config,
    embeddings::{create_embedder, ensure_generated_embedding_dimension},
};

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

    // OpenAI dims
    assert!(
        ensure_generated_embedding_dimension(&v1, 1536).is_ok(),
        "1536-dim vector should validate for OpenAI"
    );
    assert!(
        ensure_generated_embedding_dimension(&v2, 1536).is_err(),
        "384-dim vector should not validate for OpenAI"
    );
    assert!(
        ensure_generated_embedding_dimension(&v3, 1536).is_err(),
        "768-dim vector should not validate for OpenAI"
    );

    // BGE dims
    assert!(
        ensure_generated_embedding_dimension(&v2, 384).is_ok(),
        "384-dim vector should validate for BGE"
    );
    assert!(
        ensure_generated_embedding_dimension(&v1, 384).is_err(),
        "1536-dim vector should not validate for BGE"
    );
    assert!(
        ensure_generated_embedding_dimension(&v3, 384).is_err(),
        "768-dim vector should not validate for BGE"
    );
}

/// Startup must reject a genuinely stale HNSW dimension unless the documented
/// emergency bypass is set *before* `SurrealMindServer::new()` invokes schema
/// verification. This test only runs under the existing explicit disposable
/// namespace opt-in because it deliberately replaces the scratch index.
#[tokio::test]
async fn test_schema_dimension_mismatch_requires_or_honors_emergency_bypass() -> Result<()> {
    if std::env::var("RUN_DB_TESTS").is_err()
        || std::env::var("REEMBED_TEST_CONFIRM_DISPOSABLE_NS").is_err()
    {
        return Ok(());
    }

    let config = Config::load()?;
    assert!(
        !matches!(
            config.system.database_ns.as_str(),
            "surreal_mind" | "surreal-mind"
        ),
        "schema bypass test requires a disposable namespace, not {:?}",
        config.system.database_ns
    );

    // Bootstrap the disposable schema at its normal dimension, then replace
    // just this index with a real wrong-dimension definition.
    unsafe {
        std::env::remove_var("SURR_SKIP_DIM_CHECK");
    }
    let bootstrap = surreal_mind::server::SurrealMindServer::new(&config).await?;
    bootstrap
        .db
        .query(
            "REMOVE INDEX thoughts_embedding_idx ON TABLE thoughts; \
             DEFINE INDEX thoughts_embedding_idx ON TABLE thoughts FIELDS embedding HNSW DIMENSION 1;",
        )
        .await?
        .check()?;

    let normal_error = match surreal_mind::server::SurrealMindServer::new(&config).await {
        Ok(_) => anyhow::bail!("normal startup accepted a stale thoughts_embedding_idx dimension"),
        Err(error) => error,
    };
    assert!(
        normal_error.to_string().contains("thoughts_embedding_idx"),
        "normal startup must fail specifically on the stale index dimension: {normal_error}"
    );

    unsafe {
        std::env::set_var("SURR_SKIP_DIM_CHECK", "1");
    }
    let bypassed = surreal_mind::server::SurrealMindServer::new(&config).await;
    unsafe {
        std::env::remove_var("SURR_SKIP_DIM_CHECK");
    }
    let bypassed = match bypassed {
        Ok(server) => server,
        Err(error) => anyhow::bail!(
            "SURR_SKIP_DIM_CHECK must bypass only the known index-dimension verification: {error}"
        ),
    };

    // Restore the normal fixture index so this test is safe to run in the
    // same single-threaded disposable suite as the other hygiene tests.
    bypassed
        .db
        .query(format!(
            "REMOVE INDEX thoughts_embedding_idx ON TABLE thoughts; \
             DEFINE INDEX thoughts_embedding_idx ON TABLE thoughts FIELDS embedding HNSW DIMENSION {};",
            config.system.embedding_dimensions
        ))
        .await?
        .check()?;

    Ok(())
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

/// N-5-class regression guard for `run_kg_embed` (the missing-only KG
/// embedder used by the `kgembed` CLI shortcut). Before this fix, the
/// idempotent per-row UPDATE used `RETURN NONE`, giving nothing to verify a
/// zero-row match against, so every attempted row was counted as updated
/// regardless of whether the UPDATE actually touched it. This creates one
/// synthetic `kg_entities` scratch row with `embedding = NONE`, runs the
/// real `run_kg_embed` end-to-end, and asserts both that the row's
/// embedding was actually written (not just that the counter incremented)
/// and that `entities_no_match` is 0 for this freshly-created, definitely-
/// matching row. Same whole-table-scan safety gate as
/// `test_run_reembed_actually_updates_matched_row` above: `run_kg_embed`
/// walks the WHOLE `kg_entities`/`kg_observations`/`kg_edges` tables in the
/// configured namespace, not just this test's scratch row.
#[tokio::test]
async fn test_kg_embed_actually_updates_matched_row() -> Result<()> {
    if std::env::var("RUN_DB_TESTS").is_err() {
        return Ok(());
    }

    let config = Config::load()?;
    let confirmed_disposable = std::env::var("REEMBED_TEST_CONFIRM_DISPOSABLE_NS").is_ok();
    let looks_like_production_ns = matches!(
        config.system.database_ns.as_str(),
        "surreal_mind" | "surreal-mind"
    );
    if !confirmed_disposable || looks_like_production_ns {
        eprintln!(
            "Skipping test_kg_embed_actually_updates_matched_row: set \
             REEMBED_TEST_CONFIRM_DISPOSABLE_NS=1 and point SURR_DB_NS/SURR_DB_DB at a \
             genuinely disposable namespace (current ns={:?}). This test calls run_kg_embed, \
             which walks the WHOLE kg_entities/kg_observations/kg_edges tables in the \
             configured namespace, not just its own scratch row.",
            config.system.database_ns
        );
        return Ok(());
    }

    let marker = "__rmcp-followup-kgembed-falsesuccess-regression__";
    let server = surreal_mind::server::SurrealMindServer::new(&config).await?;
    server
        .db
        .query("CREATE kg_entities SET name = $name, entity_type = 'test', data = {}, embedding = NONE")
        .bind(("name", marker.to_string()))
        .await?
        .check()?;

    let stats = surreal_mind::run_kg_embed(Some(1), false).await?;

    let after: Vec<serde_json::Value> = server
        .db
        .query(
            "SELECT (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) AS elen \
             FROM kg_entities WHERE name = $name",
        )
        .bind(("name", marker.to_string()))
        .await?
        .take(0)?;
    let elen = after
        .first()
        .and_then(|r| r.get("elen"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert!(
        elen > 0,
        "the scratch kg_entities row's embedding must actually be set after run_kg_embed; \
         elen={} (pre-fix RETURN NONE gave nothing to verify a 0-row match against, so a \
         never-written row could still be counted as updated)",
        elen
    );
    assert_eq!(
        stats.entities_no_match, 0,
        "a freshly-created scratch row must match the idempotent UPDATE's WHERE clause \
         (embedding IS NULL/NONE); a no_match here means the RETURN-verification wiring \
         itself is broken, not the row. Full stats: {:?}",
        stats
    );

    server
        .db
        .query("DELETE kg_entities WHERE name = $name")
        .bind(("name", marker.to_string()))
        .await?
        .check()?;

    Ok(())
}

/// N-5-class regression guard for `run_reembed_kg` (the `maintain(action:
/// "reembed_kg")` MCP tool and `reembed_kg` CLI binary, which re-embeds
/// EVERY kg_entities/kg_observations/kg_edges row regardless of current
/// embedding state). Before this fix, all three tables' per-row UPDATEs
/// called `.await?` with no `.take()`/verification and incremented their
/// `*_updated` counter unconditionally. This creates one synthetic
/// `kg_entities` scratch row, runs the real `run_reembed_kg` end-to-end
/// scoped with `limit=1` (bounding the whole-table scan to the first row
/// found), and asserts the row's embedding was actually written and that
/// `entities_no_match` is 0. Same whole-table-scan safety gate as the two
/// tests above.
#[tokio::test]
async fn test_reembed_kg_actually_updates_matched_row() -> Result<()> {
    if std::env::var("RUN_DB_TESTS").is_err() {
        return Ok(());
    }

    let config = Config::load()?;
    let confirmed_disposable = std::env::var("REEMBED_TEST_CONFIRM_DISPOSABLE_NS").is_ok();
    let looks_like_production_ns = matches!(
        config.system.database_ns.as_str(),
        "surreal_mind" | "surreal-mind"
    );
    if !confirmed_disposable || looks_like_production_ns {
        eprintln!(
            "Skipping test_reembed_kg_actually_updates_matched_row: set \
             REEMBED_TEST_CONFIRM_DISPOSABLE_NS=1 and point SURR_DB_NS/SURR_DB_DB at a \
             genuinely disposable namespace (current ns={:?}). This test calls \
             run_reembed_kg, which walks the WHOLE kg_entities table in the configured \
             namespace, not just its own scratch row.",
            config.system.database_ns
        );
        return Ok(());
    }

    let marker = "__rmcp-followup-reembedkg-falsesuccess-regression__";
    let server = surreal_mind::server::SurrealMindServer::new(&config).await?;
    server
        .db
        .query("CREATE kg_entities SET name = $name, entity_type = 'test', data = {}, embedding = NONE")
        .bind(("name", marker.to_string()))
        .await?
        .check()?;

    let stats = surreal_mind::run_reembed_kg(Some(1), false).await?;

    let after: Vec<serde_json::Value> = server
        .db
        .query(
            "SELECT (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) AS elen \
             FROM kg_entities WHERE name = $name",
        )
        .bind(("name", marker.to_string()))
        .await?
        .take(0)?;
    let elen = after
        .first()
        .and_then(|r| r.get("elen"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert!(
        elen > 0,
        "the scratch kg_entities row's embedding must actually be set after run_reembed_kg; \
         elen={} (pre-fix: the UPDATE result was never checked, so a never-written row could \
         still be counted as updated)",
        elen
    );
    assert_eq!(
        stats.entities_no_match, 0,
        "a freshly-created scratch row's direct-by-id UPDATE should match; a no_match here \
         means the RETURN-verification wiring itself is broken, not the row. Full stats: {:?}",
        stats
    );

    server
        .db
        .query("DELETE kg_entities WHERE name = $name")
        .bind(("name", marker.to_string()))
        .await?
        .check()?;

    Ok(())
}
