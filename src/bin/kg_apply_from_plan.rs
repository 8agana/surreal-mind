use anyhow::{Result, anyhow};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;

fn getenv_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let apply = getenv_flag("APPLY");
    let do_delete = getenv_flag("DELETE");
    let plan_path = std::env::var("PLAN").unwrap_or_else(|_| "kg_cleanup_plan.json".to_string());

    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = std::env::var("SURR_DB_NS").unwrap_or_else(|_| "surreal_mind".to_string());
    let dbname = std::env::var("SURR_DB_DB").unwrap_or_else(|_| "consciousness".to_string());

    println!(
        "KG apply-from-plan starting (apply={}, delete={}, plan={})",
        apply, do_delete, plan_path
    );

    let plan_bytes = fs::read(&plan_path)?;
    let plan: Value = serde_json::from_slice(&plan_bytes)?;

    let db: Surreal<Client> = Surreal::new::<Ws>(url).await?;
    db.signin(Root {
        username: &user,
        password: &pass,
    })
    .await?;
    db.use_ns(ns).use_db(dbname).await?;

    // Build sets
    let mut forced_groups: Vec<(Option<String>, Vec<String>)> = Vec::new();
    if let Some(arr) = plan
        .get("forced_canonical_groups")
        .and_then(|v| v.as_array())
    {
        for g in arr {
            let canonical_id = g
                .get("canonical_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let members: Vec<String> = g
                .get("members")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|it| it.get("id").and_then(|v| v.as_str()))
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            forced_groups.push((canonical_id, members));
        }
    }

    let mut merge_groups: Vec<(String, Vec<String>)> = Vec::new();
    if let Some(arr) = plan.get("merge_groups").and_then(|v| v.as_array()) {
        for g in arr {
            let winner = g
                .get("winner")
                .and_then(|w| w.get("id"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("merge_groups item missing winner.id"))?;
            let losers: Vec<String> = g
                .get("losers")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|it| it.get("id").and_then(|v| v.as_str()))
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if !losers.is_empty() {
                merge_groups.push((winner.to_string(), losers));
            }
        }
    }

    let mut delete_ids: Vec<String> = Vec::new();
    if let Some(arr) = plan.get("delete_list").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                delete_ids.push(id.to_string());
            }
        }
    }

    // Helper functions
    async fn edge_count_for(db: &Surreal<Client>, ent_id: &str) -> Result<i64> {
        let src_rows: Vec<Value> = db
            .query(
                "SELECT count() AS c FROM kg_edges WHERE source = type::thing('kg_entities', $id) GROUP ALL",
            )
            .bind(("id", ent_id.to_string()))
            .await?
            .take(0)?;
        let dst_rows: Vec<Value> = db
            .query(
                "SELECT count() AS c FROM kg_edges WHERE target = type::thing('kg_entities', $id) GROUP ALL",
            )
            .bind(("id", ent_id.to_string()))
            .await?
            .take(0)?;
        let sv = src_rows
            .first()
            .and_then(|v| v.get("c"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let dv = dst_rows
            .first()
            .and_then(|v| v.get("c"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        Ok(sv + dv)
    }

    async fn redirect_edges(db: &Surreal<Client>, from_id: &str, to_id: &str) -> Result<()> {
        db
            .query(
                "UPDATE kg_edges SET source = type::thing('kg_entities', $toid) WHERE source = type::thing('kg_entities', $fromid)",
            )
            .bind(("toid", to_id.to_string()))
            .bind(("fromid", from_id.to_string()))
            .await?;
        db
            .query(
                "UPDATE kg_edges SET target = type::thing('kg_entities', $toid) WHERE target = type::thing('kg_entities', $fromid)",
            )
            .bind(("toid", to_id.to_string()))
            .bind(("fromid", from_id.to_string()))
            .await?;
        Ok(())
    }

    async fn mark_alias(db: &Surreal<Client>, alias_id: &str, canonical_id: &str) -> Result<()> {
        db
            .query(
                "UPDATE type::thing('kg_entities', $id) SET data.canonical_id = $cid, data.is_alias = true",
            )
            .bind(("id", alias_id.to_string()))
            .bind(("cid", canonical_id.to_string()))
            .await?;
        Ok(())
    }

    // Preview counters
    let mut total_redirects: i64 = 0;
    let mut total_aliased: usize = 0;

    // Process forced canonical groups
    for (canonical_id_opt, members) in &forced_groups {
        let Some(canonical_id) = canonical_id_opt else {
            println!(
                "[WARN] Forced group missing canonical_id; members={:?}",
                members
            );
            continue;
        };
        for m in members {
            if m == canonical_id {
                continue;
            }
            let cnt = edge_count_for(&db, m).await.unwrap_or(0);
            total_redirects += cnt;
            total_aliased += 1;
            if apply {
                let _ = redirect_edges(&db, m, canonical_id).await;
                let _ = mark_alias(&db, m, canonical_id).await;
            }
        }
    }

    // Process merge groups
    for (winner, losers) in &merge_groups {
        for l in losers {
            let cnt = edge_count_for(&db, l).await.unwrap_or(0);
            total_redirects += cnt;
            total_aliased += 1;
            if apply {
                let _ = redirect_edges(&db, l, winner).await;
                let _ = mark_alias(&db, l, winner).await;
            }
        }
    }

    // Deduplicate edges (keep first by (source,target,rel_type))
    let mut dup_deleted = 0usize;
    if apply {
        let rows: Vec<Value> = db
            .query(
                "SELECT meta::id(id) AS id, \
                 (IF type::is::record(source) THEN meta::id(source) ELSE string::concat(source) END) AS sid, \
                 (IF type::is::record(target) THEN meta::id(target) ELSE string::concat(target) END) AS tid, \
                 rel_type AS r \
                 FROM kg_edges",
            )
            .await?
            .take(0)?;
        let mut seen: HashMap<(String, String, String), String> = HashMap::new();
        let mut to_delete: Vec<String> = Vec::new();
        for r in rows {
            let id = r.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let sid = r.get("sid").and_then(|v| v.as_str()).unwrap_or("");
            let tid = r.get("tid").and_then(|v| v.as_str()).unwrap_or("");
            let rt = r.get("r").and_then(|v| v.as_str()).unwrap_or("");
            let key = (sid.to_string(), tid.to_string(), rt.to_string());
            if let Some(_existing) = seen.get(&key) {
                to_delete.push(id.to_string());
            } else {
                seen.insert(key, id.to_string());
            }
        }
        for id in &to_delete {
            let _ = db
                .query("DELETE type::thing('kg_edges', $id)")
                .bind(("id", id.to_string()))
                .await;
        }
        dup_deleted = to_delete.len();
    }

    // Safe deletes (only if no edges remain)
    let mut deleted_entities = 0usize;
    if apply && do_delete {
        for id in &delete_ids {
            let cnt = edge_count_for(&db, id).await.unwrap_or(0);
            if cnt == 0 {
                let _ = db
                    .query("DELETE type::thing('kg_entities', $id)")
                    .bind(("id", id.to_string()))
                    .await;
                deleted_entities += 1;
            } else {
                println!("[SKIP DELETE] {} still has {} edges; not deleting", id, cnt);
            }
        }
    }

    println!("\n===== KG MERGE PREVIEW =====");
    println!(
        "Redirected edge references (estimated): {}",
        total_redirects
    );
    println!(
        "Entities aliased (losers marked with canonical_id): {}",
        total_aliased
    );
    if apply {
        println!("Duplicate edges removed: {}", dup_deleted);
        if do_delete {
            println!("Entities deleted (safe, zero edges): {}", deleted_entities);
        }
    } else {
        println!("(Dry-run) No changes applied. Set APPLY=1 to execute merges.");
        println!("Optionally set DELETE=1 to delete safe junk after merges.");
    }

    Ok(())
}
