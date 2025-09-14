#![cfg(feature = "db_integration")]

use rmcp::ServerHandler;
use rmcp::model::{CallToolRequestParam, ContentType, Meta, NumberOrString, PaginatedRequestParam};
use rmcp::service::RequestContext;
use std::collections::HashMap;
use surreal_mind::{config::Config, server::SurrealMindServer};

#[tokio::test]
async fn test_tools_list_has_legacymind_think_when_enabled() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        return;
    }
    let config = Config::load().expect("config load");
    let server = SurrealMindServer::new(&config).await.expect("server init");
    let res = server
        .list_tools(
            Some(PaginatedRequestParam::default()),
            RequestContext {
                id: Some(NumberOrString::String("test".to_string())),
                meta: Some(Meta::default()),
                ct: ContentType::Json,
                extensions: HashMap::new(),
                peer: None,
            },
        )
        .await
        .expect("list_tools");
    let names: Vec<_> = res.tools.iter().map(|t| t.name.to_string()).collect();
    assert!(names.contains(&"legacymind_think".to_string()));
}

#[tokio::test]
async fn test_call_tool_invalid_params_rejected_legacymind_think() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        return;
    }
    let config = Config::load().expect("config load");
    let server = SurrealMindServer::new(&config).await.expect("server init");

    // invalid injection_scale
    let mut obj = serde_json::Map::new();
    obj.insert("content".into(), serde_json::Value::String("test".into()));
    obj.insert(
        "injection_scale".into(),
        serde_json::Value::Number(serde_json::Number::from(9)),
    ); // invalid

    let req = CallToolRequestParam {
        name: "legacymind_think".into(),
        arguments: Some(obj),
    };

    let err = server
        .call_tool(
            req,
            RequestContext {
                id: Some(NumberOrString::String("test".to_string())),
                meta: Some(Meta::default()),
                ct: ContentType::Json,
                extensions: HashMap::new(),
                peer: None,
            },
        )
        .await
        .expect_err("should error on invalid params");

    assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}
