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

// OpenAI API implementation
pub struct OpenAIEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dims: usize,
    retries: u32,
}

#[derive(Serialize)]
struct OpenAIRequest<'a> {
    model: &'a str,
    input: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

#[derive(Deserialize)]
struct OpenAIResponseData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    data: Vec<OpenAIResponseData>,
}

impl OpenAIEmbedder {
    pub fn new(api_key: String, model: String, dims: Option<usize>, retries: u32) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent(format!(
                "surreal-mind/{} (component=embeddings; provider=openai)",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .context("Failed to build reqwest client with timeout")?;

        let dims = dims.unwrap_or(match model.as_str() {
            // Known OpenAI embedding dims
            "text-embedding-3-small" => 1536,
            "text-embedding-3-large" => 3072,
            _ => 1536, // sensible default; can be overridden via SURR_EMBED_DIM
        });

        Ok(Self {
            client,
            api_key,
            model,
            dims,
            retries,
        })
    }
}

#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        debug!(
            "Generating OpenAI embedding (model={}, chars={})",
            self.model,
            text.len()
        );

        let body = OpenAIRequest {
            model: &self.model,
            input: text,
            dimensions: if self.dims != 1536 && self.dims != 3072 {
                Some(self.dims)
            } else {
                None // Use default for standard sizes
            },
        };

        // Retry with simple exponential backoff
        let mut last_err: Option<anyhow::Error> = None;
        let attempts = self.retries;
        for i in 0..attempts {
            let send_res = self
                .client
                .post("https://api.openai.com/v1/embeddings")
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
                .await
                .context("Failed to send request to OpenAI API");
            let response = match send_res {
                Ok(resp) => resp,
                Err(e) => {
                    last_err = Some(e);
                    let delay_ms = 200u64 * (1u64 << i);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    continue;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                last_err = Some(anyhow::anyhow!(
                    "OpenAI API error {}: {}",
                    status,
                    error_text
                ));
                let delay_ms = 200u64 * (1u64 << i);
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                continue;
            }

            let parse_res: Result<OpenAIResponse> = response
                .json()
                .await
                .context("Failed to parse OpenAI response");
            match parse_res {
                Ok(result) => {
                    return result
                        .data
                        .into_iter()
                        .next()
                        .map(|d| d.embedding)
                        .context("No embedding returned from OpenAI");
                }
                Err(e) => {
                    last_err = Some(e);
                    let delay_ms = 200u64 * (1u64 << i);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Unknown OpenAI embedding error")))
    }

    fn dimensions(&self) -> usize {
        self.dims
    }
}

// No per-call fallback wrapper. Selection happens at startup to avoid mixed dims.

// Factory function to create embedder based on configuration
pub async fn create_embedder(config: &crate::config::Config) -> Result<Arc<dyn Embedder>> {
    // Load .env file if it exists
    let _ = dotenvy::dotenv();

    // Configuration: prefer OpenAI when key present; else Candle
    let provider = &config.system.embedding_provider;
    // Allow explicit dimension override for custom models
    let dim_override = Some(config.system.embedding_dimensions);

    // Helpers
    let is_placeholder = |s: &str| {
        let t = s.trim();
        t.is_empty()
            || t.contains("${")
            || t.eq_ignore_ascii_case("your-api-key-here")
            || t.eq_ignore_ascii_case("changeme")
    };

    // Helper to build BGE
    let make_bge = || -> Result<Arc<dyn Embedder>> {
        let b = crate::bge_embedder::BGEEmbedder::new()?;
        Ok(Arc::new(b))
    };

    match provider.as_str() {
        "candle" | "local" => {
            info!("Using Candle (local) BGE-small-en-v1.5 embeddings (384 dims)");
            make_bge()
        }
        "openai" | "" => {
            let key = config.runtime.openai_api_key.clone().unwrap_or_default();
            if !is_placeholder(&key) && !key.is_empty() {
                let model = config.system.embedding_model.clone();
                let dims = dim_override.or(Some(1536));
                info!(
                    "Using OpenAI embeddings (model={}, dims={})",
                    model,
                    dims.unwrap()
                );
                Ok(Arc::new(OpenAIEmbedder::new(
                    key,
                    model,
                    dims,
                    config.system.embed_retries,
                )?))
            } else {
                info!("OPENAI_API_KEY not set; using Candle BGE-small (384)");
                make_bge()
            }
        }
        _ => {
            // Unknown provider → try OpenAI if key exists, else Candle
            let key = config.runtime.openai_api_key.clone().unwrap_or_default();
            if !is_placeholder(&key) && !key.is_empty() {
                let model = config.system.embedding_model.clone();
                let dims = dim_override.or(Some(1536));
                info!(
                    "Using OpenAI embeddings (model={}, dims={})",
                    model,
                    dims.unwrap()
                );
                Ok(Arc::new(OpenAIEmbedder::new(
                    key,
                    model,
                    dims,
                    config.system.embed_retries,
                )?))
            } else {
                info!("Unknown provider and no OpenAI key; using Candle BGE-small (384)");
                make_bge()
            }
        }
    }
}
