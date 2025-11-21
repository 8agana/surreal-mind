use anyhow::Result;
use surrealdb::{Surreal, engine::remote::ws::Ws, opt::auth::Root};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let db = Surreal::new::<Ws>("127.0.0.1:8000").await?;
    db.signin(Root { username: "root", password: "root" }).await?;
    db.use_ns("photography").use_db("ops").await?;

    println!("Bair family_competition edges:");
    let mut resp = db.query("SELECT * FROM family_competition WHERE in.last_name = bair;").await?;
    let edges: Vec<serde_json::Value> = resp.take(0)?;
    for edge in edges {
        println!("{}", serde_json::to_string_pretty(&edge)?);
    }
    Ok(())
}
