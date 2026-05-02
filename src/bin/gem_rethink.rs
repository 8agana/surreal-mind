//! gem_rethink - minimal Gemini mark queue processor
//!
//! Iterates over records marked_for "gemini", logs actions, and (for now)
//! records a CorrectionEvent (no content mutation) then clears the mark.
//! This is a stopgap to unblock Phase 5 testing; deeper Gemini-driven
//! corrections/enrichments can be layered on later.

use anyhow::{Context, Result};
use regex::Regex;
use serde_json::{Value, json};
use std::sync::Arc;
use surreal_mind::config::Config;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client as WsClient, Ws};
use surrealdb::opt::auth::Root;

#[derive(Debug)]
struct RunStats {
    processed: usize,
    corrections: usize,
    skipped: usize,
    errors: usize,
}

#[derive(Debug)]
struct MarkItem {
    id: String,
    table: String,
    mark_type: String,
    mark_note: Option<String>,
    data: serde_json::Value,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env quietly
    let _ = dotenvy::dotenv();

    println!("🚀 Starting gem_rethink (minimal queue processor)");

    let dry_run = std::env::var("DRY_RUN")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if dry_run {
        println!("🔎 Dry run: no writes to DB");
    }

    let limit: i64 = std::env::var("GEM_RETHINK_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);

    // Load configuration
    let config = Config::load().context("Failed to load configuration")?;

    // Connect DB
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

    let mut stats = RunStats {
        processed: 0,
        corrections: 0,
        skipped: 0,
        errors: 0,
    };

    let items = fetch_marks(db.clone(), limit).await?;
    if items.is_empty() {
        println!("✅ Queue empty (marked_for = gemini)");
        print_report(&stats, &[]);
        return Ok(());
    }

    println!(
        "🔄 Processing {} marked items (limit {})",
        items.len(),
        limit
    );

    let mut errors = Vec::new();
    for item in items {
        stats.processed += 1;
        match process_item(db.clone(), &item, dry_run).await {
            Ok(()) => {
                if item.mark_type == "correction" {
                    stats.corrections += 1;
                } else {
                    stats.skipped += 1;
                }
            }
            Err(e) => {
                stats.errors += 1;
                errors.push(format!("{} ({}) - {}", item.id, item.mark_type, e));
            }
        }
    }

    print_report(&stats, &errors);
    Ok(())
}

async fn fetch_marks(db: Arc<Surreal<WsClient>>, limit: i64) -> Result<Vec<MarkItem>> {
    let mut query = "SELECT meta::id(id) as rid, meta::tb(id) as tb_name, mark_type, mark_note, <string>marked_at as marked_at, marked_by, content, name \
                     FROM thoughts, kg_entities, kg_observations \
                     WHERE marked_for = 'gemini' "
        .to_string();

    if let Ok(types) = std::env::var("RETHINK_TYPES") {
        let list: Vec<String> = types
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !list.is_empty() {
            let in_clause = list
                .iter()
                .map(|s| format!("'{}'", s.replace('\'', "")))
                .collect::<Vec<String>>()
                .join(",");
            query.push_str(&format!(" AND mark_type IN [{}]", in_clause));
        }
    }

    query.push_str(" ORDER BY marked_at ASC, rid ASC LIMIT $limit");

    let rows: Vec<serde_json::Value> = db.query(query).bind(("limit", limit)).await?.take(0)?;

    let mut items = Vec::new();
    for r in rows {
        let id = r
            .get("rid")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("row missing id"))?
            .to_string();
        let table = r
            .get("tb_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("row missing tb_name"))?
            .to_string();
        let mark_type = r
            .get("mark_type")
            .and_then(|v| v.as_str())
            .unwrap_or("correction")
            .to_string();
        let mark_note = r
            .get("mark_note")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        items.push(MarkItem {
            id,
            table,
            mark_type,
            mark_note,
            data: r,
        });
    }

    Ok(items)
}

async fn process_item(db: Arc<Surreal<WsClient>>, item: &MarkItem, dry_run: bool) -> Result<()> {
    match item.mark_type.as_str() {
        "correction" => handle_correction(db, item, dry_run).await,
        _ => {
            // For now, just clear the mark for non-correction types
            if !dry_run {
                clear_mark(db, item).await?;
            }
            Ok(())
        }
    }
}

async fn handle_correction(
    db: Arc<Surreal<WsClient>>,
    item: &MarkItem,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("(dry-run) would correct {}", item.id);
        return Ok(());
    }

