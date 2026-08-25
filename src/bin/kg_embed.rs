use anyhow::Result;

fn bool_env(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(default)
}

fn summary(stats: &surreal_mind::KgEmbedStats) -> String {
    format!(
        "Entities: updated={}, skipped={}, no_match={}, failed={}\n\
         Observations: updated={}, skipped={}, no_match={}, failed={}\n\
         Edges: updated={}, skipped={}, no_match={}, failed={}",
        stats.entities_updated,
        stats.entities_skipped,
        stats.entities_no_match,
        stats.entities_failed,
        stats.observations_updated,
        stats.observations_skipped,
        stats.observations_no_match,
        stats.observations_failed,
        stats.edges_updated,
        stats.edges_skipped,
        stats.edges_no_match,
        stats.edges_failed,
    )
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
    println!("{}", summary(&stats));
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

#[cfg(test)]
mod tests {
    use super::summary;
    use surreal_mind::KgEmbedStats;

    #[test]
    fn summary_includes_every_failure_counter() {
        let stats = KgEmbedStats {
            expected_dim: 1,
            provider: "fixture".to_string(),
            model: "fixture".to_string(),
            dry_run: false,
            entities_updated: 0,
            entities_skipped: 0,
            observations_updated: 0,
            observations_skipped: 0,
            edges_updated: 0,
            edges_skipped: 0,
            entities_no_match: 0,
            observations_no_match: 0,
            edges_no_match: 0,
            entities_failed: 1,
            observations_failed: 2,
            edges_failed: 3,
        };
        let summary = summary(&stats);
        assert!(summary.contains("failed=1"));
        assert!(summary.contains("failed=2"));
        assert!(summary.contains("failed=3"));
    }
}
