//! kg_consolidate - Execute deterministic KG consolidation from correction events
//!
//! This binary resolves duplicate kg_entities flagged via correction_events by:
//! 1) reading structured merge fields from pending correction state
//! 2) redirecting edges from loser -> winner
//! 3) marking loser as alias of winner
//! 4) writing non-empty new_state to the correction event
//!
//! Safety defaults:
//! - Dry-run supported via DRY_RUN=1
//! - Deletion disabled unless CONSOLIDATE_DELETE=1

use anyhow::{Context, Result};
use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;
use surreal_mind::config::Config;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client as WsClient, Ws};
use surrealdb::opt::auth::Root;

#[derive(Debug, Serialize)]
struct ItemResult {
    event_id: String,
    loser_id: Option<String>,
    winner_id: Option<String>,
    action: String,
    redirected_edges: i64,
    deleted: bool,
    note: String,
}

#[derive(Debug, Serialize)]
struct ConsolidationReport {
    run_timestamp: String,
    dry_run: bool,
    delete_enabled: bool,
    scanned: usize,
    applied: usize,
    ignored_non_merge: usize,
    skipped: usize,
    errors: usize,
    results: Vec<ItemResult>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let dry_run = std::env::var("DRY_RUN")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let delete_enabled = std::env::var("CONSOLIDATE_DELETE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let limit: i64 = std::env::var("CONSOLIDATE_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    println!(
        "🚀 Starting kg_consolidate (dry_run={}, delete_enabled={}, limit={})",
        dry_run, delete_enabled, limit
    );

    let config = Config::load().context("Failed to load configuration")?;

    let db = Surreal::new::<Ws>(&config.system.database_url).await?;
    db.signin(Root {
        username: config.runtime.database_user.clone(),
        password: config.runtime.database_pass.clone(),
    })
    .await?;
    db.use_ns(&config.system.database_ns)
        .use_db(&config.system.database_db)
        .await?;
    let db = Arc::new(db);

    let rows: Vec<Value> = db
        .query(
            "SELECT meta::id(id) as id, target_id, target_table, reasoning, new_state, previous_state \
             FROM correction_events \
             WHERE target_table = 'kg_entities' \
             LIMIT $limit",
        )
        .bind(("limit", limit))
        .await?
        .take(0)?;

    let mut report = ConsolidationReport {
        run_timestamp: chrono::Utc::now().to_rfc3339(),
        dry_run,
        delete_enabled,
        scanned: 0,
        applied: 0,
        ignored_non_merge: 0,
        skipped: 0,
        errors: 0,
        results: Vec::new(),
    };

    let mut processed_losers: HashSet<String> = HashSet::new();

    for row in rows {
        let event_id = row
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if event_id.is_empty() {
            continue;
        }

        let new_state = row.get("new_state").unwrap_or(&Value::Null);
        let previous_state = row.get("previous_state").unwrap_or(&Value::Null);

        let target_id_raw = row
            .get("target_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let reasoning = row
            .get("reasoning")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let structured_winner_id = parse_structured_merge_target(new_state);
        let legacy_winner_id = parse_merge_target(&reasoning);
        if !is_unresolved_correction(new_state, previous_state) {
            continue;
        }
        if structured_winner_id.is_none()
            && legacy_winner_id.is_none()
            && !reasoning_mentions_merge_intent(&reasoning)
        {
            report.ignored_non_merge += 1;
            continue;
        }

        report.scanned += 1;

        let loser_id = normalize_entity_id(&target_id_raw);
        if loser_id.is_empty() {
            report.skipped += 1;
            report.results.push(ItemResult {
                event_id,
                loser_id: None,
                winner_id: None,
                action: "skip".to_string(),
                redirected_edges: 0,
                deleted: false,
                note: "Missing/invalid target_id".to_string(),
            });
            continue;
        }

        if processed_losers.contains(&loser_id) {
            report.skipped += 1;
            report.results.push(ItemResult {
                event_id,
                loser_id: Some(loser_id),
                winner_id: None,
                action: "skip".to_string(),
                redirected_edges: 0,
                deleted: false,
                note: "Loser already processed in this run".to_string(),
            });
            continue;
        }

        if let Some(structured_loser_id) = parse_structured_entity_id(new_state, "loser_id")
            && structured_loser_id != loser_id
        {
            report.skipped += 1;
            report.results.push(ItemResult {
                event_id,
                loser_id: Some(loser_id),
                winner_id: None,
                action: "skip".to_string(),
                redirected_edges: 0,
                deleted: false,
                note: format!(
                    "Structured loser_id mismatch: new_state.loser_id={}",
                    structured_loser_id
                ),
            });
            continue;
        }

        let winner_id = structured_winner_id
            .or(legacy_winner_id)
            .unwrap_or_default();
        if winner_id.is_empty() {
            report.skipped += 1;
            report.results.push(ItemResult {
                event_id,
                loser_id: Some(loser_id),
                winner_id: None,
                action: "skip".to_string(),
                redirected_edges: 0,
                deleted: false,
                note: "No structured or parseable merge target".to_string(),
            });
            continue;
        }

        if loser_id == winner_id {
            report.skipped += 1;
            report.results.push(ItemResult {
                event_id,
                loser_id: Some(loser_id),
                winner_id: Some(winner_id),
                action: "skip".to_string(),
                redirected_edges: 0,
                deleted: false,
                note: "Loser and winner are identical".to_string(),
            });
            continue;
        }

        let loser_exists = entity_exists(db.clone(), &loser_id).await?;
        let winner_exists = entity_exists(db.clone(), &winner_id).await?;
        if !loser_exists || !winner_exists {
            report.skipped += 1;
            report.results.push(ItemResult {
                event_id,
                loser_id: Some(loser_id),
                winner_id: Some(winner_id),
                action: "skip".to_string(),
                redirected_edges: 0,
                deleted: false,
                note: "Loser or winner entity does not exist".to_string(),
            });
            continue;
        }

        let redirects_estimate = edge_count_for(db.clone(), &loser_id).await.unwrap_or(0);

        if dry_run {
            report.applied += 1;
            processed_losers.insert(loser_id.clone());
            report.results.push(ItemResult {
                event_id,
                loser_id: Some(loser_id),
                winner_id: Some(winner_id),
                action: "dry_run".to_string(),
                redirected_edges: redirects_estimate,
                deleted: false,
                note: "Would redirect edges, alias loser, and resolve event".to_string(),
            });
            continue;
        }

        let apply_result = apply_merge(
            db.clone(),
            &event_id,
            &loser_id,
            &winner_id,
            redirects_estimate,
            delete_enabled,
        )
        .await;

        match apply_result {
            Ok(deleted) => {
                report.applied += 1;
                processed_losers.insert(loser_id.clone());
                report.results.push(ItemResult {
                    event_id,
                    loser_id: Some(loser_id),
                    winner_id: Some(winner_id),
                    action: "applied".to_string(),
                    redirected_edges: redirects_estimate,
                    deleted,
                    note: if deleted {
                        "Merge applied and loser deleted safely".to_string()
                    } else {
                        "Merge applied and loser marked as alias".to_string()
                    },
                });
            }
            Err(e) => {
                report.errors += 1;
                report.results.push(ItemResult {
                    event_id,
                    loser_id: Some(loser_id),
                    winner_id: Some(winner_id),
                    action: "error".to_string(),
                    redirected_edges: 0,
                    deleted: false,
                    note: e.to_string(),
                });
            }
        }
    }

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn normalize_entity_id(raw: &str) -> String {
    let cleaned = raw.trim().trim_matches(|c: char| c == '"' || c == '\'');
    if let Some((prefix, id)) = cleaned.split_once(':')
        && matches!(prefix, "kg_entities" | "entity")
    {
        return sanitize_id(id);
    }
    sanitize_id(cleaned)
}

fn sanitize_id(s: &str) -> String {
    s.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
        .to_string()
}

fn is_unresolved_correction(new_state: &Value, previous_state: &Value) -> bool {
    if new_state.is_null()
        || (new_state.is_object() && new_state.as_object().unwrap().is_empty())
        || (!new_state.is_null() && new_state == previous_state)
    {
        return true;
    }

    new_state
        .get("status")
        .and_then(|v| v.as_str())
        .map(|status| matches!(status, "pending" | "planned" | "queued"))
        .unwrap_or(false)
}

fn parse_structured_merge_target(new_state: &Value) -> Option<String> {
    if new_state
        .get("mode")
        .and_then(|v| v.as_str())
        .is_some_and(|mode| mode != "merge_alias")
    {
        return None;
    }

    ["winner_id", "canonical_id", "merge_target"]
        .iter()
        .find_map(|key| parse_structured_entity_id(new_state, key))
}

fn parse_structured_entity_id(new_state: &Value, key: &str) -> Option<String> {
    new_state.get(key).and_then(|v| v.as_str()).and_then(|raw| {
        let id = normalize_entity_id(raw);
        (!id.is_empty()).then_some(id)
    })
}

fn parse_merge_target(reasoning: &str) -> Option<String> {
    let merge_target_re = Regex::new(r"(?i)merge\s+target:\s*([A-Za-z0-9_:\-]+)").ok()?;
    let merging_into_re = Regex::new(r"(?i)merging\s+into\s*([A-Za-z0-9_:\-]+)").ok()?;
    let bracket_re = Regex::new(r"\[([A-Za-z0-9_:\-]{8,})\]").ok()?;

    if let Some(c) = merge_target_re.captures(reasoning)
        && let Some(m) = c.get(1)
    {
        let id = normalize_entity_id(m.as_str());
        if !id.is_empty() {
            return Some(id);
        }
    }

    if let Some(c) = merging_into_re.captures(reasoning)
        && let Some(m) = c.get(1)
    {
        let id = normalize_entity_id(m.as_str());
        if !id.is_empty() {
            return Some(id);
        }
    }

    if let Some(c) = bracket_re.captures(reasoning)
        && let Some(m) = c.get(1)
    {
        let id = normalize_entity_id(m.as_str());
        if !id.is_empty() {
            return Some(id);
        }
    }

    None
}

fn reasoning_mentions_merge_intent(reasoning: &str) -> bool {
    let lower = reasoning.to_lowercase();
    lower.contains("merge") || lower.contains("duplicate") || lower.contains("canonical")
}

async fn entity_exists(db: Arc<Surreal<WsClient>>, id: &str) -> Result<bool> {
    let count: Option<i64> = db
        .query(
            "RETURN count((SELECT id FROM kg_entities WHERE id = type::record('kg_entities', $id)))",
        )
        .bind(("id", id.to_string()))
        .await?
        .take(0)?;
    Ok(count.unwrap_or(0) > 0)
}

async fn edge_count_for(db: Arc<Surreal<WsClient>>, ent_id: &str) -> Result<i64> {
    let src_rows: Vec<Value> = db
        .query("SELECT count() AS c FROM kg_edges WHERE source = type::record('kg_entities', $id) GROUP ALL")
        .bind(("id", ent_id.to_string()))
        .await?
        .take(0)?;
    let dst_rows: Vec<Value> = db
        .query("SELECT count() AS c FROM kg_edges WHERE target = type::record('kg_entities', $id) GROUP ALL")
        .bind(("id", ent_id.to_string()))
        .await?
        .take(0)?;
    let src = src_rows
        .first()
        .and_then(|v| v.get("c"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let dst = dst_rows
        .first()
        .and_then(|v| v.get("c"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    Ok(src + dst)
}

async fn apply_merge(
    db: Arc<Surreal<WsClient>>,
    event_id: &str,
    loser_id: &str,
    winner_id: &str,
    redirected_edges: i64,
    delete_enabled: bool,
) -> Result<bool> {
    db.query(
        "UPDATE kg_edges SET source = type::record('kg_entities', $toid) \
         WHERE source = type::record('kg_entities', $fromid)",
    )
    .bind(("toid", winner_id.to_string()))
    .bind(("fromid", loser_id.to_string()))
    .await?;

    db.query(
        "UPDATE kg_edges SET target = type::record('kg_entities', $toid) \
         WHERE target = type::record('kg_entities', $fromid)",
    )
    .bind(("toid", winner_id.to_string()))
    .bind(("fromid", loser_id.to_string()))
    .await?;

    db.query(
        "UPDATE type::record('kg_entities', $id) \
         SET data.canonical_id = $cid, data.is_alias = true",
    )
    .bind(("id", loser_id.to_string()))
    .bind(("cid", winner_id.to_string()))
    .await?;

    let mut deleted = false;
    if delete_enabled {
        let remaining = edge_count_for(db.clone(), loser_id).await.unwrap_or(1);
        if remaining == 0 {
            db.query("DELETE type::record('kg_entities', $id)")
                .bind(("id", loser_id.to_string()))
                .await?;
            deleted = true;
        }
    }

    let new_state = json!({
        "status": "resolved",
        "mode": "merge_alias",
        "loser_id": loser_id,
        "winner_id": winner_id,
        "edge_redirects": redirected_edges,
        "deleted": deleted,
        "resolved_at": chrono::Utc::now().to_rfc3339()
    });

    db.query(
        "UPDATE type::record('correction_events', $id) \
         SET new_state = $state, verification_status = 'auto_applied'",
    )
    .bind(("id", event_id.to_string()))
    .bind(("state", new_state))
    .await?;

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_accepts_empty_equal_and_pending_states() {
        assert!(is_unresolved_correction(&Value::Null, &json!({})));
        assert!(is_unresolved_correction(
            &json!({}),
            &json!({"name": "old"})
        ));
        assert!(is_unresolved_correction(
            &json!({"name": "same"}),
            &json!({"name": "same"})
        ));
        assert!(is_unresolved_correction(
            &json!({"status": "pending", "mode": "merge_alias"}),
            &json!({"name": "old"})
        ));
        assert!(!is_unresolved_correction(
            &json!({"status": "resolved", "mode": "merge_alias"}),
            &json!({"name": "old"})
        ));
    }

    #[test]
    fn structured_merge_target_beats_reasoning_shape() {
        let state = json!({
            "status": "pending",
            "mode": "merge_alias",
            "loser_id": "kg_entities:loser123",
            "winner_id": "kg_entities:winner456"
        });

        assert_eq!(
            parse_structured_entity_id(&state, "loser_id").as_deref(),
            Some("loser123")
        );
        assert_eq!(
            parse_structured_merge_target(&state).as_deref(),
            Some("winner456")
        );
    }

    #[test]
    fn legacy_reasoning_parser_still_handles_existing_events() {
        assert_eq!(
            parse_merge_target("Merge target: kg_entities:abc_123").as_deref(),
            Some("abc_123")
        );
        assert_eq!(
            parse_merge_target("duplicate, merging into kg_entities:def-456").as_deref(),
            Some("def-456")
        );
        assert_eq!(
            parse_merge_target("winner [kg_entities:ghi789]").as_deref(),
            Some("ghi789")
        );
    }

    #[test]
    fn merge_intent_filter_ignores_general_corrections() {
        assert!(reasoning_mentions_merge_intent(
            "Duplicate of Task-5 project. Merge into project entity."
        ));
        assert!(!reasoning_mentions_merge_intent(
            "Testing correct mode - changing test_field value"
        ));
    }
}
