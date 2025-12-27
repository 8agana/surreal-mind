#![cfg(feature = "db_integration")]

// Protocol-level tests using rmcp's sink/stream transport and serve_directly.
// Approach:
// - We construct paired mpsc channels and wrap them as a SinkStreamTransport, which
//   implements the necessary Sink/Stream traits for rmcp's service layer.
// - serve_directly(server, transport, None::<InitializeRequestParam>) drives the
//   JSON-RPC protocol against our in-process transport without spawning a real stdio
//   process. This gives us full end-to-end coverage of JSON-RPC message flow while
//   keeping test control and determinism.
// - Each test runs its logic inside a "finally"-style wrapper that ensures
//   running_service.cancel().await is called even if the test panics, avoiding
//   background task leaks.

use futures_util::{StreamExt, future::FutureExt, stream};
use rmcp::{
    RoleServer,
    model::{
        CallToolRequest, CallToolRequestMethod, CallToolRequestParam, ClientRequest, ErrorCode,
        InitializeRequestParam, JsonRpcRequest, JsonRpcVersion2_0, ListToolsRequest,
        ListToolsRequestMethod, NumberOrString, RequestOptionalParam,
    },
    service::{RxJsonRpcMessage, TxJsonRpcMessage, serve_directly},
    transport::sink_stream::SinkStreamTransport,
};
use serde_json::{Map, Value, json};
use surreal_mind::{config::Config, server::SurrealMindServer};
use tokio::sync::mpsc;
use tokio_util::sync::PollSender;

// Helper to create transport channels
#[allow(clippy::type_complexity)]
fn create_test_transport() -> (
    mpsc::Sender<RxJsonRpcMessage<RoleServer>>,
    mpsc::Receiver<TxJsonRpcMessage<RoleServer>>,
    SinkStreamTransport<
        PollSender<TxJsonRpcMessage<RoleServer>>,
        stream::BoxStream<'static, RxJsonRpcMessage<RoleServer>>,
    >,
) {
    // Create channels for client -> server (client sends RxJsonRpcMessage to server)
    let (client_tx, server_rx) = mpsc::channel::<RxJsonRpcMessage<RoleServer>>(100);

    // Create channels for server -> client (server sends TxJsonRpcMessage to client)
    let (server_tx, client_rx) = mpsc::channel::<TxJsonRpcMessage<RoleServer>>(100);

    // Wrap for Sink/Stream traits
    let poll_sender = PollSender::new(server_tx);
    let receiver_stream = stream::unfold(
        server_rx,
        |mut rx: mpsc::Receiver<RxJsonRpcMessage<RoleServer>>| async {
            rx.recv().await.map(|msg| (msg, rx))
        },
    )
    .boxed();

    // Create transport for serve_directly
    let transport = SinkStreamTransport::new(poll_sender, receiver_stream);

    (client_tx, client_rx, transport)
}

// Helper: build a JSON-RPC request message with a string ID
fn build_jsonrpc_request(request: ClientRequest, id: &str) -> RxJsonRpcMessage<RoleServer> {
    let json_rpc_request = JsonRpcRequest {
        jsonrpc: JsonRpcVersion2_0,
        id: NumberOrString::String(id.to_string().into()),
        request,
    };
    RxJsonRpcMessage::<RoleServer>::Request(json_rpc_request)
}

// Helper: construct a ListTools ClientRequest
fn make_list_tools_request() -> ClientRequest {
    let list_tools_req: ListToolsRequest = RequestOptionalParam {
        method: ListToolsRequestMethod,
        params: None,
        extensions: Default::default(),
    };
    ClientRequest::ListToolsRequest(list_tools_req)
}

// Helper: construct a CallTool ClientRequest
fn make_call_tool_request(name: &str, args: Map<String, Value>) -> ClientRequest {
    let params = CallToolRequestParam {
        name: name.to_string().into(),
        arguments: Some(args),
    };
    let call_tool_req: CallToolRequest = rmcp::model::Request {
        method: CallToolRequestMethod,
        params,
        extensions: Default::default(),
    };
    ClientRequest::CallToolRequest(call_tool_req)
}

