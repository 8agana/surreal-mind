//! nlq tool handler for natural language queries over thoughts

use crate::error::{Result, SurrealMindError};
use crate::server::SurrealMindServer;
use chrono::{Datelike, Duration, LocalResult, TimeZone, Utc};
use chrono_tz::Tz;
use regex::escape as rx_escape;
use rmcp::model::{CallToolRequestParam, CallToolResult};
use serde_json::{Value, json};
use std::collections::HashMap;

/// Parameters for the nlq tool
#[derive(Debug, serde::Deserialize)]
pub struct NlqParams {
    pub query: String,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub order: Option<String>,
}

const STOPWORDS: &[&str] = &["The", "This", "That", "What", "When", "Where"];

impl SurrealMindServer {
    /// Handle the nlq tool call
    pub async fn handle_nlq(&self, request: CallToolRequestParam) -> Result<CallToolResult> {
        let args = request.arguments.ok_or_else(|| SurrealMindError::Mcp {
            message: "Missing parameters".into(),
        })?;
        let params: NlqParams = serde_json::from_value(Value::Object(args)).map_err(|e| {
            SurrealMindError::Serialization {
                message: format!("Invalid parameters: {}", e),
            }
        })?;

        tracing::info!(
            "nlq called (query_len={}, when={})",
            params.query.len(),
            params.when.as_deref().unwrap_or("all")
        );

        // Extract entities
        let entities: Vec<String> = extract_entities(&params.query);
        tracing::debug!("Extracted entities: {:?}", entities);

        // Parse temporal
        let tz: Tz = self
            .config
            .nlq
            .timezone
            .parse()
            .unwrap_or_else(|_| "America/Chicago".parse().unwrap());
        let (from, to) = params
            .when
            .as_deref()
            .and_then(|w| {
                #[allow(clippy::redundant_closure)]
                parse_temporal(w, tz, || Utc::now())
            })
            .unwrap_or_else(|| {
                let now = Utc::now();
                (now - Duration::weeks(4), now) // default to last 4 weeks
            });
        tracing::debug!("Temporal window: {} to {}", from, to);

        // Build keyword regex
        let escaped: Vec<String> = entities
            .iter()
            .filter(|k| !STOPWORDS.contains(&k.as_str()))
            .take(self.config.nlq.max_keywords)
            .map(|k| rx_escape(k))
            .collect();
        let keyword_regex = if escaped.is_empty() {
            String::from(".*")
        } else {
            format!("(?i)({})", escaped.join("|"))
        };
        tracing::debug!("Keyword regex: {}", keyword_regex);

        // ORDER BY whitelist
        let order_clause = match params.order.as_deref() {
            Some("created_at_asc") => "ORDER BY created_at ASC",
            _ => "ORDER BY created_at DESC",
        };

        let dim = self.embedder.dimensions() as i64;
        let final_limit = params
            .limit
            .unwrap_or(self.config.nlq.default_limit)
            .clamp(1, self.config.nlq.max_limit);
        // Use larger candidate pool for ranking, then truncate to final_limit
        let sql_limit = std::cmp::min(
            self.config.retrieval.candidates,
            self.config.retrieval.db_limit,
        );

        // Build SQL
        let sql = format!(
            "SELECT meta::id(id) as id, content, embedding, created_at \
             FROM thoughts \
             WHERE array::len(embedding) = $dim \
               AND created_at >= $from AND created_at < $to \
               {} \
               AND (is_summary IS NONE OR is_summary != true) \
               AND (pipeline IS NONE OR pipeline != 'inner_voice') \
             {} \
             LIMIT $limit",
            if self.config.nlq.enable_keyword_filter {
                "AND content ~ $keyword_regex"
            } else {
                ""
            },
            order_clause
        );

        tracing::debug!("NLQ query: {}", sql);

        let mut query = self
            .db
            .query(&sql)
            .bind(("dim", dim))
            .bind(("from", from))
            .bind(("to", to));
        if self.config.nlq.enable_keyword_filter {
            query = query.bind(("keyword_regex", keyword_regex));
        }
        let rows: Vec<Value> = query.bind(("limit", sql_limit as i64)).await?.take(0)?;

        // Stage B: Compute similarity against NLQ query
        let query_embedding = self.embedder.embed(&params.query).await?;
        let mut scored_sources: Vec<_> = rows
            .iter()
            .filter_map(|row| {
                let embedding: Vec<f32> = row["embedding"]
                    .as_array()?
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .map(|v| v as f32)
                    .collect();
                if embedding.len() != query_embedding.len() {
                    return None;
                }
                let score = crate::server::SurrealMindServer::cosine_similarity(
                    &query_embedding,
                    &embedding,
                );
                Some((score, row))
            })
            .collect();

        // Sort by score descending and take final limit
        scored_sources.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored_sources.truncate(final_limit);

        // Build sources with actual scores
        let sources: Vec<_> = scored_sources
            .iter()
            .map(|(score, row)| {
                json!({
                    "id": row["id"],
                    "created_at": row["created_at"],
                    "score": score
                })
            })
            .collect();

        let answer = if sources.is_empty() {
            // Refuse if ungrounded
            String::from("I'm sorry, I couldn't find any relevant thoughts for that query.")
        } else {
            // Grounded synthesis: Extract snippets from top sources
            let top_sources = scored_sources.iter().take(3).collect::<Vec<_>>();
            let snippets: Vec<String> = top_sources
                .iter()
                .filter_map(|(_, row)| {
                    let content = row["content"].as_str()?;
                    let excerpt = if content.len() <= 50 {
                        format!("\"{}\"", content)
                    } else {
                        format!("\"{}...\"", &content[..50])
                    };
                    Some(excerpt)
                })
                .collect();
            if snippets.is_empty() {
                format!(
                    "Found {} relevant thoughts, but couldn't extract content.",
                    sources.len()
                )
            } else {
                format!(
                    "Based on relevant thoughts, here are key insights: {}",
                    snippets.join("; ")
                )
            }
        };

        // Debug metrics
        tracing::debug!(
            "NLQ metrics: window_from={}, window_to={}, final_limit={}, sql_limit={}, keywords_count={}, candidates_scanned={}, candidates_after_sql={}, sim_evaluated={}, returned={}",
            from,
            to,
            final_limit,
            sql_limit,
            entities.len(),
            sql_limit,
            rows.len(),
            scored_sources.len(),
            sources.len()
        );

        Ok(CallToolResult::structured(json!({
            "answer": answer,
            "sources": sources
        })))
    }
}

