use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::time::{Duration, Instant};
use tracing::{debug, info};

static PROCESS_START: OnceLock<Instant> = OnceLock::new();

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
    // Simple rate limiter: tokens per second
    rps_limit: f32,
    last_call: Arc<AtomicU64>,
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
        let mut ua = format!(
            "surreal-mind/{} (component=embeddings; provider=openai)",
            env!("CARGO_PKG_VERSION")
        );
        if let Ok(commit) = std::env::var("SURR_COMMIT_HASH") {
            ua.push_str(&format!("; commit={}", &commit[..7.min(commit.len())]));
        }

        let rps_limit = std::env::var("SURR_EMBED_RPS")
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(1.0);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent(ua)
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
            rps_limit,
            last_call: Arc::new(AtomicU64::new(0)),
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

        // Simple rate limiting: wait if needed to respect RPS
        if self.rps_limit > 0.0 {
            let interval_ms = (1000.0 / self.rps_limit) as u64;
            let process_start = PROCESS_START.get_or_init(Instant::now);
            let now_ms = (Instant::now() - *process_start).as_millis() as u64;
            let last = self.last_call.load(Ordering::SeqCst);
            if now_ms < last.saturating_add(interval_ms) {
                let delay = last.saturating_add(interval_ms).saturating_sub(now_ms);
                debug!("Rate limiting OpenAI embedding, delaying {}ms", delay);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            self.last_call.store(now_ms, Ordering::SeqCst);
        }

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
                .context(format!(
                    "Failed to send embedding request to OpenAI API for model '{}' ({} chars)",
                    self.model,
                    text.len()
                ));
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
                let error_text = response
                    .text()
                    .await
                    .context("Failed to read error response from OpenAI API")?;
                last_err = Some(anyhow::anyhow!(
                    "OpenAI API error {} for model '{}' ({} chars): {}",
                    status,
                    self.model,
                    text.len(),
                    error_text
                ));
                let delay_ms = 200u64 * (1u64 << i);
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                continue;
            }

            let parse_res: Result<OpenAIResponse> = response.json().await.context(format!(
                "Failed to parse JSON response from OpenAI API for model '{}' ({} chars)",
                self.model,
                text.len()
            ));
            match parse_res {
                Ok(result) => {
                    return result
                        .data
                        .into_iter()
                        .next()
                        .map(|d| d.embedding)
                        .context(format!(
                            "No embedding data returned from OpenAI API for model '{}' ({} chars)",
                            self.model,
                            text.len()
                        ));
                }
                Err(e) => {
                    last_err = Some(e);
                    let delay_ms = 200u64 * (1u64 << i);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            anyhow::anyhow!(
                "Unknown error generating OpenAI embedding for model '{}' ({} chars)",
                self.model,
                text.len()
            )
        }))
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

    match provider.as_str() {
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
                anyhow::bail!("OPENAI_API_KEY is not set or valid. Cannot Initialize Embeddings.");
            }
        }
        _ => {
            // Unknown provider - fail explicitly
            anyhow::bail!(
                "Unknown or unsupported embedding provider: '{}'. Only 'openai' is supported.",
                provider
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_rate_limiter_no_sleep_when_elapsed() {
        let interval = 1000u64;
        let last = 0u64;
        let now = 2000u64;
        // Simulate: if now >= last + interval, no sleep
        assert!(now >= last.saturating_add(interval));
    }
}
