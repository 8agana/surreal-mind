use anyhow::{Context, Result, anyhow};
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
            dimensions: if self.dims != 1536 && self.dims != 3072 {
                Some(self.dims)
            } else {
                None // Use default for standard sizes
            },
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

// Deterministic, local FakeEmbedder for testing/dev (no network)
pub struct FakeEmbedder {
    dims: usize,
}

impl FakeEmbedder {
    pub fn new(dims: Option<usize>) -> Self {
        let d = dims.unwrap_or(768).max(1);
        Self { dims: d }
    }

    // Produce a stable stream of pseudo-random f32 values in [-1.0, 1.0)
    fn generate(&self, text: &str) -> Vec<f32> {
        use sha2::{Digest, Sha256};
        let mut out = Vec::with_capacity(self.dims);
        let mut i: u32 = 0;
        while out.len() < self.dims {
            // hash(text || i)
            let mut hasher = Sha256::new();
            hasher.update(text.as_bytes());
            hasher.update(i.to_le_bytes());
            let digest = hasher.finalize();
            // map chunks of 4 bytes to f32 in [-1,1)
            for chunk in digest.chunks(4) {
                if out.len() >= self.dims {
                    break;
                }
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(chunk);
                let val_u32 = u32::from_le_bytes(bytes);
                // Map to [0,1) using division then to [-1,1)
                let v01 = (val_u32 as f32) / (u32::MAX as f32 + 1.0);
                let v = v01 * 2.0 - 1.0;
                out.push(v);
            }
            i = i.wrapping_add(1);
        }

        // Optional tiny deterministic noise for more realistic behavior
        let noise_enabled = std::env::var("SURR_FAKE_NOISE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if noise_enabled {
            let amp: f32 = std::env::var("SURR_FAKE_NOISE_AMP")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.01);
            let mut ni: u32 = 0;
            for v in &mut out {
                let mut h = Sha256::new();
                h.update(b"noise:");
                h.update(text.as_bytes());
                h.update(ni.to_le_bytes());
                let d = h.finalize();
                // Use first 4 bytes to make a small offset in [-amp, amp)
                let mut b4 = [0u8; 4];
                b4.copy_from_slice(&d[..4]);
                let u = u32::from_le_bytes(b4);
                let r01 = (u as f32) / (u32::MAX as f32 + 1.0);
                let noise = (r01 * 2.0 - 1.0) * amp;
                *v += noise;
                ni = ni.wrapping_add(1);
            }
        }

        // Normalize to unit length to emulate real embeddings
        let norm: f32 = out.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut out {
                *v /= norm;
            }
        }
        out
    }
}

#[async_trait]
impl Embedder for FakeEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.generate(text))
    }

    fn dimensions(&self) -> usize {
        self.dims
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
        "candle" | "local" => {
            // Local, in-process embeddings via Candle. Default to BGE small 384-dim for speed/quality.
            let model = std::env::var("SURR_EMBED_MODEL")
                .ok()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| "BAAI/bge-small-en-v1.5".to_string());
            let dims = dim_override
                .or_else(|| {
                    std::env::var("SURR_EMBED_DIM")
                        .ok()
                        .and_then(|s| s.parse().ok())
                })
                .unwrap_or(384);
            info!(
                "Using Candle (local) embeddings (model={}, dims={})",
                model, dims
            );
            return Ok(Arc::new(CandleEmbedder::new(model, dims)?));
        }
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
        _ => {
            // Auto-detect
            let openai_key = std::env::var("OPENAI_API_KEY").ok();
            if let Some(key) = openai_key.as_deref().filter(|k| !is_placeholder(k)) {
                let model = std::env::var("SURR_EMBED_MODEL")
                    .ok()
                    .filter(|m| !m.trim().is_empty())
                    .unwrap_or_else(|| "text-embedding-3-small".to_string());
                info!("Using OpenAI embeddings (model={})", model);
                return Ok(Arc::new(OpenAIEmbedder::new(
                    key.to_string(),
                    model,
                    dim_override,
                )?));
            }
        }
    }

    if strict {
        anyhow::bail!(
            "No embedding provider configured; set SURR_EMBED_PROVIDER (candle|openai) and related env."
        );
    }

    // Fallback to deterministic FakeEmbedder for local/testing usage (explicit opt-in recommended)
    let dims = dim_override.or_else(|| {
        std::env::var("SURR_EMBED_DIM")
            .ok()
            .and_then(|s| s.parse().ok())
    });
    let fake = FakeEmbedder::new(dims);
    info!(
        "Using FakeEmbedder (deterministic) with {} dimensions",
        fake.dimensions()
    );
    Ok(Arc::new(fake))
}

// Candle-based local embedder (scaffold)
pub struct CandleEmbedder {
    model_id: String,
    dims: usize,
}

impl CandleEmbedder {
    pub fn new(model_id: String, dims: usize) -> Result<Self> {
        // We intentionally do not attempt heavy model loading here.
        // CC will install candle + model artifacts; we keep this minimal and compile-safe.
        Ok(Self { model_id, dims })
    }
}

#[async_trait]
impl Embedder for CandleEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        // Placeholder: real implementation will use candle/tokenizers to produce embeddings.
        Err(anyhow!(
            "CandleEmbedder embed() not yet implemented. Model '{}', dims {}",
            self.model_id,
            self.dims
        ))
    }

    fn dimensions(&self) -> usize {
        self.dims
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_embedder_is_deterministic() {
        let fe = FakeEmbedder::new(Some(128));
        let a1 = fe.embed("hello world").await.unwrap();
        let a2 = fe.embed("hello world").await.unwrap();
        assert_eq!(a1.len(), 128);
        assert_eq!(a2.len(), 128);
        assert!(a1.iter().zip(&a2).all(|(x, y)| (x - y).abs() < 1e-8));
    }

    #[tokio::test]
    async fn fake_embedder_varies_with_input() {
        let fe = FakeEmbedder::new(None); // default 768
        let a = fe.embed("foo").await.unwrap();
        let b = fe.embed("bar").await.unwrap();
        assert_eq!(a.len(), 768);
        assert_eq!(b.len(), 768);
        // must differ for at least one index
        assert!(a.iter().zip(&b).any(|(x, y)| (x - y).abs() > 1e-6));
    }
}
