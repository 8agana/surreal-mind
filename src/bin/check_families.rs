use anyhow::Result;
use surrealdb::{Surreal, engine::remote::ws::Ws, opt::auth::Root};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let db = Surreal::new::<Ws>("127.0.0.1:8000").await?;
    db.signin(Root { username: "root", password: "root" }).await?;
    db.use_ns("photography").use_db("ops").await?;

    println!("Families with bair in name:");
    let mut resp = db.query("SELECT id, name, last_name FROM family WHERE last_name CONTAINS air;").await?;
    let families: Vec<serde_json::Value> = resp.take(0)?;
    for family in families {
        println!("{}", serde_json::to_string_pretty(&family)?);
    }
    
    println!("\nAll family_competition edges (first 5):");
    let mut resp = db.query("SELECT * FROM family_competition LIMIT 5;").await?;
    let edges: Vec<serde_json::Value> = resp.take(0)?;
    for edge in edges {
        println!("{}", serde_json::to_string_pretty(&edge)?);
    }
    Ok(())
}
