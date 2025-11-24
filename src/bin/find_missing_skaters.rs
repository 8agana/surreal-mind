use anyhow::Result;
use std::fs;
use surrealdb::opt::auth::Root;
use surrealdb::{
    Surreal,
    engine::remote::ws::{Client, Ws},
};

/// Extract candidate skater names (First, Last) from the SkaterRequests markdown.
fn parse_requested_skaters(content: &str) -> Vec<(String, String)> {
    let mut names = Vec::new();
    for line in content.lines() {
        // Skater entries start with capital letter and have first/last name
        if line.starts_with(char::is_uppercase)
            && !line.starts_with("##")
            && !line.starts_with("   ")
        {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                names.push((parts[1].to_string(), parts[0].to_string()));
            }
        }
    }
    names
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let db: Surreal<Client> = Surreal::new::<Ws>("127.0.0.1:8000").await?;
    db.signin(Root {
        username: "root",
        password: "root",
    })
    .await?;
    db.use_ns("photography").use_db("ops").await?;

    // Read SkaterRequests.md
    let content = fs::read_to_string(
        "/Users/samuelatagana/Projects/LegacyMind/photography-information/2025 Pony Express/SkaterRequests.md",
    )?;

    let mut missing = Vec::new();
    for (first_name, last_name) in parse_requested_skaters(&content) {
        // Check if skater exists in DB
        let sql = "SELECT * FROM skater WHERE last_name = $last AND first_name = $first;";
        let mut resp = db
            .query(sql)
            .bind(("last", last_name.clone()))
            .bind(("first", first_name.clone()))
            .await?;
        let result: Vec<serde_json::Value> = resp.take(0)?;

        if result.is_empty() {
            missing.push(format!("{} {}", last_name, first_name));
        }
    }

    println!("Missing {} skaters:", missing.len());
    for skater in missing {
        println!("  - {}", skater);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_in_expected_order() {
        let sample = "\
Anderson Taylor
## Header
   Not a name
Brown Casey";

        let parsed = parse_requested_skaters(sample);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], ("Taylor".to_string(), "Anderson".to_string()));
        assert_eq!(parsed[1], ("Casey".to_string(), "Brown".to_string()));
    }
}
