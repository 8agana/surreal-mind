use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;
use serde::Deserialize;
use surrealdb::sql::Thing;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
struct Edge {
    id: Thing,
    #[serde(rename = "in")]
    in_: Thing,
    gallery_status: Option<String>,
    ty_requested: Option<bool>,
    ty_sent: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = "photography";
    let dbname = "ops";

    let db = Surreal::new::<Ws>(url).await?;
    db.signin(Root { username: &user, password: &pass }).await?;
    db.use_ns(ns).use_db(dbname).await?;

    println!("Fetching edges...");
    // Get all edges for Pony Express (using resolved name/ID if possible, but 'pony' checks name)
    let sql = "SELECT * FROM family_competition WHERE out.name CONTAINS 'pony'";
    let mut resp = db.query(sql).await?;
    let edges: Vec<Edge> = resp.take(0)?;

    println!("Total edges: {}", edges.len());

    // Group by Family (in)
    let mut groups: HashMap<String, Vec<Edge>> = HashMap::new();
    for e in edges {
        let key = e.in_.to_string();
        groups.entry(key).or_default().push(e);
    }

    println!("Unique families: {}", groups.len());

    let mut deleted_count = 0;

    for (family_id, mut group) in groups {
        if group.len() > 1 {
            // Sort logic:
            // 1. Status != 'pending' (Sent/Purchased > Pending)
            // 2. TY Requested/Sent
            group.sort_by(|a, b| {
                let a_score = score(a);
                let b_score = score(b);
                b_score.cmp(&a_score) // Descending
            });

            // Keep index 0, delete rest
            let keep = &group[0];
            let to_delete = &group[1..];

            println!("Deduplicating {}: Keeping {:?} (Score {}), Deleting {} duplicates", 
                family_id, keep.gallery_status, score(keep), to_delete.len());

            for del in to_delete {
                // Delete query
                // Use ID directly via SQL
                let _ = db.query(format!("DELETE {}", del.id)).await?;
                deleted_count += 1;
            }
        }
    }

    println!("Deduplication complete. Deleted {} edges.", deleted_count);
    Ok(())
}

fn score(e: &Edge) -> i32 {
    let mut score = 0;
    if let Some(status) = &e.gallery_status {
        if status == "sent" || status == "purchased" {
            score += 100;
        } else if status != "pending" {
            score += 50; // needs_research, not_shot
        }
    }
    if e.ty_sent == Some(true) {
        score += 20;
    }
    if e.ty_requested == Some(true) {
        score += 10;
    }
    score
}
