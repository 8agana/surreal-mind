#![cfg(feature = "db_integration")]

// Protocol-level tests using rmcp's sink/stream transport and serve_directly.
// Approach:
// - We construct paired mpsc channels and wrap them as a SinkStreamTransport, which
//   implements the necessary Sink/Stream traits for rmcp's service layer.
// - serve_directly(server, transport, None::<InitializeRequestParams>) drives the
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
        CallToolRequest, CallToolRequestParams, ClientCapabilities, ClientRequest, ErrorCode,
        Implementation, InitializeRequest, InitializeRequestParams, JsonRpcRequest,
        JsonRpcVersion2_0, ListToolsRequest, ListToolsRequestMethod, NumberOrString,
        ProtocolVersion, RequestOptionalParam,
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
    let params = CallToolRequestParams::new(name.to_string()).with_arguments(args);
    let call_tool_req: CallToolRequest = rmcp::model::Request::new(params);
    ClientRequest::CallToolRequest(call_tool_req)
}

// Helper: construct an Initialize ClientRequest for a specific requested
// protocol version (PROTO-01/02/03).
fn make_initialize_request(protocol_version: ProtocolVersion) -> ClientRequest {
    let params = InitializeRequestParams::new(
        ClientCapabilities::default(),
        Implementation::new("mcp-protocol-test", "0.0.0"),
    )
    .with_protocol_version(protocol_version);
    let init_req: InitializeRequest = rmcp::model::Request::new(params);
    ClientRequest::InitializeRequest(init_req)
}

