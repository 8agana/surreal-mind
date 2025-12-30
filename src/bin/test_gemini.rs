use anyhow::Result;
use surreal_mind::clients::traits::CognitiveAgent;
use surreal_mind::clients::GeminiClient;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let client = GeminiClient::new();
    let response = client.call("Hello", None).await?;

    println!("Response: {}", response.response);
    println!("Session ID: {}", response.session_id);

    Ok(())
}