    let reasoning = item
        .mark_note
        .clone()
        .unwrap_or_else(|| "No reasoning provided".to_string());
    let new_state = correction_new_state(item, &reasoning);

    let correction: Option<serde_json::Value> = db
        .query(
            "CREATE correction_events SET \
             target_id = $target_id, \
             target_table = $target_table, \
             previous_state = $previous_state, \
             new_state = $new_state, \
             initiated_by = 'gem_rethink', \
             reasoning = $reasoning, \
             sources = $sources, \
             verification_status = 'auto_applied', \
             corrects_previous = NONE, \
             spawned_by = NONE \
             RETURN { id: meta::id(id) }",
        )
        .bind(("target_id", item.id.clone()))
        .bind(("target_table", item.table.clone()))
        .bind(("previous_state", item.data.clone()))
        .bind(("new_state", new_state))
        .bind(("reasoning", reasoning))
        .bind(("sources", json!(["gem_rethink"])))
        .await?
        .take(0)?;

    if let Some(ev) = correction {
        println!(
            "✅ CorrectionEvent created for {} -> {}",
            item.id,
            ev.get("id").and_then(|v| v.as_str()).unwrap_or("unknown")
        );
    }

    clear_mark(db, item).await?;
    Ok(())
}

fn correction_new_state(item: &MarkItem, reasoning: &str) -> Value {
    if item.table != "kg_entities" {
        return json!({});
    }

    parse_merge_target(reasoning)
        .map(|winner_id| {
            json!({
                "status": "pending",
                "mode": "merge_alias",
                "loser_id": item.id,
                "winner_id": winner_id
            })
        })
        .unwrap_or_else(|| json!({}))
}

fn parse_merge_target(reasoning: &str) -> Option<String> {
    let merge_target_re = Regex::new(r"(?i)merge\s+target:\s*([A-Za-z0-9_:\-]+)").ok()?;
    let merging_into_re = Regex::new(r"(?i)merging\s+into\s*([A-Za-z0-9_:\-]+)").ok()?;
    let bracket_re = Regex::new(r"\[([A-Za-z0-9_:\-]{8,})\]").ok()?;

    for re in [&merge_target_re, &merging_into_re, &bracket_re] {
        if let Some(captures) = re.captures(reasoning)
            && let Some(m) = captures.get(1)
        {
            let id = normalize_entity_id(m.as_str());
            if !id.is_empty() {
                return Some(id);
            }
        }
    }

    None
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

async fn clear_mark(db: Arc<Surreal<WsClient>>, item: &MarkItem) -> Result<()> {
    let q = format!(
        "UPDATE {} SET marked_for = NONE, mark_type = NONE, mark_note = NONE, marked_at = NONE, marked_by = NONE WHERE id = type::record('{}', $id) RETURN NONE",
        item.table, item.table
    );
    db.query(q).bind(("id", item.id.clone())).await?;
    Ok(())
}

fn print_report(stats: &RunStats, errors: &[String]) {
    let report = json!({
        "run_timestamp": chrono::Utc::now().to_rfc3339(),
        "items_processed": stats.processed,
        "by_type": {
            "correction": stats.corrections,
            "skipped_other": stats.skipped,
        },
        "errors": errors,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correction_new_state_structures_kg_merge_target() {
        let item = MarkItem {
            id: "loser123".to_string(),
            table: "kg_entities".to_string(),
            mark_type: "correction".to_string(),
            mark_note: None,
            data: json!({"name": "Loser"}),
        };

        let state = correction_new_state(&item, "Merge target: kg_entities:winner456");

        assert_eq!(
            state.get("status").and_then(|v| v.as_str()),
            Some("pending")
        );
        assert_eq!(
            state.get("mode").and_then(|v| v.as_str()),
            Some("merge_alias")
        );
        assert_eq!(
            state.get("loser_id").and_then(|v| v.as_str()),
            Some("loser123")
        );
        assert_eq!(
            state.get("winner_id").and_then(|v| v.as_str()),
            Some("winner456")
        );
    }

    #[test]
    fn correction_new_state_stays_empty_for_non_merge_notes() {
        let item = MarkItem {
            id: "thing123".to_string(),
            table: "kg_entities".to_string(),
            mark_type: "correction".to_string(),
            mark_note: None,
            data: json!({"name": "Thing"}),
        };

        assert_eq!(
            correction_new_state(&item, "No merge target here"),
            json!({})
        );
    }
}
