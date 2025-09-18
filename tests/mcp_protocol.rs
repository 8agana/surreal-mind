#![cfg(feature = "db_integration")]

use rmcp::{
    model::{
        CallToolRequestParam, CallToolRequest, ListToolsRequest, ListToolsRequestMethod,
        CallToolRequestMethod, ClientRequest, InitializeRequestParam, JsonRpcRequest,
        JsonRpcVersion2_0, NumberOrString, RequestOptionalParam, ErrorCode
    },
    service::{serve_directly, RxJsonRpcMessage, TxJsonRpcMessage},
    transport::sink_stream::SinkStreamTransport,
    RoleServer,
};
use serde_json::json;
use surreal_mind::{config::Config, server::SurrealMindServer};
use tokio::sync::mpsc;
use futures_util::{stream, StreamExt};
use tokio_util::sync::PollSender;

// Helper to create transport channels
fn create_test_transport() -> (
    mpsc::Sender<RxJsonRpcMessage<RoleServer>>,
    mpsc::Receiver<TxJsonRpcMessage<RoleServer>>,
    SinkStreamTransport<
        PollSender<TxJsonRpcMessage<RoleServer>>,
        stream::BoxStream<'static, RxJsonRpcMessage<RoleServer>>
    >
) {
    // Create channels for client -> server (client sends RxJsonRpcMessage to server)
    let (client_tx, server_rx) = mpsc::channel::<RxJsonRpcMessage<RoleServer>>(100);

    // Create channels for server -> client (server sends TxJsonRpcMessage to client)
    let (server_tx, client_rx) = mpsc::channel::<TxJsonRpcMessage<RoleServer>>(100);

    // Wrap for Sink/Stream traits
    let poll_sender = PollSender::new(server_tx);
    let receiver_stream = stream::unfold(server_rx, |mut rx| async {
        rx.recv().await.map(|msg| (msg, rx))
    })
    .boxed();

    // Create transport for serve_directly
    let transport = SinkStreamTransport::new(poll_sender, receiver_stream);

    (client_tx, client_rx, transport)
}

#[tokio::test]
async fn test_list_tools_protocol() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping protocol test - set RUN_DB_TESTS=1 to run");
        return;
    }

    // Create server
    let config = Config::load().expect("Failed to load config");
    let server = SurrealMindServer::new(&config)
        .await
        .expect("Failed to create server");

    // Create transport
    let (client_tx, mut client_rx, transport) = create_test_transport();

    // Start the server with serve_directly (peer_info can be None for InitializeRequestParam)
    let running_service = serve_directly(server, transport, None::<InitializeRequestParam>);

    // Send a ListToolsRequest through the protocol
    let list_tools_req: ListToolsRequest = RequestOptionalParam {
        method: ListToolsRequestMethod,
        params: None,
        extensions: Default::default(),
    };
    let request = ClientRequest::ListToolsRequest(list_tools_req);

    // Wrap in JSON-RPC message with ID
    let json_rpc_request = JsonRpcRequest {
        jsonrpc: JsonRpcVersion2_0,
        id: NumberOrString::String("test-list-tools".into()),
        request,
    };
    let request_msg = RxJsonRpcMessage::<RoleServer>::Request(json_rpc_request);
    client_tx.send(request_msg).await.unwrap();

    // Receive response
    if let Some(response) = client_rx.recv().await {
        match response {
            TxJsonRpcMessage::<RoleServer>::Response(json_response) => {
                let id_str = match &json_response.id {
                    rmcp::model::NumberOrString::String(s) => s.as_ref() as &str,
                    rmcp::model::NumberOrString::Number(n) => panic!("Expected string ID, got number: {}", n),
                };
                assert_eq!(id_str, "test-list-tools", "Response ID should match request");

                // The result is a ClientResult, serialize it to check tools
                if let Ok(result_json) = serde_json::to_value(&json_response.result) {
                    if let Some(tools) = result_json.get("tools").and_then(|t| t.as_array()) {
                        assert!(!tools.is_empty(), "Should have at least one tool");

                        // Check for legacymind_search tool
                        let tool_names: Vec<String> = tools.iter()
                            .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                            .collect();

                        assert!(
                            tool_names.contains(&"legacymind_search".to_string()),
                            "Should include legacymind_search tool"
                        );
                    }
                }
            }
            _ => panic!("Expected Response, got {:?}", response),
        }
    } else {
        panic!("No response received");
    }

    // Cleanup
    let _ = running_service.cancel().await;
}

