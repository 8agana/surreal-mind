use anyhow::Result;
use surrealdb::{Surreal, engine::remote::ws::Ws, opt::auth::Root};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let db = Surreal::new::<Ws>("127.0.0.1:8000").await?;
    db.signin(Root { username: "root", password: "root" }).await?;
    db.use_ns("photography").use_db("ops").await?;

    let mut resp = db.query("SELECT count() FROM family_competition GROUP ALL;").await?;
    let result: Vec<serde_json::Value> = resp.take(0)?;
    println!("Total family_competition edges: {:?}", result);
    
    let mut resp2 = db.query("SELECT count() FROM competed_in GROUP ALL;").await?;
    let result2: Vec<serde_json::Value> = resp2.take(0)?;
    println!("Total competed_in edges: {:?}", result2);
    
    Ok(())
}
