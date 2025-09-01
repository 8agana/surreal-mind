use anyhow::Result;
use surreal_mind::bge_embedder::BGEEmbedder;
use surreal_mind::embeddings::Embedder;
use surrealdb::{Surreal, engine::remote::ws::Ws, opt::auth::Root};

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() { return 0.0; }
    let mut dot=0.0; let mut na=0.0; let mut nb=0.0;
    for i in 0..a.len() { dot+=a[i]*b[i]; na+=a[i]*a[i]; nb+=b[i]*b[i]; }
    if na==0.0 || nb==0.0 { 0.0 } else { dot/(na.sqrt()*nb.sqrt()) }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let db = Surreal::new::<Ws>(std::env::var("SURR_DB_URL").unwrap_or("127.0.0.1:8000".into())).await?;
    db.signin(Root{ username: &std::env::var("SURR_DB_USER").unwrap_or("root".into()), password: &std::env::var("SURR_DB_PASS").unwrap_or("root".into())}).await?;
    db.use_ns(std::env::var("SURR_DB_NS").unwrap_or("surreal_mind".into())).use_db(std::env::var("SURR_DB_DB").unwrap_or("consciousness".into())).await?;

    let rows: Vec<serde_json::Value> = db.query("SELECT meta::id(id) as id, content FROM thoughts LIMIT 1").await?.take(0)?;
    let (id, content) = {
        let r = rows.first().expect("no thoughts");
        (r.get("id").and_then(|v| v.as_str()).unwrap().to_string(), r.get("content").and_then(|v| v.as_str()).unwrap().to_string())
    };
    println!("Using thought {}: {}", id, content.chars().take(60).collect::<String>());

    let embedder = BGEEmbedder::new()?;
    let q = embedder.embed(&content).await?;
    let stored: Vec<serde_json::Value> = db.query("SELECT embedding FROM thoughts WHERE id = type::thing('thoughts', $id) LIMIT 1").bind(("id", id.clone())).await?.take(0)?;
    let emb = stored[0]["embedding"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect::<Vec<_>>();
    println!("dims: query={}, stored={}", q.len(), emb.len());
    println!("cosine: {:.4}", cosine(&q, &emb));
    Ok(())
}
