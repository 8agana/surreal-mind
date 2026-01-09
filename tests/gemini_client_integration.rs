#[allow(unused_imports)]
use anyhow::Result;
use surreal_mind::clients::GeminiClient;
use surreal_mind::clients::traits::CognitiveAgent;

#[tokio::test]
#[cfg(feature = "db_integration")]
async fn test_gemini_client_call() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    if std::env::var("RUN_GEMINI_TESTS").is_err() {
        eprintln!("Skipping Gemini integration test - set RUN_GEMINI_TESTS=1 to run");
        return Ok(());
    }

    let client = GeminiClient::new();
    let response = client
        .call(
            "Give me a one-word answer. The word should be 'test'.",
            None,
        )
        .await?;

    assert!(response.response.to_lowercase().contains("test"));
    assert!(!response.session_id.is_empty());

    println!("Response: {}", response.response);
    println!("Session ID: {}", response.session_id);

    Ok(())
}
