//! Curiosity tools: lightweight add/get/search over curiosity entries.

use crate::error::{Result, SurrealMindError};
use crate::schemas::Snippet;
use crate::server::SurrealMindServer;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct CuriosityAddParams {
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub in_reply_to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CuriosityGetParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub since: Option<String>, // YYYY-MM-DD
}

#[derive(Debug, Deserialize)]
pub struct CuriositySearchParams {
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: i64,
    #[serde(default)]
    pub recency_days: Option<i64>,
}

fn default_limit() -> i64 {
    20
}
fn default_top_k() -> i64 {
    10
}

impl SurrealMindServer {
    pub async fn handle_curiosity_add(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request
            .arguments
            .ok_or_else(|| SurrealMindError::InvalidParams {
                message: "Missing parameters".into(),
            })?;
        let params: CuriosityAddParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        if params.content.trim().is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "content cannot be empty".into(),
            });
        }

        // Embed content
        let embedding = self.embedder.embed(&params.content).await.map_err(|e| {
            SurrealMindError::Embedding {
                message: e.to_string(),
            }
        })?;
        let (provider, model, dim) = self.get_embedding_metadata();

        let created: Vec<serde_json::Value> = self
            .db
            .query(
                "CREATE curiosity_entries SET
                    content = $content,
                    tags = $tags,
                    agent = $agent,
                    topic = $topic,
                    in_reply_to = $in_reply_to,
                    created_at = time::now(),
                    embedding = $embedding,
                    embedding_provider = $provider,
                    embedding_model = $model,
                    embedding_dim = $dim
                 RETURN meta::id(id) as id",
            )
            .bind(("content", params.content))
            .bind(("tags", params.tags))
            .bind(("agent", params.agent))
            .bind(("topic", params.topic))
            .bind(("in_reply_to", params.in_reply_to))
            .bind(("embedding", embedding))
            .bind(("provider", provider))
            .bind(("model", model))
            .bind(("dim", dim))
            .await?
            .take(0)?;

        let id = created
            .first()
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| SurrealMindError::Internal {
                message: "Failed to create curiosity entry".into(),
            })?;

        Ok(CallToolResult::structured(json!({ "id": id })))
    }

    pub async fn handle_curiosity_get(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request
            .arguments
            .ok_or_else(|| SurrealMindError::InvalidParams {
                message: "Missing parameters".into(),
            })?;
        let params: CuriosityGetParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        let limit = params.limit.clamp(1, 100) as i64;
        let mut sql = "SELECT meta::id(id) as id, content, tags ?? [] AS tags, agent, topic, in_reply_to, created_at FROM curiosity_entries".to_string();
        if params.since.is_some() {
            sql.push_str(" WHERE created_at >= $since_date");
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT $limit");

        let mut query = self.db.query(&sql).bind(("limit", limit));
        if let Some(since) = params.since {
            let since_dt = format!("{}T00:00:00Z", since);
            query = query.bind(("since_date", since_dt));
        }

        let rows: Vec<serde_json::Value> = query.await?.take(0)?;
        Ok(CallToolResult::structured(json!({ "entries": rows })))
    }

    pub async fn handle_curiosity_search(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult> {
        let args = request
            .arguments
            .ok_or_else(|| SurrealMindError::InvalidParams {
                message: "Missing parameters".into(),
            })?;
        let params: CuriositySearchParams = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| SurrealMindError::InvalidParams {
                message: format!("Invalid parameters: {}", e),
            })?;

        if params.query.trim().is_empty() {
            return Err(SurrealMindError::InvalidParams {
                message: "query cannot be empty".into(),
            });
        }

        let q_emb = self.embedder.embed(&params.query).await.map_err(|e| {
            SurrealMindError::EmbedderUnavailable {
                message: e.to_string(),
            }
        })?;
        let q_dim = q_emb.len() as i64;

        let top_k = params.top_k.clamp(1, 50);

        let mut sql = "SELECT meta::id(id) as id, content, tags ?? [] AS tags, agent, topic, in_reply_to, created_at, vector::similarity::cosine(embedding, $q) as score FROM curiosity_entries WHERE array::len(embedding) = $dim".to_string();
        if let Some(_days) = params.recency_days {
            sql.push_str(" AND created_at >= time::now() - duration::from::days($days)");
        }
        sql.push_str(" ORDER BY score DESC LIMIT $k");

        let rows: Vec<serde_json::Value> = self
            .db
            .query(&sql)
            .bind(("q", q_emb))
            .bind(("dim", q_dim))
            .bind(("days", params.recency_days.unwrap_or(0)))
            .bind(("k", top_k))
            .await?
            .take(0)?;

        // Map to Snippet-like structure for convenience
        let snippets: Vec<Snippet> = rows
            .iter()
            .filter_map(|r| {
                let id = r.get("id")?.as_str()?.to_string();
                let content = r.get("content")?.as_str()?.to_string();
                let created_at = r.get("created_at")?.to_string();
                let score = r.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                Some(Snippet {
                    id,
                    table: "curiosity_entries".into(),
                    source_type: "curiosity".into(),
                    origin: r
                        .get("agent")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .into(),
                    trust_tier: "amber".into(),
                    created_at,
                    text: content,
                    score,
                    content_hash: String::new(),
                    span_start: None,
                    span_end: None,
                })
            })
            .collect();

        Ok(CallToolResult::structured(
            json!({ "results": rows, "snippets": snippets }),
        ))
    }
}
