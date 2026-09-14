#![cfg(feature = "db_integration")]

// Integration tests focused on direct handler + DB checks.
// For full MCP protocol coverage (JSON-RPC over sink/stream), see tests/mcp_protocol.rs.

use rmcp::{ServerHandler, model::CallToolRequestParams};
use serde_json::json;
use surreal_mind::{config::Config, server::SurrealMindServer};

mod common;

// Helper to create a test server instance
async fn create_test_server() -> SurrealMindServer {
    let config = Config::load().expect("Failed to load config");
    SurrealMindServer::new(&config)
        .await
        .expect("Failed to create server")
}

/// Offline-embedder test server (clu fed-77afac): wired to the
/// deterministic, zero-network `FakeEmbedder` instead of a live OpenAI
/// call.
///
/// TWO gates must both be open, and `scripts/test_db.sh` opens both:
///  1. the crate must be built with `--features test-embedder`, or
///     `create_embedder` has no "fake" arm to select at all;
///  2. `SURR_ALLOW_FAKE_EMBEDDER=1` must be set, or the "fake" arm REFUSES to
///     construct the embedder at runtime (fed-77afac review round 2 -- the
///     Cargo feature is public, so compile-time exclusion cannot be the only
///     guard).
///
/// Either gate closed fails loudly here rather than silently reaching the
/// network.
async fn create_offline_test_server() -> SurrealMindServer {
    let mut config = Config::load().expect("Failed to load config");
    config.system.embedding_provider = "fake".to_string();
    // NOTE (fed-77afac design note 6, verified against the code rather than
    // assumed): `embed_strict` does NOT gate embedding failure inside
    // `handle_legacymind_think` -- `ThoughtBuilder::execute` (src/tools/
    // thinking.rs) tolerates embedder failure unconditionally and still
    // returns Ok with embedding_status "pending"/"failed", and the only
    // consumer of `embed_strict` in this codebase is main.rs's startup
    // dimension preflight, which these tests never reach (they construct
    // SurrealMindServer directly, bypassing main()). Set here anyway
    // because it costs nothing and matches the documented intent, but the
    // REAL non-vacuous guard is `common::assert_think_result` below:
    // `result.is_ok()` alone is true on both success AND embedder failure,
    // and the response-shape half of that assertion is itself backed by the
    // persisted-row positive witness because absence-of-a-failure-marker is
    // not evidence of success.
    config.runtime.embed_strict = true;
    SurrealMindServer::new(&config).await.expect(
        "Failed to create server -- built with `--features test-embedder`, and with \
             SURR_ALLOW_FAKE_EMBEDDER=1 set? scripts/test_db.sh does both.",
    )
}

#[tokio::test]
async fn test_server_initialization() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping integration test - set RUN_DB_TESTS=1 to run");
        return;
    }

    let server = create_test_server().await;

    // Verify database is connected
    let health = server.db.health().await;
    assert!(health.is_ok(), "Database health check failed");

    // Verify server info
    let info = <SurrealMindServer as ServerHandler>::get_info(&server);
    assert_eq!(info.server_info.name.as_ref() as &str, "surreal-mind");
    let version: &str = info.server_info.version.as_ref();
    // Router (src/server/router.rs:157) sets this to exactly
    // `env!("CARGO_PKG_VERSION")` -- compare against the crate's actual
    // version instead of a hardcoded literal so this cannot silently rot
    // again the way the old `"0.1"` prefix did once the crate moved past
    // 0.1.x (it was last correct at some 0.1.x release; Cargo.toml is now
    // 0.8.2 and this assertion kept passing on a coincidental prefix match
    // it no longer meaningfully checked).
    assert_eq!(
        version,
        env!("CARGO_PKG_VERSION"),
        "server_info.version must be exactly the crate's Cargo.toml version"
    );

    // Verify embedder metadata
    let (provider, model, dims) = server.get_embedding_metadata();
    assert!(!provider.is_empty());
    assert!(!model.is_empty());
    assert!(dims > 0);
}

// Note: Cannot test handler methods directly as they require RequestContext which is pub(crate)
// The rmcp 0.6.4 API prevents external testing of protocol handlers list_tools and call_tool
// We can only test the internal handler functions and database operations

#[tokio::test]
async fn test_database_operations() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping integration test - set RUN_DB_TESTS=1 to run");
        return;
    }

    let server = create_test_server().await;

    // Test database query
    let result = server.db.query("SELECT * FROM thoughts LIMIT 1").await;
    assert!(result.is_ok(), "Database query failed");
}

#[tokio::test]
async fn test_database_schema() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping integration test - set RUN_DB_TESTS=1 to run");
        return;
    }

    let server = create_test_server().await;

    // Verify tables exist
    let tables = vec![
        "thoughts",
        "kg_entities",
        "kg_relationships",
        "kg_observations",
    ];

    for table in tables {
        let query = format!("INFO FOR TABLE {}", table);
        let result = server.db.query(&query).await;
        assert!(result.is_ok(), "Table {} should exist", table);
    }
}

