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

    println!("===== KG EMBEDDING (missing only) =====");
    if dry_run {
        println!("[mode] DRY_RUN: no writes to DB");
    }
    if let Some(l) = limit {
        println!("[mode] LIMIT: {} per table", l);
    }
    println!();

    // Call the library function for missing-only embedding
    let stats = surreal_mind::run_kg_embed(limit, dry_run).await?;

    println!();
    println!("===== KG EMBEDDING SUMMARY =====");
    println!(
        "Entities:     updated={}, skipped={}",
        stats.entities_updated, stats.entities_skipped
    );
    println!(
        "Observations: updated={}, skipped={}",
        stats.observations_updated, stats.observations_skipped
    );
    println!(
        "Edges:        updated={}, skipped={}",
        stats.edges_updated, stats.edges_skipped
    );
    println!();
    println!(
        "Provider: {} | Model: {} | Dims: {}",
        stats.provider, stats.model, stats.expected_dim
    );
    if stats.dry_run {
        println!("[DRY_RUN] No changes were made");
    }

    Ok(())
}
