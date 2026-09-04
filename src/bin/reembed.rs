use anyhow::Result;
// use chrono::Utc;
use surreal_mind::embeddings::create_embedder;
use surreal_mind::maintenance::reembed::run_reembed_standalone_with;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

/// Dry-run is requested by `--dry-run` on the command line OR by a truthy
/// `DRY_RUN` environment variable. The truthiness table is copied verbatim from
/// the `bool_env` helper used by `reembed_kg`, `kg_embed`, and `kg_wander` so
/// this binary cannot disagree with its siblings about what `DRY_RUN=yes` means.
///
/// Pure so it can be unit-tested without a process environment.
fn dry_run_requested(args: &[String], env_value: Option<&str>) -> bool {
    let env_on = env_value
        .map(|v| matches!(v, "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false);
    env_on || args.iter().any(|a| a == "--dry-run")
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment from .env file
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Warning: Could not load .env file: {}", e);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let dry_run = dry_run_requested(&args, std::env::var("DRY_RUN").ok().as_deref());

    // Load configuration
    let config = surreal_mind::config::Config::load().map_err(|e| {
        eprintln!("Failed to load configuration: {}", e);
        e
    })?;

    println!("🚀 Starting thought re-embedding process...");
    if dry_run {
        println!("🔎 DRY RUN: no embedding-provider calls and no writes will be made");
    }
    // Prefer OpenAI 1536; fallback to local BGE if unavailable
    let embedder = create_embedder(&config).await?;
    let embed_dims = embedder.dimensions();
    println!(
        "✅ Embedder initialized (expected primary dims = {}), provider/model from config",
        embed_dims
    );

    // Connect to SurrealDB using config
    let db = Surreal::new::<Ws>(&config.system.database_url).await?;
    db.signin(Root {
        username: config.runtime.database_user.clone(),
        password: config.runtime.database_pass.clone(),
    })
    .await?;
    db.use_ns(&config.system.database_ns)
        .use_db(&config.system.database_db)
        .await?;

    // All behaviour (including the dry-run contract) lives in
    // `run_reembed_standalone_with`, so this binary and any future callers
    // cannot drift apart from what the tests exercise. This binary's job is
    // just: parse args/env, build config/embedder/db, call the runner, print
    // the final summary from the returned report.
    let report = run_reembed_standalone_with(
        &db,
        embedder.as_ref(),
        &config.system.embedding_provider,
        &config.system.embedding_model,
        dry_run,
    )
    .await?;

    // Final statistics
    println!("\n{}", "=".repeat(50));
    if dry_run {
        println!("📊 RE-EMBEDDING DRY RUN COMPLETE (no provider calls, no writes)");
        println!("🔎 Would re-embed: {} thoughts", report.would_reembed);
    } else {
        println!("📊 RE-EMBEDDING COMPLETE!");
    }
    println!("✅ Successfully re-embedded: {} thoughts", report.success);
    println!(
        "⏭️  Skipped (already target dims={} & model): {} thoughts",
        report.expected_dim, report.skipped
    );
    println!("❌ Errors: {} thoughts", report.error);
    println!("🧪 Mismatched dims/model: {}", report.mismatched);
    println!("∅ Missing embeddings: {}", report.missing);
    println!("🎯 Target embedding dimensions: {}", report.expected_dim);
    println!("{}", "=".repeat(50));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::dry_run_requested;

    #[test]
    fn dry_run_flag_is_honored() {
        let args = vec!["--dry-run".to_string()];
        assert!(dry_run_requested(&args, None));
    }

    #[test]
    fn dry_run_env_truthiness_matches_sibling_binaries() {
        let none: Vec<String> = Vec::new();
        for truthy in ["1", "true", "TRUE", "yes", "on"] {
            assert!(
                dry_run_requested(&none, Some(truthy)),
                "DRY_RUN={truthy} must enable dry run"
            );
        }
        for falsy in ["0", "false", "no", "off", ""] {
            assert!(
                !dry_run_requested(&none, Some(falsy)),
                "DRY_RUN={falsy} must not enable dry run"
            );
        }
        assert!(!dry_run_requested(&none, None));
    }

    #[test]
    fn unrelated_args_do_not_enable_dry_run() {
        let args = vec!["--verbose".to_string(), "dry-run".to_string()];
        assert!(!dry_run_requested(&args, None));
    }
}
