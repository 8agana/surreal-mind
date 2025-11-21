use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;
use serde::Deserialize;
use surrealdb::sql::Thing;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
struct Family {
    id: Thing,
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

    println!("Fetching all families...");
    let mut resp = db.query("SELECT id FROM family").await?;
    let families: Vec<Family> = resp.take(0)?;

    // Group by normalized ID string
    let mut groups: HashMap<String, Vec<Thing>> = HashMap::new();
    for f in families {
        let id_str = f.id.to_string();
        // Remove "family:" prefix and backticks for comparison
        let clean_id = id_str.replace("family:", "").replace("`", "").to_lowercase();
        groups.entry(clean_id).or_default().push(f.id);
    }

    let mut merged_count = 0;

    for (key, ids) in groups {
        if ids.len() > 1 {
            println!("Found duplicates for '{}': {:?}", key, ids);
            
            // Pick winner: The one that matches the key exactly (lowercase) or just the first one
            // Prefer "family:name" over "family:Name"
            // Construct target ID
            let target_id_str = format!("family:{}", key);
            
            // Find if target exists in the list
            let mut winner = ids[0].clone();
            for id in &ids {
                if id.to_string() == target_id_str {
                    winner = id.clone();
                    break;
                }
            }

            for id in ids {
                if id != winner {
                    println!("Merging {} into {}", id, winner);
                    
                    // 1. Move Skater relations (belongs_to)
                    let sql_skater = format!("UPDATE belongs_to SET out = {} WHERE out = {}", winner, id);
                    db.query(sql_skater).await?;

                    // 2. Move Competition relations (family_competition)
                    // We need to be careful not to create duplicates if winner already has the edge
                    // Strategy: Select edges for loser. For each, check if winner has it. If not, move. If yes, delete loser edge.
                    
                    // Simplify: Just delete loser edges for now? No, we want to preserve status.
                    // If Loser has "Sent" and Winner has "Pending", we want Winner to become "Sent".
                    // This logic is complex in SQL.
                    
                    // Brute force: Update 'in' to winner. If collision, SurrealDB might error or merge?
                    // "UPDATE family_competition SET in = winner WHERE in = loser"
                    // If duplicate `in, out` pair created, does it fail?
                    // Without UNIQUE index, it creates duplicate.
                    // So we run dedupe_edges.rs AFTER this merge.
                    
                    let sql_comp = format!("UPDATE family_competition SET in = {} WHERE in = {}", winner, id);
                    db.query(sql_comp).await?;

                    // 3. Delete Loser Family
                    let sql_del = format!("DELETE {}", id);
                    db.query(sql_del).await?;
                    
                    merged_count += 1;
                }
            }
        }
    }

    println!("Merged {} duplicate families.", merged_count);
    Ok(())
}
