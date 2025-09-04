#![cfg(feature = "db_integration")]

use rmcp::ServerHandler;
use rmcp::model::{CallToolRequestParam, PaginatedRequestParam};
use rmcp::service::RequestContext;
use surreal_mind::{config::Config, server::SurrealMindServer};

#[tokio::test]
async fn test_tools_list_has_think_convo_when_enabled() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        return;
    }
    let config = Config::load().expect("config load");
    let server = SurrealMindServer::new(&config).await.expect("server init");
    let res = server
        .list_tools(
            Some(PaginatedRequestParam::default()),
            RequestContext::with_id("test".into()),
        )
        .await
        .expect("list_tools");
    let names: Vec<_> = res.tools.iter().map(|t| t.name.to_string()).collect();
    assert!(names.contains(&"think_convo".to_string()));
}

#[tokio::test]
async fn test_call_tool_invalid_params_rejected_when_enabled() {
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
        name: "think_convo".into(),
        arguments: Some(obj),
    };

    let err = server
        .call_tool(req, RequestContext::with_id("test".into()))
        .await
        .expect_err("should error on invalid params");

    assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}