// Helper: run a protocol test with guaranteed cancellation of the running service
async fn with_direct_service<F, Fut>(server: SurrealMindServer, f: F)
where
    F: FnOnce(
        mpsc::Sender<RxJsonRpcMessage<RoleServer>>,
        mpsc::Receiver<TxJsonRpcMessage<RoleServer>>,
    ) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let (client_tx, client_rx, transport) = create_test_transport();
    let running_service = serve_directly(server, transport, None::<InitializeRequestParam>);

    // Run the provided future and always cancel the service afterward
    let result = std::panic::AssertUnwindSafe(f(client_tx, client_rx))
        .catch_unwind()
        .await;
    let _ = running_service.cancel().await;
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
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

    // Run with guaranteed cancellation
    with_direct_service(server, |client_tx, mut client_rx| async move {
        // Send a ListToolsRequest through the protocol
        let request = make_list_tools_request();
        let request_msg = build_jsonrpc_request(request, "test-list-tools");
        client_tx.send(request_msg).await.unwrap();

        // Receive response
        if let Some(response) = client_rx.recv().await {
            match response {
                TxJsonRpcMessage::<RoleServer>::Response(json_response) => {
                    let id_str = match &json_response.id {
                        rmcp::model::NumberOrString::String(s) => s.as_ref(),
                        rmcp::model::NumberOrString::Number(n) => {
                            panic!("Expected string ID, got number: {}", n)
                        }
                    };
                    assert_eq!(
                        id_str, "test-list-tools",
                        "Response ID should match request"
                    );

                    // The result is a ClientResult, serialize it to check tools
                    if let Ok(result_json) = serde_json::to_value(&json_response.result)
                        && let Some(tools) = result_json.get("tools").and_then(|t| t.as_array())
                    {
                        assert!(!tools.is_empty(), "Should have at least one tool");

                        // Check for legacymind_search tool
                        let tool_names: Vec<String> = tools
                            .iter()
                            .filter_map(|t| {
                                t.get("name")
                                    .and_then(|n| n.as_str())
                                    .map(|s| s.to_string())
                            })
                            .collect();

                        assert!(
                            tool_names.contains(&"legacymind_search".to_string()),
                            "Should include legacymind_search tool"
                        );
                    }
                }
                _ => panic!("Expected Response, got {:?}", response),
            }
        } else {
            panic!("No response received");
        }
    })
    .await;
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

    with_direct_service(server, |client_tx, mut client_rx| async move {
        // Send CallToolRequest with invalid params
        let args = json!({
            "invalid_param": "this should fail"
        })
        .as_object()
        .unwrap()
        .clone();
        let request = make_call_tool_request("legacymind_think", args);

        // Send request (client sends RxJsonRpcMessage to server)
        let request_msg = build_jsonrpc_request(request, "test-invalid-call");
        client_tx.send(request_msg).await.unwrap();

        // Receive error response
        if let Some(response) = client_rx.recv().await {
            match response {
                TxJsonRpcMessage::<RoleServer>::Error(json_error) => {
                    // JsonRpcError has id and error fields
                    let id_str = match &json_error.id {
                        rmcp::model::NumberOrString::String(s) => s.as_ref(),
                        rmcp::model::NumberOrString::Number(n) => {
                            panic!("Expected string ID, got number: {}", n)
                        }
                    };
                    assert_eq!(id_str, "test-invalid-call", "Error ID should match request");
                    // Should be INVALID_PARAMS error
                    assert_eq!(
                        json_error.error.code,
                        ErrorCode(-32602),
                        "Should return INVALID_PARAMS error code"
                    );
                }
                _ => panic!(
                    "Expected Error response for invalid params, got {:?}",
                    response
                ),
            }
        } else {
            panic!("No response received");
        }
    })
    .await;
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

    with_direct_service(server, |client_tx, mut client_rx| async move {
        // Send CallToolRequest with non-existent previous_thought_id
        let non_existent_id = "non_existent_thought_12345";
        let args = json!({
            "content": "Test thought with continuity fallback",
            "previous_thought_id": non_existent_id
        })
        .as_object()
        .unwrap()
        .clone();
        let request = make_call_tool_request("legacymind_think", args);

        // Send request (client sends RxJsonRpcMessage to server)
        let request_msg = build_jsonrpc_request(request, "test-continuity");
        client_tx.send(request_msg).await.unwrap();

        // Receive response
        if let Some(response) = client_rx.recv().await {
            match response {
                TxJsonRpcMessage::<RoleServer>::Response(json_response) => {
                    let id_str = match &json_response.id {
                        rmcp::model::NumberOrString::String(s) => s.as_ref(),
                        rmcp::model::NumberOrString::Number(n) => panic!("Expected string ID, got number: {}", n),
                    };
                    assert_eq!(id_str, "test-continuity", "Response ID should match request");

                    // Serialize the ClientResult to check the response
                    if let Ok(tool_result) = serde_json::to_value(&json_response.result)
                        && let Some(content) = tool_result.get("content").and_then(|c| c.as_array())
                        && let Some(first) = content.first()
                    {
                        // Try to parse the actual thought response
                        if let Some(text) = first.get("text").and_then(|t| t.as_str())
                            && let Ok(thought_data) = serde_json::from_str::<serde_json::Value>(text)
                            && let Some(links) = thought_data.get("links")
                            && let Some(prev_id) = links.get("previous_thought_id").and_then(|p| p.as_str())
                        {
                            // ID should be preserved (may have "thoughts:" prefix)
                            assert!(
                                prev_id == non_existent_id || prev_id == format!("thoughts:{}", non_existent_id),
                                "Previous thought ID should be preserved through protocol (got: {})",
                                prev_id
                            );
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
    }).await;
}
