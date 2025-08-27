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
}

#[derive(Serialize)]
struct OpenAIRequest<'a> {
    model: &'a str,
    input: &'a str,
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
    pub fn new(api_key: String, model: String, dims: Option<usize>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
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
        };

        // Retry with simple exponential backoff
        let mut last_err: Option<anyhow::Error> = None;
        let attempts = std::env::var("SURR_EMBED_RETRIES")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&n| n > 0 && n <= 5)
            .unwrap_or(3);
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

// Factory function to create embedder based on environment
pub async fn create_embedder() -> Result<Arc<dyn Embedder>> {
    // Configuration: provider preference and keys
    let provider = std::env::var("SURR_EMBED_PROVIDER").unwrap_or_default();
    // Allow explicit dimension override for custom models
    let dim_override = std::env::var("SURR_EMBED_DIM")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());
    let is_true = |s: &str| s == "1" || s.eq_ignore_ascii_case("true");
    let strict = std::env::var("SURR_EMBED_STRICT").is_ok_and(|v| is_true(&v));

    // Helpers
    let is_placeholder = |s: &str| {
        let t = s.trim();
        t.is_empty()
            || t.contains("${")
            || t.eq_ignore_ascii_case("your-api-key-here")
            || t.eq_ignore_ascii_case("changeme")
    };

    // Provider selection order:
    // 1) Respect SURR_EMBED_PROVIDER if set
    // 2) Else prefer OpenAI if key set
    // 3) Else use Nomic if key set
    // 4) Else error (no fake embedder)

    match provider.as_str() {
        "openai" => {
            let key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
            if is_placeholder(&key) {
                anyhow::bail!("SURR_EMBED_PROVIDER=openai but OPENAI_API_KEY is not set");
            }
            let model = std::env::var("SURR_EMBED_MODEL")
                .ok()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| "text-embedding-3-small".to_string());
            info!("Using OpenAI embeddings (model={})", model);
            return Ok(Arc::new(OpenAIEmbedder::new(key, model, dim_override)?));
        }
        "nomic" => {
            let key = std::env::var("NOMIC_API_KEY").unwrap_or_default();
            if is_placeholder(&key) {
                anyhow::bail!("SURR_EMBED_PROVIDER=nomic but NOMIC_API_KEY is not set");
            }
            info!("Using Nomic API for embeddings");
            return Ok(Arc::new(NomicEmbedder::new(key)?));
        }
        "local" => {
            // Reserved for a future local embedder implementation
            anyhow::bail!(
                "SURR_EMBED_PROVIDER=local is not implemented yet. Please configure OPENAI_API_KEY or NOMIC_API_KEY."
            );
        }
        _ => {
            // Auto-detect
            let openai_key = std::env::var("OPENAI_API_KEY").ok();
            if let Some(key) = openai_key.as_deref().filter(|k| !is_placeholder(k)) {
                let model = std::env::var("SURR_EMBED_MODEL")
                    .ok()
                    .filter(|m| !m.trim().is_empty())
                    .unwrap_or_else(|| "text-embedding-3-small".to_string());
                info!("Using OpenAI embeddings (model={})", model);
                return Ok(Arc::new(OpenAIEmbedder::new(key.to_string(), model, dim_override)?));
            }
            let nomic_key = std::env::var("NOMIC_API_KEY").ok();
            if let Some(key) = nomic_key.as_deref().filter(|k| !is_placeholder(k)) {
                info!("Using Nomic API for embeddings");
                return Ok(Arc::new(NomicEmbedder::new(key.to_string())?));
            }
        }
    }

    if strict {
        anyhow::bail!(
            "No embedding provider configured; set OPENAI_API_KEY or NOMIC_API_KEY (or SURR_EMBED_PROVIDER)."
        );
    }

    anyhow::bail!(
        "No embedding provider configured. Please set OPENAI_API_KEY (preferred) or NOMIC_API_KEY."
    )
}
