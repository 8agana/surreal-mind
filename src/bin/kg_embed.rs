use anyhow::Result;

fn bool_env(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(default)
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let dry_run = bool_env("DRY_RUN", false);
    let limit = std::env::var("LIMIT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());

    println!("🚀 KG embedding population starting (missing or mismatched only)");
    if dry_run {
        println!("🔎 Dry run: no writes to DB");
    }

    // Call the library function
    let stats = surreal_mind::run_reembed_kg(limit, dry_run).await?;

    println!("\n===== KG EMBEDDING SUMMARY =====");
    println!(
        "Entities: updated={}, skipped={}, mismatched={}, missing={}",
        stats.entities_updated,
        stats.entities_skipped,
        stats.entities_mismatched,
        stats.entities_missing
    );
    println!(
        "Observations: updated={}, skipped={}, mismatched={}, missing={}",
        stats.observations_updated,
        stats.observations_skipped,
        stats.observations_mismatched,
        stats.observations_missing
    );
    println!(
        "Edges: updated={}, skipped={}, mismatched={}, missing={}",
        stats.edges_updated,
        stats.edges_skipped,
        stats.edges_mismatched,
        stats.edges_missing
    );
    println!(
        "Provider/model: {} / {} ({} dims)",
        stats.provider, stats.model, stats.expected_dim
    );

    Ok(())
}
