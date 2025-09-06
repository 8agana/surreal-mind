use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod convo;

/// Generic envelope for framework analysis results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkEnvelope<T> {
    pub framework_version: String,
    pub methodology: String,
    pub data: T,
}

/// Data structure for convo-specific analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvoData {
    pub summary: String,
    pub takeaways: Vec<String>,
    pub prompts: Vec<String>,
    pub next_step: String,
    pub tags: Vec<String>,
}

/// Options for running the convo framework.
#[derive(Debug, Clone)]
pub struct ConvoOpts {
    pub strict_json: bool,
    pub tag_whitelist: Vec<String>,
    pub timeout_ms: u64,
}

/// Run the convo framework on the given content, returning an enhanced envelope.
pub async fn run_convo(content: &str, opts: &ConvoOpts) -> Result<FrameworkEnvelope<ConvoData>> {
    let content = content.to_string();
    let opts = opts.clone();
    tokio::task::spawn_blocking(move || convo::run_convo_impl(&content, &opts))
        .await
        .map_err(|_| anyhow::anyhow!("Framework task panicked"))?
}