// Test the internal handler functions directly (they don't require RequestContext)
#[tokio::test]
async fn test_think_handler() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping integration test - set RUN_DB_TESTS=1 to run");
        return;
    }

    // Offline embedder by default (clu fed-77afac): runs against the
    // deterministic, zero-network FakeEmbedder instead of reaching
    // api.openai.com. ALLOW_NETWORK_EMBED=1 (scripts/test_db.sh
    // --allow-network) is still honored as the explicit, loud opt-in to a
    // real (intentionally invalid-key, bound-to-degrade) network call --
    // it no longer means "skip this test entirely".
    let network_mode = std::env::var("ALLOW_NETWORK_EMBED").ok().as_deref() == Some("1");
    let server = if network_mode {
        create_test_server().await
    } else {
        create_offline_test_server().await
    };

    // Test with valid params
    let request = CallToolRequestParams::new("think").with_arguments(
        json!({
            "content": "Test thought content"
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    // Call the internal handler directly
    let result = server.handle_legacymind_think(request).await;
    assert!(result.is_ok(), "think handler should succeed");

    let result = result.unwrap();
    assert!(!result.content.is_empty(), "Should return content");
    // Response-shape assertion + the persisted-row POSITIVE witness. Both
    // live in tests/common/mod.rs, shared with tests/mcp_protocol.rs and
    // negative-controlled without a database in tests/embedding_shape.rs.
    common::assert_think_result(&server, &result, network_mode).await;
}

#[tokio::test]
async fn test_think_with_continuity() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping integration test - set RUN_DB_TESTS=1 to run");
        return;
    }

    // Offline embedder by default (clu fed-77afac); ALLOW_NETWORK_EMBED=1
    // still opts into the real, intentionally invalid-key, bound-to-degrade
    // path (see test_think_handler above for the full rationale).
    let network_mode = std::env::var("ALLOW_NETWORK_EMBED").ok().as_deref() == Some("1");
    let server = if network_mode {
        create_test_server().await
    } else {
        create_offline_test_server().await
    };

    // Test with non-existent previous_thought_id
    let non_existent_id = "non_existent_thought_id_12345";
    let request = CallToolRequestParams::new("think").with_arguments(
        json!({
            "content": "Test thought with non-existent previous_thought_id",
            "previous_thought_id": non_existent_id
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    // Call the internal handler directly
    let result = server.handle_legacymind_think(request).await;

    // Should succeed despite non-existent ID
    assert!(
        result.is_ok(),
        "Should succeed even with non-existent previous_thought_id"
    );

    let result = result.unwrap();
    assert!(!result.content.is_empty(), "Should return content");
    // Response-shape assertion + the persisted-row POSITIVE witness. Both
    // live in tests/common/mod.rs, shared with tests/mcp_protocol.rs and
    // negative-controlled without a database in tests/embedding_shape.rs.
    common::assert_think_result(&server, &result, network_mode).await;

    // Check that the response contains the preserved ID. This is a hard
    // assertion (not the previous soft `if let` chain, which silently
    // no-oped on any parse failure) -- offline and deterministic, this
    // response shape should never fail to parse.
    let first_content = result
        .content
        .first()
        .expect("think response should have at least one content block");
    let rmcp::model::ContentBlock::Text(text_content) = first_content else {
        panic!("expected a text content block, got {:?}", first_content);
    };
    let parsed: serde_json::Value =
        serde_json::from_str(&text_content.text).expect("think response text should be valid JSON");
    let links = parsed
        .get("links")
        .expect("think response should include a links object");
    let prev_id = links
        .get("previous_thought_id")
        .expect("links should include previous_thought_id");

    // The ID may be prefixed with "thoughts:" when processed
    let expected_with_prefix = format!("thoughts:{}", non_existent_id);
    assert!(
        prev_id == non_existent_id || prev_id == &expected_with_prefix,
        "Previous thought ID should be preserved (found: {}, expected: {})",
        prev_id,
        non_existent_id
    );
}

#[tokio::test]
async fn test_think_invalid_params() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping integration test - set RUN_DB_TESTS=1 to run");
        return;
    }

    let server = create_test_server().await;

    // Test with invalid params (missing required 'content' field)
    let request = CallToolRequestParams::new("think").with_arguments(
        json!({
            "invalid_param": "this parameter doesn't exist"
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    // Call the internal handler directly
    let result = server.handle_legacymind_think(request).await;

    // Should return an error for missing required field
    assert!(result.is_err(), "Should fail with invalid parameters");

    if let Err(err) = result {
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("missing field") || err_msg.contains("content"),
            "Error should indicate missing 'content' field: {}",
            err_msg
        );
    }
}
