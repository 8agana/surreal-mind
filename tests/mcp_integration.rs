#![cfg(feature = "db_integration")]

use rmcp::model::{CallToolRequestParam, PaginatedRequestParam};
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use surreal_mind::{config::Config, *};

#[tokio::test]
async fn test_tools_list_has_convo_think_when_enabled() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        return;
    }
    let config = Config::load().expect("config load");
    let server = SurrealMindServer::new(&config).await.expect("server init");
    let res = server
        .list_tools(
            Some(PaginatedRequestParam::default()),
            RequestContext::<RoleServer>::default(),
        )
        .await
        .expect("list_tools");
    let names: Vec<_> = res.tools.iter().map(|t| t.name.to_string()).collect();
    assert!(names.contains(&"convo_think".to_string()));
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
        name: "convo_think".into(),
        arguments: Some(obj),
        ..Default::default()
    };

    let err = server
        .call_tool(req, RequestContext::<RoleServer>::default())
        .await
        .expect_err("should error on invalid params");

    assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
}
