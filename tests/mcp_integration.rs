#![cfg(feature = "db_integration")]

// Integration tests focused on direct handler + DB checks.
// For full MCP protocol coverage (JSON-RPC over sink/stream), see tests/mcp_protocol.rs.

use rmcp::{ServerHandler, model::CallToolRequestParams};
use serde_json::json;
use surreal_mind::{config::Config, server::SurrealMindServer};

// Helper to create a test server instance
async fn create_test_server() -> SurrealMindServer {
    let config = Config::load().expect("Failed to load config");
    SurrealMindServer::new(&config)
        .await
        .expect("Failed to create server")
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

    // Reaches api.openai.com for real: `handle_legacymind_think` always
    // attempts a live OpenAI embedding call for `content`
    // (src/tools/thinking.rs:236 -> embedder.embed()), and `embed_strict`
    // defaults to false (src/config.rs:149), so the handler tolerates the
    // failure and this test would otherwise "pass" while silently making a
    // real network call under a wrapper whose contract is zero external
    // calls (Codex blocked fed-734b8f #172 item 2 for exactly this). Skip
    // unless the operator explicitly opts in; the real fix is the offline
    // embedder tracked at clu fed-77afac.
    if std::env::var("ALLOW_NETWORK_EMBED").is_err() {
        eprintln!("skipped: reaches api.openai.com; needs the offline embedder (clu fed-77afac)");
        return;
    }

    let server = create_test_server().await;

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
}

#[tokio::test]
async fn test_think_with_continuity() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping integration test - set RUN_DB_TESTS=1 to run");
        return;
    }

    // Reaches api.openai.com for real: `handle_legacymind_think` always
    // attempts a live OpenAI embedding call for `content`
    // (src/tools/thinking.rs:236 -> embedder.embed()), and `embed_strict`
    // defaults to false (src/config.rs:149), so the handler tolerates the
    // failure and this test would otherwise "pass" while silently making a
    // real network call under a wrapper whose contract is zero external
    // calls (Codex blocked fed-734b8f #172 item 2 for exactly this). Skip
    // unless the operator explicitly opts in; the real fix is the offline
    // embedder tracked at clu fed-77afac.
    if std::env::var("ALLOW_NETWORK_EMBED").is_err() {
        eprintln!("skipped: reaches api.openai.com; needs the offline embedder (clu fed-77afac)");
        return;
    }

    let server = create_test_server().await;

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

    // Check that the response contains the preserved ID
    if !result.content.is_empty()
        && let Some(first_content) = result.content.first()
    {
        // Extract text from ContentBlock enum
        if let rmcp::model::ContentBlock::Text(text_content) = first_content
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text_content.text)
            && let Some(links) = parsed.get("links")
            && let Some(prev_id) = links.get("previous_thought_id")
        {
            // The ID may be prefixed with "thoughts:" when processed
            let expected_with_prefix = format!("thoughts:{}", non_existent_id);
            assert!(
                prev_id == non_existent_id || prev_id == &expected_with_prefix,
                "Previous thought ID should be preserved (found: {}, expected: {})",
                prev_id,
                non_existent_id
            );
        }
    }
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
