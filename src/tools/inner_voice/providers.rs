//! Provider helpers for inner_voice

use std::time::Duration;

use reqwest::Client;
use serde_json::Value;

use crate::error::{Result, SurrealMindError};
use crate::schemas::Snippet;

use super::check_http_status;

pub async fn grok_call(base: &str, model: &str, api_key: &str, messages: &Value) -> Result<String> {
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": 0.2,
        "max_tokens": 400
    });
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| SurrealMindError::Internal {
            message: format!("Failed to build HTTP client: {}", e),
        })?;
    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| SurrealMindError::Internal {
            message: e.to_string(),
        })?;

    // Check response status before parsing
    let status = resp.status();
    if !status.is_success() {
        let body_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        check_http_status(status.as_u16(), &body_text, "Grok synthesis")?;
        unreachable!(); // check_http_status always returns an error for non-success
    }

    let val: serde_json::Value = resp.json().await.map_err(|e| SurrealMindError::Internal {
        message: e.to_string(),
    })?;
    if let Some(choice) = val.get("choices").and_then(|c| c.get(0)) {
        if let Some(content) = choice
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
        {
            return Ok(content.trim().to_string());
        }
    }
    // Fallback: return the raw JSON if format unexpected
    Ok(val.to_string())
}

pub fn allow_grok() -> bool {
    std::env::var("IV_ALLOW_GROK").unwrap_or_else(|_| "true".to_string()) != "false"
}

pub fn allow_grok_from(iv_allow: Option<&str>) -> bool {
    iv_allow.unwrap_or("true") != "false"
}

pub fn fallback_from_snippets(snippets: &[Snippet]) -> String {
    if !snippets.is_empty() {
        let joined = snippets
            .iter()
            .take(3)
            .map(|s| s.text.trim())
            .collect::<Vec<_>>()
            .join(" ");
        let summary: String = joined.chars().take(440).collect();
        format!("Based on what I could find: {}", summary)
    } else {
        "Based on what I could find, there wasn’t enough directly relevant material in the corpus to answer confidently.".to_string()
    }
}

pub fn compute_auto_extract(params_auto: Option<bool>, default_auto: bool) -> bool {
    params_auto.unwrap_or(default_auto)
}
