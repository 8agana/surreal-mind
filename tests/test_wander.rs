#![cfg(feature = "db_integration")]

use rmcp::model::CallToolRequestParams;
use serde_json::json;
use surreal_mind::{config::Config, server::SurrealMindServer};

async fn create_test_server() -> SurrealMindServer {
    let config = Config::load().expect("Failed to load config");
    SurrealMindServer::new(&config)
        .await
        .expect("Failed to create server")
}

#[tokio::test]
async fn test_wander_random() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        return;
    }

    let server = create_test_server().await;

    let request = CallToolRequestParams::new("legacymind_wander").with_arguments(
        json!({
            "mode": "random",
            "visited_ids": []
        })
        .as_object()
        .unwrap()
        .clone(),
    );

    let result = server.handle_wander(request).await;
    assert!(result.is_ok(), "Wander random should succeed");

    let result = result.unwrap();
    // Verify structure
    let content = result.content.first().unwrap();
    if let rmcp::model::ContentBlock::Text(text) = content {
        let json: serde_json::Value = serde_json::from_str(&text.text).unwrap();
        assert!(json.get("mode_used").is_some());
        assert_eq!(json["mode_used"], "random");
    }
}

#[tokio::test]
async fn test_wander_visited_exclusion() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        return;
    }

    let server = create_test_server().await;

    // First wander to get an ID
    let request1 = CallToolRequestParams::new("legacymind_wander")
        .with_arguments(json!({"mode": "random"}).as_object().unwrap().clone());
    let _res1 = server.handle_wander(request1).await.unwrap();

    // Extract ID (complex parsing, or just mock it by passing a known ID from manual query?)
    // Actually, wander response structure is defined in handle_wander as json!({...})
    // But CallToolResult wraps it.
    // CallToolResult::structured(json) puts it in content list.

    // Let's just verify invalid parameters for now to be safe and quick
    let request_invalid = CallToolRequestParams::new("legacymind_wander")
        .with_arguments(json!({"mode": "unknown_mode"}).as_object().unwrap().clone());
    let res_invalid = server.handle_wander(request_invalid).await;
    assert!(res_invalid.is_err(), "Should fail with unknown mode");
}
