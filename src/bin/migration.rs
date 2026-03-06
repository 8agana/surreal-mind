use anyhow::Result;

use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[derive(Debug)]
struct ParsedSkater {
    first_name: String,
    last_name: String,
}

#[derive(Debug)]
struct ParsedName {
    skaters: Vec<ParsedSkater>,
    is_family: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = "photography";
    let dbname = "ops";

    // Connect to SurrealDB
    let db = Surreal::new::<Ws>(url).await?;
    db.signin(Root {
        username: user,
        password: pass,
    })
    .await?;
    db.use_ns(ns).use_db(dbname).await?;

    println!("Starting migration of broken skater records...");

    // Query skaters with "and" in first_name
    let mut resp = db
        .query("SELECT id, first_name, last_name FROM skater WHERE first_name CONTAINS 'and'")
        .await?;
    let broken_skaters: Vec<serde_json::Value> = resp.take(0)?;

    println!("Found {} broken skater records", broken_skaters.len());

    for skater in broken_skaters {
        let skater_id = skater
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let first_name = skater
            .get("first_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let last_name = skater
            .get("last_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        println!(
            "Migrating skater: {} {} (id: {})",
            first_name, last_name, skater_id
        );

        // Reconstruct full name
        let full_name = format!("{} {}", first_name, last_name);

        // Parse
        let parsed = match parse_skater_names(&full_name) {
            Ok(p) => p,
            Err(e) => {
                println!("Failed to parse '{}': {}", full_name, e);
                continue;
            }
        };

        if !parsed.is_family {
            println!("Parsed as non-family, skipping");
            continue;
        }

        // Create family
        let family_id = format!(
            "family:{}",
            parsed.skaters[0].last_name.to_lowercase().replace(" ", "_")
        );
        let family_resp = db
            .query(
                "INSERT INTO family (id, first_name, last_name, created_at)
                 VALUES ($id, 'Family', $last, time::now())
                 ON DUPLICATE KEY UPDATE first_name = 'Family', last_name = $last",
            )
            .bind(("id", family_id.clone()))
            .bind(("last", parsed.skaters[0].last_name.clone()))
            .await?;
        family_resp.check()?;

        // Get existing competed_in relations
        let mut compete_resp = db
            .query("SELECT * FROM competed_in WHERE in = $skater_id")
            .bind(("skater_id", skater_id.clone()))
            .await?;
        let competitions: Vec<serde_json::Value> = compete_resp.take(0)?;

        // For each parsed skater
        for skater_data in &parsed.skaters {
            let new_skater_id = format!(
                "{}_{}",
                skater_data.last_name.to_lowercase(),
                skater_data.first_name.to_lowercase()
            )
            .replace('-', "_");

            // Create new skater
            let skater_resp = db
                .query(
                    "INSERT INTO skater (id, first_name, last_name, created_at)
                     VALUES ($id, $first, $last, time::now())
                     ON DUPLICATE KEY UPDATE first_name = $first, last_name = $last",
                )
                .bind(("id", new_skater_id.clone()))
                .bind(("first", skater_data.first_name.clone()))
                .bind(("last", skater_data.last_name.clone()))
                .await?;
            skater_resp.check()?;

            // Create belongs_to
            let belongs_resp = db
                .query(
                    "RELATE (type::record('skater', $skater_id))->belongs_to->(type::record('family', $family_id))
                     CONTENT { created_at: time::now() }",
                )
                .bind(("skater_id", new_skater_id.clone()))
                .bind(("family_id", family_id.clone()))
                .await?;
            belongs_resp.check()?;

            // Create new competed_in relations
            for comp in &competitions {
                let event_id = comp
                    .get("out")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let skate_order = comp
                    .get("skate_order")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32);
                let request_status = comp
                    .get("request_status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let gallery_status = comp
                    .get("gallery_status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let gallery_url = comp
                    .get("gallery_url")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let purchase_amount = comp.get("purchase_amount").and_then(|v| v.as_f64());
                let notes = comp
                    .get("notes")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let new_relation_resp = db
                    .query(
                        "RELATE (type::record('skater', $new_skater_id))->competed_in->(type::record('event', $event_id))
                         CONTENT {
                            skate_order: $skate_order,
                            request_status: $request_status,
                            gallery_status: $gallery_status,
                            gallery_url: $gallery_url,
                            purchase_amount: $purchase_amount,
                            notes: $notes
                         }",
                    )
                    .bind(("new_skater_id", new_skater_id.clone()))
                    .bind(("event_id", event_id))
                    .bind(("skate_order", skate_order))
                    .bind(("request_status", request_status))
                    .bind(("gallery_status", gallery_status))
                    .bind(("gallery_url", gallery_url))
                    .bind(("purchase_amount", purchase_amount))
                    .bind(("notes", notes))
                    .await?;
                new_relation_resp.check()?;
            }
        }

        // Delete old competed_in relations
        for comp in &competitions {
            let comp_id = comp
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let _ = db.query("DELETE $id").bind(("id", comp_id)).await?;
        }

        // Delete old skater
        let _ = db.query("DELETE $id").bind(("id", skater_id)).await?;
    }

    println!("Migration completed!");
    Ok(())
}

fn parse_skater_names(name: &str) -> Result<ParsedName> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow::anyhow!("Empty skater name"));
    }

    // Check if it's synchro
    if name.to_lowercase().starts_with("synchro ") {
        let team = name[8..].trim();
        let skater = ParsedSkater {
            first_name: "Synchro".to_string(),
            last_name: team.to_string(),
        };
        return Ok(ParsedName {
            skaters: vec![skater],
            is_family: false,
        });
    }

    // Split into words
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.is_empty() {
        return Err(anyhow::anyhow!("Empty skater name"));
    }

    // Last word is last_name
    let last_name = words.last().unwrap().to_string();

    // First part is all except last word
    let first_part = &name[..name.len() - last_name.len()].trim();

    // Parse first_part
    let first_names: Vec<String> = first_part
        .split(',')
        .flat_map(|s| s.split(" and "))
        .map(|s| s.trim().trim_end_matches(','))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if first_names.is_empty() {
        return Err(anyhow::anyhow!("No first names found"));
    }

    let skaters: Vec<ParsedSkater> = first_names
        .into_iter()
        .map(|first| ParsedSkater {
            first_name: first,
            last_name: last_name.clone(),
        })
        .collect();

    let is_family = skaters.len() > 1;

    Ok(ParsedName { skaters, is_family })
}
