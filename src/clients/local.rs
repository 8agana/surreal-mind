use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct LocalClient {
    endpoint: String,
    model: String,
    client: Client,
}

impl LocalClient {
    pub fn new() -> Self {
        let endpoint = env::var("SURR_SCALPEL_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:8111".to_string());
        
        // Ensure endpoint has the correct path if not provided
        let endpoint = if endpoint.ends_with("/v1/chat/completions") {
            endpoint
        } else {
            format!("{}/v1/chat/completions", endpoint.trim_end_matches('/'))
        };

        let model = env::var("SURR_SCALPEL_MODEL")
            .unwrap_or_else(|_| "NousResearch/Hermes-3-Llama-3.2-3B-GGUF".to_string());
            
        let timeout = env::var("SURR_SCALPEL_TIMEOUT_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(120000);

        let client = Client::builder()
            .timeout(Duration::from_millis(timeout))
            .build()
            .unwrap_or_default();

        Self {
            endpoint,
            model,
            client,
        }
    }

    pub async fn call(&self, task: &str, context: Option<Value>, system_prompt: &str) -> Result<String> {
        let mut user_content = format!("Task: {}\n", task);
        
        if let Some(ctx) = context {
            user_content.push_str("\nContext:\n");
            if let Ok(pretty) = serde_json::to_string_pretty(&ctx) {
                user_content.push_str(&pretty);
            } else {
                user_content.push_str(&ctx.to_string());
            }
        }

        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_content}
            ],
            "max_tokens": env::var("SURR_SCALPEL_MAX_TOKENS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(500),
            "temperature": 0.1
        });

        let res = self.client
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .context("Failed to connect to local model endpoint")?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("Local model returned error {}: {}", status, text);
        }

        let response_json: Value = res.json().await.context("Failed to parse local model response")?;
        
        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        Ok(content)
    }

    /// Call model with raw messages array (for agentic loop)
    pub async fn call_with_messages(&self, messages: &[Value]) -> Result<String> {
        let body = json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": env::var("SURR_SCALPEL_MAX_TOKENS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1000),
            "temperature": 0.1
        });

        let res = self.client
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .context("Failed to connect to local model endpoint")?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("Local model returned error {}: {}", status, text);
        }

        let response_json: Value = res.json().await.context("Failed to parse local model response")?;
        
        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        Ok(content)
    }
}
