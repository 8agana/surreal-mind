//! Dry-run contract for the KG re-embed path, and for the standalone
//! `thoughts` re-embed path used by the `reembed` binary
//! (`src/bin/reembed.rs`).
//!
//! The bug this guards: `run_reembed_kg` used to call the embedding provider for
//! every candidate row and only guard the `UPDATE` behind `dry_run`. Both
//! reachable entry points (`maintain(action: "reembed_kg")` and the `reembed_kg`
//! binary) therefore billed a provider once per row on a "dry" run.
//!
//! The embedder is the provider sink, so the embedder is what these tests count.
//! A counting mock is injected through [`run_reembed_kg_with`] and
//! [`run_reembed_standalone_with`] together with a disposable SurrealDB, which
//! also means these tests need no API key and can never reach a real provider.
//!
//! The `reembed_standalone_*` tests below are the library-level negative
//! control for the standalone `reembed` binary's dry-run contract: they cover
//! the same "dry_run=false calls the provider and writes; dry_run=true does
//! neither" property that `tests/reembed_bin_dry_run.rs` used to prove by
//! spawning a live (non-dry) run of the compiled binary against a real
//! `api.openai.com` request. That subprocess-level negative control has been
//! removed (see that file's module doc comment) in favor of this in-process
//! one, which needs no network access at all.
//!
//! Gated behind the `db_integration` feature AND `RUN_DB_TESTS`, and refuses to
//! run against anything that looks like the production endpoint.
#![cfg(feature = "db_integration")]

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use surreal_mind::embeddings::Embedder;
use surreal_mind::maintenance::reembed::{run_reembed_kg_with, run_reembed_standalone_with};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;

const MOCK_DIMS: usize = 4;
/// Must stay inside `run_reembed_kg`'s "already current" model allowlist so the
/// up-to-date fixture row is genuinely skipped rather than accidentally stale.
const TARGET_MODEL: &str = "text-embedding-3-small";

/// Counting mock embedder: every `embed` call is a provider call that a dry run
/// must never make. Returns a fixed-dimension vector so the real
/// `ensure_generated_embedding_dimension` guard is exercised on the live path.
#[derive(Default)]
struct CountingEmbedder {
    calls: AtomicUsize,
}

impl CountingEmbedder {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Embedder for CountingEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![0.25f32; MOCK_DIMS])
    }

    fn dimensions(&self) -> usize {
        MOCK_DIMS
    }
}

/// `None` means "do not run" — the caller returns Ok(()) so the DB-free suite
/// stays green and a misconfigured environment can never touch production.
async fn disposable_db() -> Result<Option<(Surreal<Client>, String)>> {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("skipping: set RUN_DB_TESTS=1 to run the reembed dry-run contract tests");
        return Ok(None);
    }
    let url = std::env::var("SURR_TEST_DB_URL").unwrap_or_else(|_| "127.0.0.1:8100".to_string());
    if url.contains(":8000") {
        eprintln!(
            "refusing to run against {url}: that is the production SurrealDB endpoint. \
             Point SURR_TEST_DB_URL at a disposable instance (e.g. 127.0.0.1:8100)."
        );
        return Ok(None);
    }

    let db = Surreal::new::<Ws>(url.as_str()).await?;
    db.signin(Root {
        username: "root".to_string(),
        password: "root".to_string(),
    })
    .await?;

    // Unique namespace per test run: other workers share this instance, and a
    // fresh namespace means these tests never need DELETE or REMOVE TABLE.
    let suffix = format!(
        "{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let ns = format!("reembed_dryrun_{suffix}");
    db.query(format!("DEFINE NAMESPACE IF NOT EXISTS {ns}"))
        .await?
        .check()?;
    db.use_ns(&ns).await?;
    db.query("DEFINE DATABASE IF NOT EXISTS scratch")
        .await?
        .check()?;
    db.use_ns(&ns).use_db("scratch").await?;
    Ok(Some((db, ns)))
}

/// Seed three entities (missing / stale / up-to-date), one observation, and one
/// edge, so all three of `run_reembed_kg`'s loops have work to do.
async fn seed(db: &Surreal<Client>) -> Result<()> {
    db.query(
        "CREATE kg_entities:missing SET name = 'dryrun-missing', entity_type = 'test', data = {}, embedding = NONE;
         CREATE kg_entities:stale SET name = 'dryrun-stale', entity_type = 'test', data = {}, embedding = [0.1f, 0.2f], embedding_model = $model, embedding_dim = 2;
         CREATE kg_entities:current SET name = 'dryrun-current', entity_type = 'test', data = {}, embedding = [0.1f, 0.2f, 0.3f, 0.4f], embedding_model = $model, embedding_dim = 4;
         CREATE kg_observations:missing SET name = 'dryrun-obs', data = { description: 'obs body' }, embedding = NONE;
         CREATE kg_edges:missing SET source = kg_entities:missing, target = kg_entities:stale, rel_type = 'relates_to', data = { description: 'edge body' }, embedding = NONE;",
    )
    .bind(("model", TARGET_MODEL))
    .await?
    .check()?;
    Ok(())
}

