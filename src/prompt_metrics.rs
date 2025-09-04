//! Prompt invocation tracking and metrics
//! 
//! This module handles recording and analyzing prompt usage patterns, helping the
//! system understand how its cognitive frameworks are performing and evolving.

use crate::error::Result;
use crate::prompts::Prompt;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

/// Outcome of a prompt invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PromptOutcome {
    Success,
    Error(String),
    Refusal,
}

impl std::fmt::Display for PromptOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::Error(e) => write!(f, "error:{}", e),
            Self::Refusal => write!(f, "refusal"),
        }
    }
}

/// Record of a single prompt invocation
#[derive(Debug, Serialize, Deserialize)]
pub struct PromptInvocation {
    pub id: Option<Thing>,
    pub prompt_id: String,
    pub version: String,
    pub tool: String,
    pub created_at: DateTime<Utc>,
    pub latency_ms: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub outcome: String,
    pub error_type: Option<String>,
    pub coverage_score: Option<f32>,
    pub notes: Option<String>,
}

impl PromptInvocation {
    /// Create a new invocation record
    pub fn new(
        prompt: &Prompt,
        tool: impl Into<String>,
        latency_ms: i64,
        tokens_in: i64,
        tokens_out: i64,
        outcome: PromptOutcome,
        coverage_score: Option<f32>,
        notes: Option<String>,
    ) -> Self {
        Self {
            id: None,
            prompt_id: prompt.id.clone(),
            version: prompt.version.clone(),
            tool: tool.into(),
            created_at: Utc::now(),
            latency_ms,
            tokens_in,
            tokens_out,
            outcome: outcome.to_string(),
            error_type: match outcome {
                PromptOutcome::Error(e) => Some(e),
                _ => None,
            },
            coverage_score,
            notes,
        }
    }
}

/// Metrics for analyzing prompt effectiveness
#[derive(Debug, Serialize)]
pub struct PromptMetrics {
    pub prompt_id: String,
    pub version: String,
    pub total_invocations: i64,
    pub success_rate: f32,
    pub refusal_rate: f32,
    pub error_rate: f32,
    pub avg_latency_ms: f32,
    pub avg_tokens_in: f32,
    pub avg_tokens_out: f32,
    pub avg_coverage: Option<f32>,
    pub common_errors: Vec<(String, i64)>,
}

impl crate::server::SurrealMindServer {
    /// Record a prompt invocation
    pub async fn record_prompt_invocation(&self, invocation: PromptInvocation) -> Result<Thing> {
        let created: PromptInvocation = self
            .db
            .create("prompt_invocations")
            .content(invocation)
            .await?
            .take().ok_or_else(|| crate::error::SurrealMindError::Internal {
                message: "Failed to get prompt invocation from response".into(),
            })?;
        
        Ok(created.id
            .ok_or_else(|| crate::error::SurrealMindError::Internal {
                message: "Failed to create prompt invocation record".into(),
            })?)
    }

    /// Get metrics for a specific prompt
    pub async fn get_prompt_metrics(&self, prompt_id: &str) -> Result<PromptMetrics> {
        // Get basic stats
        let stats: Vec<serde_json::Value> = self
            .db
            .query(
                "SELECT 
                    count() as total,
                    math::sum(IF outcome = 'success' THEN 1 ELSE 0 END) as successes,
                    math::sum(IF outcome = 'refusal' THEN 1 ELSE 0 END) as refusals,
                    math::sum(IF outcome CONTAINS 'error:' THEN 1 ELSE 0 END) as errors,
                    math::mean(latency_ms) as avg_latency,
                    math::mean(tokens_in) as avg_tokens_in,
                    math::mean(tokens_out) as avg_tokens_out,
                    math::mean(coverage_score) as avg_coverage
                FROM prompt_invocations 
                WHERE prompt_id = $id
                GROUP ALL",
            )
.bind(("id", prompt_id.to_string()))
            .await?
            .take(0)?;

        let stats = stats.first().ok_or_else(|| crate::error::SurrealMindError::Internal {
            message: "No stats returned for prompt".into(),
        })?;

        // Get common errors
        let errors: Vec<serde_json::Value> = self
            .db
            .query(
                "SELECT 
                    error_type,
                    count() as count
                FROM prompt_invocations
                WHERE prompt_id = $id 
                    AND error_type IS NOT NULL
                GROUP BY error_type
                ORDER BY count DESC
                LIMIT 5",
            )
.bind(("id", prompt_id.to_string()))
            .await?
            .take(0)?;

        let total = stats["total"].as_i64().unwrap_or(0);
        let successes = stats["successes"].as_i64().unwrap_or(0);
        let refusals = stats["refusals"].as_i64().unwrap_or(0);
        let errors_count = stats["errors"].as_i64().unwrap_or(0);

        // Safe division helper
        let safe_rate = |count: i64| {
            if total > 0 {
                count as f32 / total as f32
            } else {
                0.0
            }
        };

        Ok(PromptMetrics {
            prompt_id: prompt_id.to_string(),
            version: "1.0.0".to_string(), // TODO: Get from latest invocation
            total_invocations: total,
            success_rate: safe_rate(successes),
            refusal_rate: safe_rate(refusals),
            error_rate: safe_rate(errors_count),
            avg_latency_ms: stats["avg_latency"].as_f64().unwrap_or(0.0) as f32,
            avg_tokens_in: stats["avg_tokens_in"].as_f64().unwrap_or(0.0) as f32,
            avg_tokens_out: stats["avg_tokens_out"].as_f64().unwrap_or(0.0) as f32,
            avg_coverage: stats["avg_coverage"]
                .as_f64()
                .map(|v| v as f32),
            common_errors: errors
                .into_iter()
                .filter_map(|e| {
                    Some((
                        e["error_type"].as_str()?.to_string(),
                        e["count"].as_i64().unwrap_or(0),
                    ))
                })
                .collect(),
        })
    }
}

/// Initialize prompt_invocations table
pub async fn init_prompt_metrics(db: &surrealdb::Surreal<surrealdb::engine::remote::ws::Client>) -> Result<()> {
    use crate::prompts::PROMPT_INVOCATION_SCHEMA;
    
    db.query(PROMPT_INVOCATION_SCHEMA)
        .await
        .map_err(|e| crate::error::SurrealMindError::Database {
            message: format!("Failed to initialize prompt metrics: {}", e),
        })?;
    
    Ok(())
}