// The exact 16-tool contract this upgrade must preserve (TOOL-01), in
// registration order.
const EXPECTED_TOOL_NAMES: [&str; 16] = [
    "think",
    "wander",
    "maintain",
    "journal",
    "rethink",
    "corrections",
    "test_notification",
    "remember",
    "howto",
    "call_gem",
    "call_cc",
    "call_vibe",
    "search",
    "call_status",
    "call_jobs",
    "call_cancel",
];

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
    let running_service = serve_directly(server, transport, None::<InitializeRequestParams>);

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
                        // TOOL-01: exact 16-name, ordered tool contract. No
                        // additions or removals; `journal` must be present
                        // (it was previously omitted from a stale startup
                        // log — see upgrade doc Phase 8).
                        let tool_names: Vec<String> = tools
                            .iter()
                            .filter_map(|t| {
                                t.get("name")
                                    .and_then(|n| n.as_str())
                                    .map(|s| s.to_string())
                            })
                            .collect();
                        assert_eq!(
                            tool_names, EXPECTED_TOOL_NAMES,
                            "tools/list must return exactly the 16-tool contract, in order"
                        );

                        // TOOL-02: spot-check titles/descriptions/input
                        // schemas survived the `Tool::new(...).with_title(...)`
                        // migration for a representative sample of tools.
                        let think = tools
                            .iter()
                            .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("think"))
                            .expect("think tool must be present");
                        assert_eq!(think.get("title").and_then(|v| v.as_str()), Some("Think"));
                        assert_eq!(
                            think.get("description").and_then(|v| v.as_str()),
                            Some(
                                "Unified thinking tool with automatic mode routing (Plan, Build, Debug, Stuck)"
                            )
                        );
                        assert_eq!(
                            think
                                .get("inputSchema")
                                .and_then(|s| s.get("required"))
                                .and_then(|r| r.as_array())
                                .map(|r| r.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>()),
                            Some(vec!["content"]),
                            "think's input schema must still require content"
                        );

                        let journal = tools
                            .iter()
                            .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("journal"))
                            .expect("journal tool must be present");
                        assert_eq!(
                            journal.get("title").and_then(|v| v.as_str()),
                            Some("Journal")
                        );

                        let call_vibe = tools
                            .iter()
                            .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("call_vibe"))
                            .expect("call_vibe tool must be present");
                        assert_eq!(
                            call_vibe.get("title").and_then(|v| v.as_str()),
                            Some("Call Vibe")
                        );
                    } else {
                        panic!("tools/list response did not contain a `tools` array");
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
        let request = make_call_tool_request("think", args);

        // Send request (client sends RxJsonRpcMessage to server)
        let request_msg = build_jsonrpc_request(request, "test-invalid-call");
        client_tx.send(request_msg).await.unwrap();

        // Receive error response
        if let Some(response) = client_rx.recv().await {
            match response {
                TxJsonRpcMessage::<RoleServer>::Error(json_error) => {
                    // JsonRpcError has id and error fields. `id` is
                    // `Option<RequestId>` in rmcp 3.1.4 (MCP 2026-07-28 error
                    // responses may omit it), so this test's synchronous
                    // request/response exchange asserts it is present.
                    let id_str = match json_error.id.as_ref() {
                        Some(rmcp::model::NumberOrString::String(s)) => s.as_ref(),
                        Some(rmcp::model::NumberOrString::Number(n)) => {
                            panic!("Expected string ID, got number: {}", n)
                        }
                        None => panic!("Expected an id on the error response, got none"),
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
        let request = make_call_tool_request("think", args);

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

// PROTO-01/02/03/05: rmcp's default `initialize` negotiates the response
// protocol version against `supported_protocol_versions()` rather than
// echoing whatever the client claims (upgrade doc D3 — the previous
// `initialize` override did the latter). This test drives real `initialize`
// requests through the protocol harness for a supported legacy version, the
// current latest supported version, and an unsupported future version, and
// also asserts `tools.listChanged` still serializes as explicit `false`
// (D4/PROTO-05) rather than becoming absent.
#[tokio::test]
async fn test_initialize_protocol_negotiation() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping protocol test - set RUN_DB_TESTS=1 to run");
        return;
    }

    async fn initialize_and_get_result(requested: ProtocolVersion, id: &str) -> serde_json::Value {
        let config = Config::load().expect("Failed to load config");
        let server = SurrealMindServer::new(&config)
            .await
            .expect("Failed to create server");

        // `with_direct_service`'s callback has a fixed `Output = ()`, so the
        // captured initialize result is carried out through a oneshot
        // channel rather than a closed-over variable (an `async move` block
        // inside a non-`move` outer closure cannot move a variable owned by
        // the enclosing function scope).
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let id = id.to_string();
        with_direct_service(server, move |client_tx, mut client_rx| async move {
            let request = make_initialize_request(requested);
            let request_msg = build_jsonrpc_request(request, &id);
            client_tx.send(request_msg).await.unwrap();

            match client_rx.recv().await {
                Some(TxJsonRpcMessage::<RoleServer>::Response(json_response)) => {
                    let value = serde_json::to_value(&json_response.result)
                        .expect("initialize result must serialize");
                    let _ = result_tx.send(value);
                }
                other => panic!("Expected an initialize Response, got {:?}", other),
            }
        })
        .await;
        result_rx.await.expect("initialize result was not captured")
    }

    // PROTO-01: a supported legacy version negotiates to itself (rmcp
    // accepts any version present in `ProtocolVersion::KNOWN_VERSIONS`,
    // which `supported_protocol_versions()` defaults to unmodified).
    let legacy_result =
        initialize_and_get_result(ProtocolVersion::V_2025_03_26, "test-init-legacy").await;
    assert_eq!(
        legacy_result
            .get("protocolVersion")
            .and_then(|v| v.as_str()),
        Some(ProtocolVersion::V_2025_03_26.as_str()),
        "A supported legacy version must be negotiated, not silently upgraded"
    );

    // PROTO-02: the current latest supported version negotiates to itself.
    let latest_result =
        initialize_and_get_result(ProtocolVersion::LATEST, "test-init-latest").await;
    assert_eq!(
        latest_result
            .get("protocolVersion")
            .and_then(|v| v.as_str()),
        Some(ProtocolVersion::LATEST.as_str()),
    );

    // PROTO-03: an unsupported future version must never be echoed back as
    // accepted (that was the previous override's bug — D3). rmcp's default
    // negotiation falls back to its own preferred version instead.
    // `ProtocolVersion`'s inner string is private with no public
    // constructor for arbitrary values, so build one the same way the wire
    // protocol does: through `Deserialize`.
    let future_version: ProtocolVersion =
        serde_json::from_value(json!("2099-01-01")).expect("ProtocolVersion must deserialize");
    let future_result = initialize_and_get_result(future_version.clone(), "test-init-future").await;
    assert_ne!(
        future_result
            .get("protocolVersion")
            .and_then(|v| v.as_str()),
        Some(future_version.as_str()),
        "An unsupported future protocol version must never be echoed back as accepted"
    );

    // PROTO-05: `tools.listChanged` must remain an explicit `false` in every
    // negotiated response, not become absent.
    for result in [&legacy_result, &latest_result, &future_result] {
        assert_eq!(
            result
                .get("capabilities")
                .and_then(|c| c.get("tools"))
                .and_then(|t| t.get("listChanged")),
            Some(&serde_json::Value::Bool(false)),
            "tools.listChanged must serialize as explicit false"
        );
    }
}

// TOOL-06: `test_notification` remains listed and produces a
// client-observable notification without workspace warnings (D8, the
// narrow SEP-2577 deprecation bridge).
#[tokio::test]
async fn test_notification_protocol_bridge() {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("Skipping protocol test - set RUN_DB_TESTS=1 to run");
        return;
    }

    let config = Config::load().expect("Failed to load config");
    let server = SurrealMindServer::new(&config)
        .await
        .expect("Failed to create server");

    with_direct_service(server, |client_tx, mut client_rx| async move {
        let args = json!({ "message": "protocol bridge smoke", "level": "info" })
            .as_object()
            .unwrap()
            .clone();
        let request = make_call_tool_request("test_notification", args);
        let request_msg = build_jsonrpc_request(request, "test-notification-bridge");
        client_tx.send(request_msg).await.unwrap();

        let mut saw_notification = false;
        let mut saw_response = false;
        // The notification and the call_tool response can arrive in either
        // order; drain both messages off the channel.
        for _ in 0..2 {
            match client_rx.recv().await {
                Some(TxJsonRpcMessage::<RoleServer>::Notification(_)) => {
                    saw_notification = true;
                }
                Some(TxJsonRpcMessage::<RoleServer>::Response(json_response)) => {
                    saw_response = true;
                    let result_json = serde_json::to_value(&json_response.result)
                        .expect("call_tool result must serialize");
                    assert_ne!(
                        result_json.get("isError"),
                        Some(&serde_json::Value::Bool(true)),
                        "test_notification call must not report an error"
                    );
                }
                other => panic!(
                    "Unexpected message while awaiting notification+response: {:?}",
                    other
                ),
            }
        }
        assert!(
            saw_notification,
            "test_notification must emit a client-observable notification"
        );
        assert!(
            saw_response,
            "test_notification's call_tool request must still receive a response"
        );
    })
    .await;
}
