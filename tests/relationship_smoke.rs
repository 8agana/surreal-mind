use rmcp::model::CallToolRequestParam;
use surreal_mind::{config::Config, server::SurrealMindServer};

#[tokio::test]
async fn relationship_flow_smoke() {
    // Only run when explicitly enabled to avoid external DB dependency in CI
    if std::env::var("SURR_SMOKE_TEST").ok().as_deref() != Some("1") {
        eprintln!("Skipping relationship_flow_smoke (set SURR_SMOKE_TEST=1 to run)");
        return;
    }

    let config = Config::load().expect("config load");
    let server = SurrealMindServer::new(&config).await.expect("server init");

    // Create two entities
    let mut a_args = serde_json::Map::new();
    a_args.insert("kind".into(), serde_json::Value::String("entity".into()));
    a_args.insert(
        "data".into(),
        serde_json::json!({"name": "SmokeEntityA", "entity_type": "test"}),
    );
    a_args.insert("upsert".into(), serde_json::Value::Bool(true));
    let e1 = server
        .handle_knowledgegraph_create(CallToolRequestParam {
            name: "remember".into(),
            arguments: Some(a_args),
        })
        .await
        .unwrap();
    let id_a = e1
        .structured_content
        .as_ref()
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    let mut b_args = serde_json::Map::new();
    b_args.insert("kind".into(), serde_json::Value::String("entity".into()));
    b_args.insert(
        "data".into(),
        serde_json::json!({"name": "SmokeEntityB", "entity_type": "test"}),
    );
    b_args.insert("upsert".into(), serde_json::Value::Bool(true));
    let e2 = server
        .handle_knowledgegraph_create(CallToolRequestParam {
            name: "remember".into(),
            arguments: Some(b_args),
        })
        .await
        .unwrap();
    let id_b = e2
        .structured_content
        .as_ref()
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    // Create relationship (upsert-safe)
    let mut r_args = serde_json::Map::new();
    r_args.insert(
        "kind".into(),
        serde_json::Value::String("relationship".into()),
    );
    r_args.insert(
        "data".into(),
        serde_json::json!({"from_id": id_a, "to_id": id_b, "relationship_type": "relates_to", "notes": "smoke"}),
    );
    r_args.insert("upsert".into(), serde_json::Value::Bool(true));
    let rel = server
        .handle_knowledgegraph_create(CallToolRequestParam {
            name: "remember".into(),
            arguments: Some(r_args),
        })
        .await
        .unwrap();
    let rel_id = rel
        .structured_content
        .as_ref()
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    assert!(!rel_id.is_empty());

    // Verify via search
    let mut s_args = serde_json::Map::new();
    s_args.insert(
        "target".into(),
        serde_json::Value::String("relationship".into()),
    );
    s_args.insert("top_k".into(), serde_json::Value::Number(10u64.into()));
    let items_val = server
        .handle_unified_search(CallToolRequestParam {
            name: "search".into(),
            arguments: Some(s_args),
        })
        .await
        .unwrap()
        .structured_content
        .unwrap();
    let items = items_val["memories"]["items"].as_array().cloned().unwrap();

    assert!(items.iter().any(|it| it["id"].as_str() == Some(&rel_id)));
}