fn extract_entities(query: &str) -> Vec<String> {
    let aliases = HashMap::from([
        ("sam", "Sam Atagana"),
        ("cc", "Claude Code"),
        ("codex", "Codex"),
    ]);

    query
        .split_whitespace()
        .filter(|w| w.chars().next().is_some_and(|c| c.is_uppercase()))
        .filter(|w| !STOPWORDS.contains(w))
        .filter_map(|w| aliases.get(w.to_lowercase().as_str()))
        .map(|s| s.to_string())
        .collect()
}

fn parse_temporal(
    phrase: &str,
    tz: Tz,
    now_utc: impl Fn() -> chrono::DateTime<Utc>,
) -> Option<(chrono::DateTime<Utc>, chrono::DateTime<Utc>)> {
    let now_local = now_utc().with_timezone(&tz);

    let day_start = |d: chrono::NaiveDate| -> Option<chrono::DateTime<Utc>> {
        match tz.with_ymd_and_hms(d.year(), d.month(), d.day(), 0, 0, 0) {
            LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => Some(dt.with_timezone(&Utc)),
            LocalResult::None => None,
        }
    };

    let (start_local, end_local) = match phrase {
        "yesterday" => {
            let y = now_local.date_naive().pred_opt()?;
            (y, y.succ_opt()?)
        }
        "two weeks ago" => {
            let target = now_local - Duration::weeks(2);
            let d = target.date_naive();
            (d, d.succ_opt()?)
        }
        "this week" => {
            let start_week = now_local.date_naive()
                - chrono::Duration::days(now_local.weekday().num_days_from_monday() as i64);
            (start_week, start_week + chrono::Duration::weeks(1))
        }
        "last month" => {
            let first_this = now_local.date_naive().with_day(1)?;
            let first_last = first_this - chrono::Months::new(1);
            let last_last = first_this - chrono::Duration::days(1);
            (first_last, last_last.succ_opt()?)
        }
        "last week" => {
            let monday_this = now_local.date_naive()
                - chrono::Duration::days(now_local.weekday().num_days_from_monday() as i64);
            let monday_last = monday_this - chrono::Duration::weeks(1);
            (monday_last, monday_this)
        }
        _ => return None,
    };

    Some((day_start(start_local)?, day_start(end_local)?))
}
