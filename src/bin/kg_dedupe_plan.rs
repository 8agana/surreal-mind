use anyhow::Result;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashMap;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[derive(Debug, Clone)]
struct Entity {
    id: String,
    name: String,
    entity_type: String,
    data: Value,
    #[allow(dead_code)]
    created_at_raw: String,
    created_at_ts: i64,
    has_embedding: bool,
}

fn normalize_name(name: &str) -> String {
    // Lowercase, strip possessive 's/’s, remove punctuation, collapse whitespace
    let mut s = name.to_lowercase();
    s = s.replace("’s", "");
    s = s.replace("'s", "");
    // Replace punctuation with space
    let re_punct = Regex::new(r"[^a-z0-9]+").unwrap();
    s = re_punct.replace_all(&s, " ").to_string();
    let re_ws = Regex::new(r"\s+").unwrap();
    s = re_ws.replace_all(s.trim(), " ").to_string();
    s
}

fn data_completeness(data: &Value) -> usize {
    match data {
        Value::Object(map) => map
            .iter()
            .filter(|(k, v)| !k.is_empty() && !v.is_null())
            .count(),
        _ => 0,
    }
}

fn is_junk_norm(norm: &str) -> bool {
    matches!(
        norm,
        "these"
            | "ram"
            | "ci"
            | "let"
            | "let s"
            | "let's"
            | "private"
            | "creative"
            | "internal"
            | "ultra"
            | "test"
            | "test entity"
            | "test_entity"
            | "database"
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    // DB config
    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = std::env::var("SURR_DB_NS").unwrap_or_else(|_| "surreal_mind".to_string());
    let dbname = std::env::var("SURR_DB_DB").unwrap_or_else(|_| "consciousness".to_string());

    let db = Surreal::new::<Ws>(url).await?;
    db.signin(Root {
        username: &user,
        password: &pass,
    })
    .await?;
    db.use_ns(ns).use_db(dbname).await?;

    // Fetch entities
    let entities: Vec<Value> = db
        .query(
            "SELECT meta::id(id) as id, name, data, entity_type, created_at, \
             (IF type::is::array(embedding) THEN array::len(embedding) ELSE 0 END) AS emb_len \
             FROM kg_entities",
        )
        .await?
        .take(0)?;

    // Fetch edges for degree calc
    let edges: Vec<Value> = db
        .query(
            "SELECT meta::id(id) as id, \
                (IF type::is::record(source) THEN meta::id(source) ELSE string::concat(source) END) as source_id, \
                (IF type::is::record(target) THEN meta::id(target) ELSE string::concat(target) END) as target_id \
             FROM kg_edges",
        )
        .await?
        .take(0)?;

    let mut degree: HashMap<String, usize> = HashMap::new();
    for e in edges {
        if let Some(sid) = e.get("source_id").and_then(|v| v.as_str()) {
            *degree.entry(sid.to_string()).or_insert(0) += 1;
        }
        if let Some(tid) = e.get("target_id").and_then(|v| v.as_str()) {
            *degree.entry(tid.to_string()).or_insert(0) += 1;
        }
    }

    let mut prepared: Vec<Entity> = Vec::new();
    for r in entities {
        let id = r
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = r
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let etype = r
            .get("entity_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                r.get("data")
                    .and_then(|d| d.get("entity_type"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());
        let data = r.get("data").cloned().unwrap_or(json!({}));
        let created_at_raw = r
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let created_at_ts: i64 = DateTime::parse_from_rfc3339(&created_at_raw)
            .ok()
            .map(|dt| dt.timestamp())
            .unwrap_or(0);
        let emb_len = r.get("emb_len").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        prepared.push(Entity {
            id,
            name,
            entity_type: etype,
            data,
            created_at_raw,
            created_at_ts,
            has_embedding: emb_len > 0,
        });
    }

    // Build fast degree accessor for scoring
    let degree_map = degree;

    // Identify junk and exclude from grouping; collect delete list
    let mut delete_list: Vec<Value> = Vec::new();
    let mut survivors: Vec<Entity> = Vec::new();
    for e in prepared.into_iter() {
        let norm = normalize_name(&e.name);
        let deg = degree_map.get(&e.id).copied().unwrap_or(0);
        let dkeys = data_completeness(&e.data);
        let is_test_type = e.entity_type.eq_ignore_ascii_case("test");
        if is_junk_norm(&norm) || is_test_type || norm == "test_entity" {
            // Mark for deletion (dry-run)
            delete_list.push(json!({
                "id": e.id,
                "name": e.name,
                "norm": norm,
                "reason": if is_test_type { "test_type" } else { "junk_name" },
                "degree": deg,
                "data_keys": dkeys
            }));
        } else {
            survivors.push(e);
        }
    }

    // Group survivors by (normalized_name, entity_type)
    let mut groups: HashMap<(String, String), Vec<Entity>> = HashMap::new();
    for e in survivors.into_iter() {
        let norm = normalize_name(&e.name);
        groups
            .entry((norm, e.entity_type.clone()))
            .or_default()
            .push(e);
    }

    // Canonical merge map (normalize → canonical name)
    let canonical_map: HashMap<&'static str, &'static str> = HashMap::from([
        ("sam", "sam atagana"),
        ("sam s", "sam atagana"),
        ("federation", "federation"),
        ("chrome", "chrome"),
        ("firefox", "firefox"),
    ]);

    // Resolve canonical IDs
    let mut canonical_ids: HashMap<String, Option<String>> = HashMap::new();
    let canonical_targets = vec![
        ("sam atagana", "Sam Atagana"),
        ("federation", "Federation"),
        ("chrome", "Chrome"),
        ("firefox", "Firefox"),
    ];
    for (norm, name) in canonical_targets {
        let rows: Vec<Value> = db
            .query("SELECT meta::id(id) as id FROM kg_entities WHERE name = $name LIMIT 1")
            .bind(("name", name))
            .await?
            .take(0)?;
        let cid = rows
            .first()
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        canonical_ids.insert(norm.to_string(), cid);
    }

    // Build plan for groups with >1
    let mut plan_groups: Vec<Value> = Vec::new();
    for ((norm_name, etype), ents) in groups.into_iter() {
        if ents.len() <= 1 {
            continue;
        }
        // Score and pick winner
        let mut best_idx = 0usize;
        let mut best_score: isize = -1;
        for (i, e) in ents.iter().enumerate() {
            let comp = data_completeness(&e.data) as isize; // data richness
            let deg = *degree_map.get(&e.id).unwrap_or(&0) as isize; // connectivity
            let emb = if e.has_embedding { 1 } else { 0 } as isize; // has vector
            // Prefer older records slightly (stable id)
            let age = if e.created_at_ts > 0 {
                (Utc::now().timestamp() - e.created_at_ts) as isize
            } else {
                0
            };
            let score = 5 * deg + 3 * comp + 2 * emb + (age / 86_400); // age in days
            if score > best_score {
                best_score = score;
                best_idx = i;
            }
        }
        // Ensure we never pick a 0/0 candidate if better exists
        let best_is_empty = degree_map.get(&ents[best_idx].id).copied().unwrap_or(0) == 0
            && data_completeness(&ents[best_idx].data) == 0;
        if best_is_empty {
            if let Some((i_alt, _)) = ents
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    degree_map.get(&e.id).copied().unwrap_or(0) > 0
                        || data_completeness(&e.data) > 0
                })
                .max_by_key(|(_, e)| degree_map.get(&e.id).copied().unwrap_or(0))
            {
                best_idx = i_alt;
            }
        }
        let winner = &ents[best_idx];
        let losers: Vec<&Entity> = ents
            .iter()
            .enumerate()
            .filter_map(|(i, e)| if i != best_idx { Some(e) } else { None })
            .collect();

        // Summarize data completeness and degree
        let mut loser_summaries: Vec<Value> = Vec::new();
        for l in &losers {
            loser_summaries.push(json!({
                "id": l.id,
                "name": l.name,
                "data_keys": data_completeness(&l.data),
                "degree": degree_map.get(&l.id).copied().unwrap_or(0),
                "has_embedding": l.has_embedding
            }));
        }

        plan_groups.push(json!({
            "normalized_name": norm_name,
            "entity_type": etype,
            "winner": {
                "id": winner.id,
                "name": winner.name,
                "data_keys": data_completeness(&winner.data),
                "degree": degree_map.get(&winner.id).copied().unwrap_or(0),
                "has_embedding": winner.has_embedding
            },
            "losers": loser_summaries
        }));
    }

    // Forced canonical merge groups based on alias mapping
    let mut forced_groups: Vec<Value> = Vec::new();
    // Build reverse map canonical_norm -> set of member entities
    let _canonical_members: HashMap<String, Vec<&Value>> = HashMap::new();
    // We need an index of survivors by normalized name
    let mut by_norm: HashMap<String, Vec<(String, String)>> = HashMap::new(); // norm -> [(id, name)]
    // Re-fetch a light list for indexing survivors quickly
    let survivors_rows: Vec<Value> = db
        .query("SELECT meta::id(id) as id, name FROM kg_entities")
        .await?
        .take(0)?;
    for r in &survivors_rows {
        if let (Some(id), Some(name)) = (
            r.get("id").and_then(|v| v.as_str()),
            r.get("name").and_then(|v| v.as_str()),
        ) {
            let norm = normalize_name(name);
            by_norm
                .entry(norm)
                .or_default()
                .push((id.to_string(), name.to_string()));
        }
    }
    for (alias, canonical_norm) in &canonical_map {
        if let Some(list) = by_norm.get(*alias) {
            let canonical_id = canonical_ids.get(*canonical_norm).and_then(|o| o.clone());
            forced_groups.push(json!({
                "canonical_norm": canonical_norm,
                "canonical_id": canonical_id,
                "members": list.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>()
            }));
        }
    }

    let plan = json!({
        "summary": {
            "duplicate_groups": plan_groups.len(),
            "forced_canonical_groups": forced_groups.len(),
            "delete_candidates": delete_list.len()
        },
        "delete_list": delete_list,
        "merge_groups": plan_groups,
        "forced_canonical_groups": forced_groups
    });

    // Print to stdout
    println!("{}", serde_json::to_string_pretty(&plan)?);

    // Also write to file for review
    std::fs::write("kg_cleanup_plan.json", serde_json::to_vec_pretty(&plan)?)?;

    Ok(())
}
