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
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model: "nomic-embed-text-v1.5".to_string(),
        }
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

        let response = self
            .client
            .post("https://api-atlas.nomic.ai/v1/embedding/text")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Nomic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Nomic API error {}: {}", status, error_text);
        }

        let result: NomicResponse = response
            .json()
            .await
            .context("Failed to parse Nomic response")?;

        result
            .embeddings
            .into_iter()
            .next()
            .context("No embedding returned from Nomic")
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
    let is_placeholder = |s: &str| {
        let t = s.trim();
        t.is_empty()
            || t.contains("${")
            || t.eq_ignore_ascii_case("your-nomic-api-key-here")
            || t.eq_ignore_ascii_case("your-api-key-here")
            || t.eq_ignore_ascii_case("changeme")
    };

    if let Some(key) = api_key.as_deref() {
        if !is_placeholder(key) {
            info!("Using Nomic API for embeddings");
            return Ok(Arc::new(NomicEmbedder::new(key.to_string())));
        }
    }

    info!("No valid NOMIC_API_KEY found, using fake embeddings for testing");
    Ok(Arc::new(FakeEmbedder::new(768)))
}