#[tokio::test]
async fn test_call_tool_protocol_invalid_params() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping protocol test - set RUN_DB_TESTS=1 to run");
        return;
    }

    // Create server
    let config = Config::load().expect("Failed to load config");
    let server = SurrealMindServer::new(&config)
        .await
        .expect("Failed to create server");

    // Create transport
    let (client_tx, mut client_rx, transport) = create_test_transport();

    // Start the server
    let running_service = serve_directly(server, transport, None::<InitializeRequestParam>);

    // Send CallToolRequest with invalid params
    let params = CallToolRequestParam {
        name: "legacymind_think".into(),
        arguments: Some(json!({
            "invalid_param": "this should fail"
        }).as_object().unwrap().clone()),
    };

    let call_tool_req: CallToolRequest = rmcp::model::Request {
        method: CallToolRequestMethod,
        params,
        extensions: Default::default(),
    };
    let request = ClientRequest::CallToolRequest(call_tool_req);

    // Send request (client sends RxJsonRpcMessage to server)
    let json_rpc_request = JsonRpcRequest {
        jsonrpc: JsonRpcVersion2_0,
        id: NumberOrString::String("test-invalid-call".into()),
        request,
    };
    let request_msg = RxJsonRpcMessage::<RoleServer>::Request(json_rpc_request);
    client_tx.send(request_msg).await.unwrap();

    // Receive error response
    if let Some(response) = client_rx.recv().await {
        match response {
            TxJsonRpcMessage::<RoleServer>::Error(json_error) => {
                // JsonRpcError has id and error fields
                let id_str = match &json_error.id {
                    rmcp::model::NumberOrString::String(s) => s.as_ref() as &str,
                    rmcp::model::NumberOrString::Number(n) => panic!("Expected string ID, got number: {}", n),
                };
                assert_eq!(id_str, "test-invalid-call", "Error ID should match request");
                // Should be INVALID_PARAMS error
                assert_eq!(json_error.error.code, ErrorCode(-32602), "Should return INVALID_PARAMS error code");
            }
            _ => panic!("Expected Error response for invalid params, got {:?}", response),
        }
    } else {
        panic!("No response received");
    }

    let _ = running_service.cancel().await;
}

#[tokio::test]
async fn test_call_tool_continuity_fallback_protocol() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping protocol test - set RUN_DB_TESTS=1 to run");
        return;
    }

    // Create server
    let config = Config::load().expect("Failed to load config");
    let server = SurrealMindServer::new(&config)
        .await
        .expect("Failed to create server");

    // Create transport
    let (client_tx, mut client_rx, transport) = create_test_transport();

    // Start the server
    let running_service = serve_directly(server, transport, None::<InitializeRequestParam>);

    // Send CallToolRequest with non-existent previous_thought_id
    let non_existent_id = "non_existent_thought_12345";
    let params = CallToolRequestParam {
        name: "legacymind_think".into(),
        arguments: Some(json!({
            "content": "Test thought with continuity fallback",
            "previous_thought_id": non_existent_id
        }).as_object().unwrap().clone()),
    };

    let call_tool_req: CallToolRequest = rmcp::model::Request {
        method: CallToolRequestMethod,
        params,
        extensions: Default::default(),
    };
    let request = ClientRequest::CallToolRequest(call_tool_req);

    // Send request (client sends RxJsonRpcMessage to server)
    let json_rpc_request = JsonRpcRequest {
        jsonrpc: JsonRpcVersion2_0,
        id: NumberOrString::String("test-continuity".into()),
        request,
    };
    let request_msg = RxJsonRpcMessage::<RoleServer>::Request(json_rpc_request);
    client_tx.send(request_msg).await.unwrap();

    // Receive response
    if let Some(response) = client_rx.recv().await {
        match response {
            TxJsonRpcMessage::<RoleServer>::Response(json_response) => {
                let id_str = match &json_response.id {
                    rmcp::model::NumberOrString::String(s) => s.as_ref() as &str,
                    rmcp::model::NumberOrString::Number(n) => panic!("Expected string ID, got number: {}", n),
                };
                assert_eq!(id_str, "test-continuity", "Response ID should match request");

                // Serialize the ClientResult to check the response
                if let Ok(tool_result) = serde_json::to_value(&json_response.result) {
                    if let Some(content) = tool_result.get("content").and_then(|c| c.as_array()) {
                        if let Some(first) = content.first() {
                            // Try to parse the actual thought response
                            if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                                if let Ok(thought_data) = serde_json::from_str::<serde_json::Value>(text) {
                                    if let Some(links) = thought_data.get("links") {
                                        if let Some(prev_id) = links.get("previous_thought_id").and_then(|p| p.as_str()) {
                                            // ID should be preserved (may have "thoughts:" prefix)
                                            assert!(
                                                prev_id == non_existent_id || prev_id == &format!("thoughts:{}", non_existent_id),
                                                "Previous thought ID should be preserved through protocol (got: {})",
                                                prev_id
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            TxJsonRpcMessage::<RoleServer>::Error(json_error) => {
                panic!("Should not error on non-existent previous_thought_id: {:?}", json_error);
            }
            _ => panic!("Expected Response, got {:?}", response),
        }
    } else {
        panic!("No response received");
    }

    let _ = running_service.cancel().await;
}