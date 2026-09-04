use anyhow::Result;
// use chrono::Utc;
use surreal_mind::embeddings::create_embedder;
use surreal_mind::maintenance::reembed::{ThoughtReembedDecision, classify_thought_reembed};
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

/// Truncate for a log line without splitting a UTF-8 code point.
fn preview(text: &str) -> String {
    text.chars().take(60).collect()
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

    // Show current distribution by provider/model/dimension
    println!("\n📊 Current embedding distribution (before re-embed):");
    let dist_rows: Vec<serde_json::Value> = db
        .query(
            "SELECT embedding_provider as provider, embedding_model as model, embedding_dim as dim, count() as count FROM thoughts GROUP BY embedding_provider, embedding_model, embedding_dim ORDER BY count DESC"
        )
        .await?
        .take(0)?;
    if dist_rows.is_empty() {
        println!("  (no existing embeddings found)");
    } else {
        for r in &dist_rows {
            let prov = r.get("provider").and_then(|v| v.as_str()).unwrap_or("NONE");
            let model = r.get("model").and_then(|v| v.as_str()).unwrap_or("NONE");
            let dim = r.get("dim").and_then(|v| v.as_i64()).unwrap_or(0);
            let count = r.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
            println!(
                "  - {:>6} dims | {:<8} | {:<28} | {:>6} items",
                dim, prov, model, count
            );
        }
    }

    // Get all thoughts using a raw query with meta::id() to avoid Thing serialization
    println!("\n📚 Fetching all thoughts from database...");
    let result = db
        .query("SELECT meta::id(id) as id, content, (IF type::is_array(embedding) THEN array::len(embedding) ELSE 0 END) as emb_len, embedding_model, embedding_provider, embedding_dim FROM thoughts")
        .await?;
    let mut response = result.check()?;
    let thoughts: Vec<serde_json::Value> = response.take(0)?;
    println!("✅ Found {} thoughts to process", thoughts.len());

    let mut success_count = 0;
    let mut skip_count = 0;
    let mut error_count = 0;
    let mut mismatched_count = 0;
    let mut missing_count = 0;
    let mut would_reembed_count = 0;

    println!("\n🔄 Re-embedding thoughts with configured embedder (OpenAI→BGE fallback)...");
    for i in 0..thoughts.len() {
        let thought = &thoughts[i];
        // Progress indicator
        if i % 10 == 0 && i > 0 {
            println!(
                "  Progress: {}/{} ({}%)",
                i,
                thoughts.len(),
                i * 100 / thoughts.len()
            );
        }

        // Extract fields
        let thought_id = thought["id"].as_str().unwrap_or("unknown").to_string();
        let content = thought["content"].as_str().unwrap_or("").to_string();
        let existing_emb_len = thought["emb_len"].as_u64().unwrap_or(0) as usize;
        let existing_model = thought["embedding_model"].as_str().unwrap_or("");

        // Hygiene counts and the skip decision now come from one shared, pure
        // classifier so a dry run reports exactly the rows a live run would
        // re-embed (same predicate, not a parallel reimplementation of it).
        let decision = classify_thought_reembed(
            existing_emb_len,
            existing_model,
            embed_dims,
            &config.system.embedding_model,
        );
        match decision {
            ThoughtReembedDecision::Missing => missing_count += 1,
            ThoughtReembedDecision::Mismatched => mismatched_count += 1,
            ThoughtReembedDecision::UpToDate => {}
        }

        // Skip if already embedded with the current embedder's dimensions AND model matches config model
        if !decision.needs_embedding() {
            skip_count += 1;
            continue;
        }

        // DRY-RUN GUARD: branch BEFORE the provider call and before the UPDATE.
        // This binary previously ignored DRY_RUN entirely: it embedded and wrote
        // every non-matching row no matter how it was invoked.
        if dry_run {
            would_reembed_count += 1;
            if would_reembed_count <= 3 {
                eprintln!(
                    "  🔎 [dry_run] Would re-embed {} ({:?}): \"{}\"",
                    thought_id,
                    decision,
                    preview(&content)
                );
            }
            continue;
        }

        // Generate new embedding
        match embedder.embed(&content).await {
            Ok(new_embedding) => {
                if let Err(e) = surreal_mind::embeddings::ensure_generated_embedding_dimension(
                    &new_embedding,
                    embed_dims,
                ) {
                    error_count += 1;
                    eprintln!(
                        "  ⚠️  Refusing wrong-dimension embedding for {}: {}",
                        thought_id, e
                    );
                    continue;
                }
                // Update thought with new embedding and metadata
                let (provider, model) = (
                    config.system.embedding_provider.clone(),
                    config.system.embedding_model.clone(),
                );
                let query = "UPDATE type::record('thoughts', $id) SET embedding = $embedding, embedding_provider = $provider, embedding_model = $model, embedding_dim = $dims, embedded_at = time::now() RETURN meta::id(id) as id";

                match db
                    .query(query)
                    .bind(("id", thought_id.clone()))
                    .bind(("embedding", new_embedding))
                    .bind(("provider", provider.clone()))
                    .bind(("model", model.clone()))
                    .bind(("dims", embed_dims as i64))
                    .await
                {
                    // `Ok(response)` from `.query().await` only means the driver
                    // received a response, not that this UPDATE matched a row —
                    // same per-statement-error semantics as N-2/N-5. Only count a
                    // success when `.take(0)` yields a non-empty result array for
                    // the RETURN clause above.
                    Ok(mut response) => match response.take::<Vec<serde_json::Value>>(0) {
                        Ok(rows) if !rows.is_empty() => {
                            success_count += 1;
                            if i < 3 {
                                eprintln!(
                                    "  ✅ Updated {} with provider={}, model={}, dims={}",
                                    thought_id, provider, model, embed_dims
                                );
                                eprintln!("     Rows: {:?}", rows);
                            }
                        }
                        Ok(_) => {
                            error_count += 1;
                            eprintln!(
                                "  ⚠️  Update for {} matched 0 rows; not counted as success",
                                thought_id
                            );
                        }
                        Err(e) => {
                            error_count += 1;
                            eprintln!(
                                "  ⚠️  Statement error verifying update for {}: {}",
                                thought_id, e
                            );
                        }
                    },
                    Err(e) => {
                        error_count += 1;
                        eprintln!("  ⚠️  Failed to update {}: {}", thought_id, e);
                    }
                }
            }
            Err(e) => {
                error_count += 1;
                eprintln!("  ⚠️  Failed to embed content for {}: {}", thought_id, e);
            }
        }
    }

    // Final statistics
    println!("\n{}", "=".repeat(50));
    if dry_run {
        println!("📊 RE-EMBEDDING DRY RUN COMPLETE (no provider calls, no writes)");
        println!("🔎 Would re-embed: {} thoughts", would_reembed_count);
    } else {
        println!("📊 RE-EMBEDDING COMPLETE!");
    }
    println!("✅ Successfully re-embedded: {} thoughts", success_count);
    println!(
        "⏭️  Skipped (already target dims={} & model): {} thoughts",
        embed_dims, skip_count
    );
    println!("❌ Errors: {} thoughts", error_count);
    println!("🧪 Mismatched dims/model: {}", mismatched_count);
    println!("∅ Missing embeddings: {}", missing_count);
    println!("🎯 Target embedding dimensions: {}", embed_dims);
    println!("{}", "=".repeat(50));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{dry_run_requested, preview};

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

    #[test]
    fn preview_does_not_split_multibyte_characters() {
        let text = "\u{e9}".repeat(100);
        let out = preview(&text);
        assert_eq!(out.chars().count(), 60);
    }
}
