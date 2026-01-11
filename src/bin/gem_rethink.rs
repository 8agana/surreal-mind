//! gem_rethink - minimal Gemini mark queue processor
//!
//! Iterates over records marked_for "gemini", logs actions, and (for now)
//! records a CorrectionEvent (no content mutation) then clears the mark.
//! This is a stopgap to unblock Phase 5 testing; deeper Gemini-driven
//! corrections/enrichments can be layered on later.

use anyhow::{Context, Result};
use serde_json::json;
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
        username: &config.runtime.database_user,
        password: &config.runtime.database_pass,
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
    let query = "SELECT meta::id(id) as rid, meta::tb(id) as table, mark_type, mark_note, marked_at, marked_by, content, name, data.name as data_name \
                 FROM thoughts, kg_entities, kg_observations \
                 WHERE marked_for = 'gemini' \
                 ORDER BY marked_at ASC, rid ASC \
                 LIMIT $limit";

    let rows: Vec<serde_json::Value> = db.query(query).bind(("limit", limit)).await?.take(0)?;

    let mut items = Vec::new();
    for r in rows {
        let id = r
            .get("rid")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("row missing id"))?
            .to_string();
        let table = r
            .get("table")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("row missing table"))?
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

    // CorrectionEvent with previous_state/new_state identical (placeholder)
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
        .bind(("new_state", item.data.clone()))
        .bind((
            "reasoning",
            item.mark_note
                .clone()
                .unwrap_or_else(|| "No reasoning provided".to_string()),
        ))
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

async fn clear_mark(db: Arc<Surreal<WsClient>>, item: &MarkItem) -> Result<()> {
    let q = format!(
        "UPDATE {} SET marked_for = NONE, mark_type = NONE, mark_note = NONE, marked_at = NONE, marked_by = NONE WHERE id = type::thing('{}', $id) RETURN NONE",
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
