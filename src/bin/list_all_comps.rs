use anyhow::Result;
use surrealdb::{Surreal, engine::remote::ws::Ws, opt::auth::Root};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let db = Surreal::new::<Ws>("127.0.0.1:8000").await?;
    db.signin(Root { username: "root", password: "root" }).await?;
    db.use_ns("photography").use_db("ops").await?;

    let mut resp = db.query("SELECT id, name FROM competition ORDER BY name;").await?;
    let comps: Vec<serde_json::Value> = resp.take(0)?;
    println!("All {} competitions:", comps.len());
    for comp in comps {
        println!("  ID: {:?} | Name: {:?}", comp.get("id"), comp.get("name"));
    }
    Ok(())
}
