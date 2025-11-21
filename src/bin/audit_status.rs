use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;
use serde::Deserialize;
use surrealdb::sql::Thing;

#[derive(Debug, Deserialize)]
struct Edge {
    id: Thing,
    #[serde(rename = "in")]
    in_: Thing,
    gallery_status: Option<String>,
    ty_requested: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let url = std::env::var("SURR_DB_URL").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let user = std::env::var("SURR_DB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("SURR_DB_PASS").unwrap_or_else(|_| "root".to_string());
    let ns = "photography";
    let dbname = "ops";

    let db = Surreal::new::<Ws>(url).await?;
    db.signin(Root { username: &user, password: &pass }).await?;
    db.use_ns(ns).use_db(dbname).await?;

    println!("Auditing Sent/Purchased records for Pony Express...");
    
    let sql = "
        SELECT id, in, gallery_status, ty_requested 
        FROM family_competition 
        WHERE out.name CONTAINS 'pony' 
        AND gallery_status != 'pending'
    ";
    
    let mut resp = db.query(sql).await?;
    let edges: Vec<Edge> = resp.take(0)?;

    println!("Found {} non-pending edges.", edges.len());
    for e in edges {
        println!("{} | Status: {:?} | TY: {:?}", e.in_, e.gallery_status, e.ty_requested);
    }

    Ok(())
}
