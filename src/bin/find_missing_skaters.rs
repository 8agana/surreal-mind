use anyhow::Result;
use surrealdb::{Surreal, engine::remote::ws::{Ws, Client}};
use surrealdb::opt::auth::Root;
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    
    let db: Surreal<Client> = Surreal::new::<Ws>("127.0.0.1:8000").await?;
    db.signin(Root { username: "root", password: "root" }).await?;
    db.use_ns("photography").use_db("ops").await?;

    // Read SkaterRequests.md
    let content = fs::read_to_string("/Users/samuelatagana/Projects/LegacyMind/photography-information/2025 Pony Express/SkaterRequests.md")?;
    
    let mut missing = Vec::new();
    
    for line in content.lines() {
        // Skater entries start with capital letter and have first/last name
        if line.starts_with(char::is_uppercase) && !line.starts_with("##") && !line.starts_with("   ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let last_name = parts[0];
                let first_name = parts[1];
                
                // Check if skater exists in DB
                let sql = "SELECT * FROM skater WHERE last_name = $last AND first_name = $first;";
                let mut resp = db.query(sql)
                    .bind(("last", last_name))
                    .bind(("first", first_name))
                    .await?;
                let result: Vec<serde_json::Value> = resp.take(0)?;
                
                if result.is_empty() {
                    missing.push(format!("{} {}", last_name, first_name));
                }
            }
        }
    }
    
    println!("Missing {} skaters:", missing.len());
    for skater in missing {
        println!("  - {}", skater);
    }
    
    Ok(())
}
