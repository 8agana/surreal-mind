use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn dimensions(&self) -> usize;
}

// Nomic API implementation
pub struct NomicEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct NomicRequest {
    texts: Vec<String>,
    model: String,
    task_type: String,
}

#[derive(Deserialize)]
struct NomicResponse {
    embeddings: Vec<Vec<f32>>,
}

impl NomicEmbedder {
    pub fn new(api_key: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .context("Failed to build reqwest client with timeout")?;
        Ok(Self {
            client,
            api_key,
            model: "nomic-embed-text-v1.5".to_string(),
        })
    }
}

#[async_trait]
impl Embedder for NomicEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        debug!("Generating Nomic embedding for text: {} chars", text.len());

        let request = NomicRequest {
            texts: vec![text.to_string()],
            model: self.model.clone(),
            task_type: "search_document".to_string(),
        };

        // Simple retry with exponential backoff
        let mut last_err: Option<anyhow::Error> = None;
        let attempts = std::env::var("SURR_EMBED_RETRIES")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&n| n > 0 && n <= 5)
            .unwrap_or(3);
        for i in 0..attempts {
            let send_res = self
                .client
                .post("https://api-atlas.nomic.ai/v1/embedding/text")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request)
                .send()
                .await
                .context("Failed to send request to Nomic API");
            let response = match send_res {
                Ok(resp) => resp,
                Err(e) => {
                    last_err = Some(e);
                    // backoff then retry
                    let delay_ms = 200u64 * (1u64 << i);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    continue;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                last_err = Some(anyhow::anyhow!(
                    "Nomic API error {}: {}",
                    status,
                    error_text
                ));
                let delay_ms = 200u64 * (1u64 << i);
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                continue;
            }

            let parse_res: Result<NomicResponse> = response
                .json()
                .await
                .context("Failed to parse Nomic response");
            match parse_res {
                Ok(result) => {
                    return result
                        .embeddings
                        .into_iter()
                        .next()
                        .context("No embedding returned from Nomic");
                }
                Err(e) => {
                    last_err = Some(e);
                    let delay_ms = 200u64 * (1u64 << i);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Unknown Nomic embedding error")))
    }

    fn dimensions(&self) -> usize {
        768 // Nomic uses 768 dimensions
    }
}

// Fallback implementation for testing
pub struct FakeEmbedder {
    dimensions: usize,
}

impl FakeEmbedder {
    pub fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }
}

#[async_trait]
impl Embedder for FakeEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Generate deterministic fake embedding based on text length
        let seed = text.len() as f32 / 100.0;
        Ok(vec![seed; self.dimensions])
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

// Factory function to create embedder based on environment
pub async fn create_embedder() -> Result<Arc<dyn Embedder>> {
    // Treat empty or placeholder-like values as "not set"
    let api_key = std::env::var("NOMIC_API_KEY").ok();
    let strict = std::env::var("SURR_EMBED_STRICT")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let is_placeholder = |s: &str| {
        let t = s.trim();
        t.is_empty()
            || t.contains("${")
            || t.eq_ignore_ascii_case("your-nomic-api-key-here")
            || t.eq_ignore_ascii_case("your-api-key-here")
            || t.eq_ignore_ascii_case("changeme")
    };

    if let Some(key) = api_key.as_deref()
        && !is_placeholder(key)
    {
        info!("Using Nomic API for embeddings");
        return Ok(Arc::new(NomicEmbedder::new(key.to_string())?));
    }

    if strict {
        anyhow::bail!(
            "SURR_EMBED_STRICT is set but NOMIC_API_KEY is missing/invalid; refusing to use fake embeddings"
        );
    }

    info!("No valid NOMIC_API_KEY found, using fake embeddings for testing");
    Ok(Arc::new(FakeEmbedder::new(768)))
}
