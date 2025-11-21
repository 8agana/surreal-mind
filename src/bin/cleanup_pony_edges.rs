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

    println!("Connecting...");
    let db = Surreal::new::<Ws>(url).await?;
    db.signin(Root { username: &user, password: &pass }).await?;
    db.use_ns("photography").use_db("ops").await?;

    println!("Deleting all Pony Express edges...");
    db.query("DELETE family_competition WHERE out.name CONTAINS Pony;").await?;
    println!("Done!");
    Ok(())
}
