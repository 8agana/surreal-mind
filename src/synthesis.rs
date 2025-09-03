use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Clone, Deserialize)]
pub struct SynthesisInput {
    pub prompt: String, // grounded prompt with snippets and schema instruction
    pub model_hint: Option<String>, // "pro" | "flash" | full id
}

#[derive(Debug, Clone, Deserialize)]
pub struct SynthesisOutput {
    pub answer: String,
    pub sources: Vec<Value>,
    pub provider_used: String,
    pub fallback_used: bool,
}

pub async fn synthesize_with_chain(
    cfg: &crate::config::SynthesisConfig,
    input: SynthesisInput,
) -> Result<SynthesisOutput> {
    // Ordered providers
    let mut last_err: Option<anyhow::Error> = None;
    for p in &cfg.providers {
        if p.starts_with("gemini_cli:") {
            let which = p.split(':').nth(1).unwrap_or("pro");
            let model = match which {
                "pro" => cfg.gemini_cli.pro_model.clone(),
                "flash" => cfg.gemini_cli.flash_model.clone(),
                other => other.to_string(),
            };
            match gemini_cli(&cfg.gemini_cli, &input.prompt, &model).await {
                Ok((answer, sources)) => {
                    return Ok(SynthesisOutput {
                        answer,
                        sources,
                        provider_used: format!("gemini_cli:{}", which),
                        fallback_used: last_err.is_some(),
                    })
                }
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            }
        } else if p == "groq" {
            match groq_http(&cfg.groq, &input.prompt).await {
                Ok((answer, sources)) => {
                    return Ok(SynthesisOutput {
                        answer,
                        sources,
                        provider_used: "groq".to_string(),
                        fallback_used: last_err.is_some(),
                    })
                }
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("No synthesis provider available")))
}

async fn gemini_cli(
    cfg: &crate::config::GeminiCliConfig,
    prompt: &str,
    model: &str,
) -> Result<(String, Vec<Value>)> {
    let mut cmd = Command::new(&cfg.path);
    cmd.args(["-m", model, "-p", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn().context("spawn gemini cli")?;
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(prompt.as_bytes())
            .await
            .context("write prompt to gemini stdin")?;
    }
    let timeout = tokio::time::Duration::from_millis(cfg.timeout_ms);
    let out = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .context("gemini cli timeout")?
        .context("gemini cli failed to run")?;
    if !out.status.success() {
        return Err(anyhow!("gemini exited non-zero: {}", out.status));
    }
    let mut stdout = out.stdout;
    if stdout.len() > cfg.max_output_bytes {
        stdout.truncate(cfg.max_output_bytes);
    }
    let text = String::from_utf8_lossy(&stdout);
    parse_answer_sources(&text)
}

async fn groq_http(cfg: &crate::config::GroqConfig, prompt: &str) -> Result<(String, Vec<Value>)> {
    let timeout = std::time::Duration::from_millis(cfg.timeout_ms);
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .context("build http client")?;
    // Reuse existing keys if set: prefer GROQ_API_KEY, else OPENAI_API_KEY
    let api_key = std::env::var("GROQ_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_default();
    if api_key.is_empty() {
        return Err(anyhow!("GROQ_API_KEY not set"));
    }
    let body = serde_json::json!({
        "model": cfg.model,
        "messages": [
            {"role": "system", "content": "You are a careful assistant. Only use provided snippets. Respond as JSON with keys: answer (string), sources (array). Refuse if not grounded."},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.2,
        "response_format": {"type":"json_object"}
    });
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .context("groq http send")?;
    if !resp.status().is_success() {
        return Err(anyhow!("groq error: {}", resp.text().await.unwrap_or_default()));
    }
    let v: Value = resp.json().await.context("parse groq response json")?;
    let text = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
    parse_answer_sources(text)
}

fn parse_answer_sources(text: &str) -> Result<(String, Vec<Value>)> {
    // Try to parse JSON object from the text; strip fences if present
    let trimmed = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```");
    let v: Value = serde_json::from_str(trimmed).context("parse provider JSON")?;
    let answer = v
        .get("answer")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let sources = v
        .get("sources")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    Ok((answer, sources))
}
