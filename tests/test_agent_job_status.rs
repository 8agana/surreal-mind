//! Integration tests for agent_job_status tool
//!
//! These tests verify that the agent_job_status tool can properly deserialize
//! job records from SurrealDB without encountering enum serialization errors.

use serde_json::Value;
use surreal_mind::config::Config;
use surreal_mind::error::Result;
use surreal_mind::server::SurrealMindServer;
use tokio::sync::OnceCell;

static SERVER: OnceCell<SurrealMindServer> = OnceCell::const_new();

async fn get_server() -> Result<&'static SurrealMindServer> {
    SERVER
        .get_or_try_init(|| async {
            let config = Config::load().unwrap_or_default();
            SurrealMindServer::new(&config).await
        })
        .await
}

#[tokio::test]
async fn test_agent_job_status_deserialization() {
    let server = get_server().await.expect("Failed to initialize server");

    // Create a test job record directly in the database
    let job_id = uuid::Uuid::new_v4().to_string();
    let sql = "
        CREATE agent_jobs SET
            job_id = $job_id,
            tool_name = 'test_tool',
            agent_source = 'test',
            agent_instance = 'test',
            status = 'completed',
            prompt = 'test prompt',
            task_name = 'test_task',
            created_at = time::now(),
            started_at = time::now(),
            completed_at = time::now(),
            duration_ms = 1000
    ";

    server
        .db
        .query(sql)
        .bind(("job_id", job_id.clone()))
        .await
        .expect("Failed to create test job");

    // Test the agent_job_status functionality
    let result = server
        .handle_agent_job_status(rmcp::model::CallToolRequestParam {
            name: "agent_job_status".into(),
            arguments: Some(
                serde_json::json!({
                    "job_id": job_id
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await;

    assert!(result.is_ok(), "agent_job_status should succeed");
    let response = result.unwrap();

    // Verify the response contains expected fields
    let response_json = serde_json::to_value(&response.structured_content).unwrap();
    let response_value: Value = response_json;

    assert_eq!(response_value["job_id"], job_id);
    assert_eq!(response_value["status"], "completed");
    assert!(response_value["created_at"].is_string());
    assert!(response_value["started_at"].is_string());
    assert!(response_value["completed_at"].is_string());
    assert_eq!(response_value["duration_ms"], 1000);

    // Clean up
    let _ = server
        .db
        .query("DELETE agent_jobs WHERE job_id = $job_id")
        .bind(("job_id", job_id))
        .await;
}

#[tokio::test]
async fn test_agent_job_status_with_exchange_id() {
    let server = get_server().await.expect("Failed to initialize server");

    // Create a test exchange record first
    let exchange_sql = "
        CREATE agent_exchanges SET
            agent_source = 'test',
            agent_instance = 'test',
            prompt = 'test prompt',
            response = 'test response',
            tool_name = 'test_tool',
            session_id = 'test_session'
    ";

    let mut exchange_response = server
        .db
        .query(exchange_sql)
        .await
        .expect("Failed to create test exchange");

    let exchange_rows: Vec<serde_json::Value> = exchange_response.take(0).unwrap();
    let exchange_id = exchange_rows.first().unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Create a test job record with exchange_id
    let job_id = uuid::Uuid::new_v4().to_string();
    let job_sql = format!(
        "
        CREATE agent_jobs SET
            job_id = '{}',
            tool_name = 'test_tool',
            agent_source = 'test',
            agent_instance = 'test',
            status = 'completed',
            prompt = 'test prompt',
            task_name = 'test_task',
            exchange_id = {},  // This is a Record type that caused serialization issues
            created_at = time::now(),
            started_at = time::now(),
            completed_at = time::now(),
            duration_ms = 1000
        ",
        job_id, exchange_id
    );

    server
        .db
        .query(job_sql)
        .await
        .expect("Failed to create test job with exchange_id");

    // Test the agent_job_status functionality - this should NOT fail with serialization error
    let result = server
        .handle_agent_job_status(rmcp::model::CallToolRequestParam {
            name: "agent_job_status".into(),
            arguments: Some(
                serde_json::json!({
                    "job_id": job_id
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await;

    assert!(
        result.is_ok(),
        "agent_job_status should succeed even with exchange_id"
    );
    let response = result.unwrap();

    // Verify the response contains expected fields
    let response_json = serde_json::to_value(&response.structured_content).unwrap();
    let response_value: Value = response_json;

    assert_eq!(response_value["job_id"], job_id);
    assert_eq!(response_value["status"], "completed");
    // exchange_id should be present as a string (not cause serialization error)
    assert!(response_value["exchange_id"].is_string() || response_value["exchange_id"].is_null());

    // Clean up
    let _ = server
        .db
        .query("DELETE agent_jobs WHERE job_id = $job_id")
        .bind(("job_id", job_id))
        .await;
    let _ = server
        .db
        .query("DELETE agent_exchanges WHERE id = $exchange_id")
        .bind(("exchange_id", exchange_id))
        .await;
}
