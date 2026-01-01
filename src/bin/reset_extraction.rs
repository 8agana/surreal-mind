use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = std::env::var("SURR_DB_NS").unwrap_or_else(|_| "surreal_mind".to_string());
    let dbname = std::env::var("SURR_DB_DB").unwrap_or_else(|_| "consciousness".to_string());

    let db = Surreal::new::<Ws>(url).await?;
    db.signin(Root { username: &user, password: &pass }).await?;
    db.use_ns(&ns).use_db(&dbname).await?;

    let res = db.query("UPDATE (SELECT id FROM thoughts WHERE extracted_to_kg = true LIMIT 10) SET extracted_to_kg = false").await?;
    res.check()?;
    println!("Reset 10 thoughts to unextracted.");

    Ok(())
}
