use anyhow::Result;
use surrealdb::{Surreal, engine::remote::ws::{Ws, Client}};
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    
    let db: Surreal<Client> = Surreal::new::<Ws>("127.0.0.1:8000").await?;
    db.signin(Root { username: "root", password: "root" }).await?;
    db.use_ns("photography").use_db("ops").await?;

    let sql = "SELECT count() FROM skater GROUP ALL;";
    let mut resp = db.query(sql).await?;
    let result: Vec<serde_json::Value> = resp.take(0)?;
    
    println!("Total skaters in database: {:?}", result);
    
    Ok(())
}