/// Canonical snapshot of every embedding-bearing column the re-embed path can
/// write, across all three tables. String comparison of this is the
/// "database did not change" assertion.
async fn snapshot(db: &Surreal<Client>) -> Result<String> {
    let mut out = String::new();
    for table in ["kg_entities", "kg_observations", "kg_edges"] {
        let rows: Vec<serde_json::Value> = db
            .query(format!(
                "SELECT meta::id(id) AS id, embedding, embedding_model, embedding_provider, \
                 embedding_dim, embedded_at FROM {table} ORDER BY id"
            ))
            .await?
            .take(0)?;
        out.push_str(table);
        out.push('=');
        out.push_str(&serde_json::to_string(&rows)?);
        out.push('\n');
    }
    Ok(out)
}

/// Seed three `thoughts` rows exercising all three `ThoughtReembedDecision`
/// branches `run_reembed_standalone_with`'s selection predicate
/// (`classify_thought_reembed`) reads off `array::len(embedding)` and
/// `embedding_model`, mirroring `tests/reembed_bin_dry_run.rs::seed`:
///   - `thoughts:missing`    -- embedding = NONE                 -> Missing    (needs embedding)
///   - `thoughts:mismatched` -- 2-length embedding, wrong model  -> Mismatched (needs embedding)
///   - `thoughts:current`    -- MOCK_DIMS-length embedding, TARGET_MODEL -> UpToDate (must be skipped)
async fn seed_thoughts(db: &Surreal<Client>) -> Result<()> {
    db.query(
        "CREATE thoughts:missing SET content = 'standalone-dryrun-missing', embedding = NONE;
         CREATE thoughts:mismatched SET content = 'standalone-dryrun-mismatched', embedding = [0.1f, 0.2f], embedding_model = 'old-model', embedding_provider = 'openai', embedding_dim = 2;
         CREATE thoughts:current SET content = 'standalone-dryrun-current', embedding = [0.1f, 0.2f, 0.3f, 0.4f], embedding_model = $model, embedding_provider = 'openai', embedding_dim = 4;",
    )
    .bind(("model", TARGET_MODEL))
    .await?
    .check()?;
    Ok(())
}

/// Canonical snapshot of every embedding-bearing column
/// `run_reembed_standalone_with` can write, mirroring
/// `tests/reembed_bin_dry_run.rs::snapshot`.
async fn snapshot_thoughts(db: &Surreal<Client>) -> Result<String> {
    let rows: Vec<serde_json::Value> = db
        .query(
            "SELECT meta::id(id) AS id, embedding, embedding_model, embedding_provider, \
             embedding_dim, embedded_at FROM thoughts ORDER BY id",
        )
        .await?
        .take(0)?;
    Ok(serde_json::to_string(&rows)?)
}

/// THE CONTRACT: `dry_run = true` makes zero provider calls and zero writes,
/// while still reporting how many rows would be re-embedded.
#[tokio::test]
async fn reembed_kg_dry_run_makes_no_provider_calls_and_no_writes() -> Result<()> {
    let Some((db, _ns)) = disposable_db().await? else {
        return Ok(());
    };
    seed(&db).await?;

    let before = snapshot(&db).await?;
    let embedder = Arc::new(CountingEmbedder::default());

    let stats = run_reembed_kg_with(
        &db,
        embedder.as_ref(),
        "mock-provider",
        TARGET_MODEL,
        None,
        true,
    )
    .await?;

    assert_eq!(
        embedder.calls(),
        0,
        "a dry run must make ZERO embedding-provider calls; made {}. Stats: {stats:?}",
        embedder.calls()
    );

    let after = snapshot(&db).await?;
    assert_eq!(
        before, after,
        "a dry run must not mutate any row of kg_entities/kg_observations/kg_edges"
    );

    // The report must still be useful: it names the rows a live run would touch.
    assert_eq!(
        stats.entities_updated, 2,
        "dry run should report the 2 candidate entities (missing + stale); stats={stats:?}"
    );
    assert_eq!(
        stats.entities_skipped, 1,
        "the already-current entity must be skipped, not reported; stats={stats:?}"
    );
    assert_eq!(
        stats.observations_updated, 1,
        "dry run should report the 1 candidate observation; stats={stats:?}"
    );
    assert_eq!(
        stats.edges_updated, 1,
        "dry run should report the 1 candidate edge; stats={stats:?}"
    );
    assert!(stats.dry_run, "stats must record that this was a dry run");
    assert_eq!(
        (
            stats.entities_failed,
            stats.observations_failed,
            stats.edges_failed
        ),
        (0, 0, 0),
        "a dry run cannot fail a provider call it never makes; stats={stats:?}"
    );

    Ok(())
}

