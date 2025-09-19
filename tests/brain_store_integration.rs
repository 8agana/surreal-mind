#![cfg(feature = "db_integration")]

use rmcp::model::CallToolRequestParam;
use serde_json::json;
use surreal_mind::{config::Config, server::SurrealMindServer};

#[tokio::test]
async fn brain_store_set_and_get_round_trip() {
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Skipping brain_store integration test: failed to load config ({e})");
            return;
        }
    };

    if !config.runtime.brain_enable {
        eprintln!("Skipping brain_store integration test: SURR_ENABLE_BRAIN not set");
        return;
    }

    let server = match SurrealMindServer::new(&config).await {
        Ok(srv) => srv,
        Err(e) => {
            eprintln!("Skipping brain_store integration test: failed to init server ({e})");
            return;
        }
    };

    let agent = "test_agent";
    let section = "integration_section";
    let content = "## Test\n- integration content";

    // Set content
    let mut set_args = serde_json::Map::new();
    set_args.insert("action".into(), json!("set"));
    set_args.insert("agent".into(), json!(agent));
    set_args.insert("section".into(), json!(section));
    set_args.insert("content".into(), json!(content));

    let set_result = server
        .handle_brain_store(CallToolRequestParam {
            name: "brain_store".into(),
            arguments: Some(set_args),
        })
        .await
        .expect("brain_store set to succeed");

    let set_content = set_result
        .structured_content
        .as_ref()
        .and_then(|v| v.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert_eq!(set_content, content);

    // Fetch content
    let mut get_args = serde_json::Map::new();
    get_args.insert("action".into(), json!("get"));
    get_args.insert("agent".into(), json!(agent));
    get_args.insert("section".into(), json!(section));

    let get_result = server
        .handle_brain_store(CallToolRequestParam {
            name: "brain_store".into(),
            arguments: Some(get_args),
        })
        .await
        .expect("brain_store get to succeed");

    let fetched_content = get_result
        .structured_content
        .as_ref()
        .and_then(|v| v.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert_eq!(fetched_content, content);
}
