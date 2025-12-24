use chrono::{Duration, Utc};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client;

const SESSION_TTL_HOURS: i64 = 24; // Sessions older than this are considered stale

pub async fn get_tool_session(
    db: &Surreal<Client>,
    tool_name: String,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let cutoff = Utc::now() - Duration::hours(SESSION_TTL_HOURS);

    let sql = r#"
        SELECT gemini_session_id, last_used
        FROM tool_sessions
        WHERE tool_name = $tool_name
          AND last_used > $cutoff
        ORDER BY last_used DESC
        LIMIT 1
    "#;

    let mut result = db
        .query(sql)
        .bind(("tool_name", tool_name))
        .bind(("cutoff", cutoff))
        .await?;

    #[derive(serde::Deserialize)]
    struct SessionRow {
        gemini_session_id: String,
        #[allow(dead_code)]
        last_used: chrono::DateTime<chrono::Utc>,
    }

    let rows: Vec<SessionRow> = result.take(0)?;
    Ok(rows.into_iter().next().map(|r| r.gemini_session_id))
}

pub async fn store_tool_session(
    db: &Surreal<Client>,
    tool_name: String,
    session_id: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let sql = r#"
        UPSERT tool_sessions
        SET gemini_session_id = $session_id,
            last_used = time::now()
        WHERE tool_name = $tool_name
    "#;

    db.query(sql)
        .bind(("tool_name", tool_name))
        .bind(("session_id", session_id))
        .await?;

    Ok(())
}

pub async fn clear_tool_session(
    db: &Surreal<Client>,
    tool_name: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let sql = "DELETE FROM tool_sessions WHERE tool_name = $tool_name";
    db.query(sql).bind(("tool_name", tool_name)).await?;
    Ok(())
}
