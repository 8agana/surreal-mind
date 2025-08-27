use anyhow::Result;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    surreal_mind::load_env();
    let mut batch_size: usize = env::var("REEMBED_BATCH").ok().and_then(|v| v.parse().ok()).unwrap_or(64);
    let mut limit: Option<usize> = env::var("REEMBED_LIMIT").ok().and_then(|v| v.parse().ok());
    let mut missing_only: bool = env::var("REEMBED_MISSING_ONLY").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(true);
    let mut dry_run: bool = env::var("REEMBED_DRY_RUN").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--batch-size" => if let Some(v) = args.next() { batch_size = v.parse().unwrap_or(batch_size); },
            "--limit" => if let Some(v) = args.next() { limit = v.parse().ok(); },
            "--missing-only" => missing_only = true,
            "--all" => missing_only = false,
            "--dry-run" => dry_run = true,
            _ => {}
        }
    }
    batch_size = batch_size.clamp(1, 512);

    eprintln!("Re-embedding thoughts: batch_size={}, limit={:?}, missing_only={}, dry_run={}", batch_size, limit, missing_only, dry_run);
    let stats = surreal_mind::run_reembed(batch_size, limit, missing_only, dry_run).await?;
    println!("{}", serde_json::to_string(&stats).unwrap());
    Ok(())
}
