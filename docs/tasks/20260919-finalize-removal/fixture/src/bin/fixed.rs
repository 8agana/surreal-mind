use serde_json::Value;
use surrealdb::types::RecordId;
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
    db.use_ns("fed_ee4538_fixed").use_db("delete_repro").await?;
    db.query("DEFINE TABLE thoughts SCHEMAFULL; DEFINE FIELD status ON thoughts TYPE string; DEFINE FIELD created_at ON thoughts TYPE datetime;")
        .await?;
    db.query("CREATE thoughts:eligible SET status = 'removal', created_at = time::now() - 31d;")
        .await?;
    db.query("CREATE thoughts:42 SET status = 'removal', created_at = time::now() - 31d;")
        .await?;
    db.query("CREATE thoughts:keep SET status = 'active', created_at = time::now() - 31d;")
        .await?;

    let before = manifest(&db).await?;
    let mut select = db
        .query("SELECT id FROM thoughts WHERE status = 'removal' AND created_at < time::now() - 30d LIMIT 10")
        .await?;
    let ids: Vec<RecordId> = select.take(0)?;
    let candidate_count = ids.len();

    let mut delete = db
        .query("DELETE FROM thoughts WHERE id IN $ids RETURN BEFORE")
        .bind(("ids", ids))
        .await?;
    let errors = delete.take_errors();
    if !errors.is_empty() {
        return Err(format!("statement errors: {errors:?}").into());
    }
    let deleted: Vec<Value> = delete.take(0)?;
    let after = manifest(&db).await?;

    println!("BEFORE={}", serde_json::to_string(&before)?);
    println!("CANDIDATE_COUNT={candidate_count}");
    println!("DELETED_ROWS={}", serde_json::to_string(&deleted)?);
    println!("DELETED_COUNT={}", deleted.len());
    println!("AFTER={}", serde_json::to_string(&after)?);
    Ok(())
}