/// Negative control: the same fixtures with `dry_run = false` DO call the
/// provider and DO change the database. Without this, the test above would pass
/// against a function that silently does nothing at all.
#[tokio::test]
async fn reembed_kg_live_run_does_call_provider_and_does_write() -> Result<()> {
    let Some((db, _ns)) = disposable_db().await? else {
        return Ok(());
    };
    seed(&db).await?;

    let before = snapshot(&db).await?;
    let embedder = Arc::new(CountingEmbedder::default());

    let stats = run_reembed_kg_with(
        &db,
        embedder.as_ref(),
        "mock-provider",
        TARGET_MODEL,
        None,
        false,
    )
    .await?;

    assert_eq!(
        embedder.calls(),
        4,
        "a live run must embed the 2 candidate entities + 1 observation + 1 edge; \
         got {} calls. Stats: {stats:?}",
        embedder.calls()
    );

    let after = snapshot(&db).await?;
    assert_ne!(
        before, after,
        "a live run must actually change the stored embeddings (negative control \
         for the dry-run snapshot assertion)"
    );

    assert_eq!(
        stats.entities_updated, 2,
        "live run should have updated both candidate entities; stats={stats:?}"
    );
    assert_eq!(
        (
            stats.entities_no_match,
            stats.observations_no_match,
            stats.edges_no_match
        ),
        (0, 0, 0),
        "every UPDATE targets a freshly created row and must match; stats={stats:?}"
    );
    assert!(
        !stats.dry_run,
        "stats must record that this was NOT a dry run"
    );

    Ok(())
}

/// THE CONTRACT for the standalone `thoughts` path: `dry_run = true` makes
/// zero provider calls and zero writes, while still reporting how many rows
/// would be re-embedded. This is `run_reembed_standalone_with`'s half of the
/// contract the `reembed` binary itself is a thin wrapper around.
#[tokio::test]
async fn reembed_standalone_dry_run_makes_no_provider_calls_and_no_writes() -> Result<()> {
    let Some((db, _ns)) = disposable_db().await? else {
        return Ok(());
    };
    seed_thoughts(&db).await?;

    let before = snapshot_thoughts(&db).await?;
    let embedder = Arc::new(CountingEmbedder::default());

    let report =
        run_reembed_standalone_with(&db, embedder.as_ref(), "mock-provider", TARGET_MODEL, true)
            .await?;

    assert_eq!(
        embedder.calls(),
        0,
        "a dry run must make ZERO embedding-provider calls; made {}. Report: {report:?}",
        embedder.calls()
    );

    let after = snapshot_thoughts(&db).await?;
    assert_eq!(
        before, after,
        "a dry run must not mutate any row of thoughts"
    );

    assert_eq!(
        report.would_reembed, 2,
        "dry run should report the 2 candidate rows (missing + mismatched); report={report:?}"
    );
    assert_eq!(
        report.skipped, 1,
        "the already-current row must be skipped, not reported; report={report:?}"
    );
    assert!(report.dry_run, "report must record that this was a dry run");
    assert_eq!(
        report.error, 0,
        "a dry run cannot fail a provider call it never makes; report={report:?}"
    );

    Ok(())
}

/// Negative control for the standalone `thoughts` path: the same fixtures
/// with `dry_run = false` DO call the provider and DO change the database.
/// Without this, the test above would pass against a function that silently
/// does nothing at all. This replaces the subprocess-level negative control
/// `tests/reembed_bin_dry_run.rs` used to run against a real OpenAI request
/// (see that file's module doc comment) — the assertion is the same
/// property, proven here with an injected mock instead of live production
/// infrastructure.
#[tokio::test]
async fn reembed_standalone_live_run_does_call_provider_and_does_write() -> Result<()> {
    let Some((db, _ns)) = disposable_db().await? else {
        return Ok(());
    };
    seed_thoughts(&db).await?;

    let before = snapshot_thoughts(&db).await?;
    let embedder = Arc::new(CountingEmbedder::default());

    let report =
        run_reembed_standalone_with(&db, embedder.as_ref(), "mock-provider", TARGET_MODEL, false)
            .await?;

    assert_eq!(
        embedder.calls(),
        2,
        "a live run must embed the 2 candidate rows (missing + mismatched); \
         got {} calls. Report: {report:?}",
        embedder.calls()
    );

    let after = snapshot_thoughts(&db).await?;
    assert_ne!(
        before, after,
        "a live run must actually change the stored embeddings (negative control \
         for the dry-run snapshot assertion)"
    );

    assert_eq!(
        report.success, 2,
        "live run should have updated both candidate rows; report={report:?}"
    );
    assert_eq!(
        report.error, 0,
        "every UPDATE targets a freshly created row and must match; report={report:?}"
    );
    assert!(
        !report.dry_run,
        "report must record that this was NOT a dry run"
    );

    Ok(())
}
