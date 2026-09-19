use serde_json::Value;
use surrealdb::Surreal;
use surrealdb::engine::local::SurrealKv;

type AnyError = Box<dyn std::error::Error + Send + Sync>;

async fn manifest(db: &Surreal<surrealdb::engine::local::Db>) -> Result<Vec<Value>, AnyError> {
    let mut response = db
        .query("SELECT meta::id(id) AS id, status FROM thoughts ORDER BY id")
        .await?;
    Ok(response.take(0)?)
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let dir = tempfile::tempdir()?;
    let db = Surreal::new::<SurrealKv>(dir.path().to_str().unwrap()).await?;
    db.use_ns("fed_ee4538").use_db("delete_repro").await?;
    db.query("DEFINE TABLE thoughts SCHEMAFULL; DEFINE FIELD status ON thoughts TYPE string; DEFINE FIELD created_at ON thoughts TYPE datetime;")
        .await?;
    let create1 = db
        .query("CREATE thoughts:eligible SET status = 'removal', created_at = time::now() - 31d;")
        .await?;
    let create2 = db
        .query("CREATE thoughts:keep SET status = 'active', created_at = time::now() - 31d;")
        .await?;
    println!("CREATE1_RESPONSE={:?}", create1);
    println!("CREATE2_RESPONSE={:?}", create2);

    let before = manifest(&db).await?;
    let mut response = db
        .query("SELECT meta::id(id) AS id FROM thoughts WHERE status = 'removal' AND created_at < time::now() - 30d LIMIT 10")
        .await?;
    let candidates: Vec<Value> = response.take(0)?;
    let ids: Vec<String> = candidates
        .iter()
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect();
    let mut string_delete = db
        .query("DELETE FROM thoughts WHERE id IN $ids")
        .bind(("ids", ids.clone()))
        .await?;
    let string_result: Vec<Value> = string_delete.take(0)?;
    let after_string = manifest(&db).await?;

    let mut typed_delete = db
        .query("DELETE FROM thoughts WHERE id IN [thoughts:eligible]")
        .await?;
    let typed_result: Vec<Value> = typed_delete.take(0)?;
    let after_typed = manifest(&db).await?;

    println!("BEFORE={}", serde_json::to_string(&before)?);
    println!("CANDIDATE_IDS={}", serde_json::to_string(&ids)?);
    println!("STRING_DELETE_RESULT={}", serde_json::to_string(&string_result)?);
    println!("AFTER_STRING={}", serde_json::to_string(&after_string)?);
    println!("TYPED_DELETE_RESULT={}", serde_json::to_string(&typed_result)?);
    println!("AFTER_TYPED={}", serde_json::to_string(&after_typed)?);
    Ok(())
}
