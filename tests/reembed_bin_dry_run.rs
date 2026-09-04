//! Binary-level dry-run contract for `src/bin/reembed.rs` (the `thoughts`
//! table re-embed CLI), companion to `tests/reembed_dry_run_contract.rs`
//! which covers `run_reembed_kg_with` (the `kg_entities`/`kg_observations`/
//! `kg_edges` path via a function call with an injected mock embedder).
//!
//! Commit aeceb2b taught the `reembed` binary to respect `--dry-run` and
//! `DRY_RUN` before it ever calls the embedding provider or writes anything
//! (the guard sits at src/bin/reembed.rs:145-159, right after the
//! `classify_thought_reembed`/`needs_embedding()` skip check and strictly
//! before `embedder.embed(...)`). The only coverage that guard had was two
//! unit tests on the pure `dry_run_requested` arg/env parser at the bottom
//! of that file -- nothing exercised the actual dry-run BEHAVIOR: that a
//! real invocation of the compiled binary makes zero provider calls and
//! zero writes.
//!
//! Unlike `reembed_dry_run_contract.rs`, the `reembed` binary cannot take an
//! injected mock `Embedder` -- it always builds a real `OpenAIEmbedder` via
//! `create_embedder(&config)` (src/bin/reembed.rs:48), constructed
//! unconditionally before the dry-run branch. So this test spawns the
//! actual compiled `reembed` binary as a subprocess (never a function call),
//! the same `env!("CARGO_BIN_EXE_<name>")` pattern
//! `tests/stdio_smoke.rs:71-78` uses for `surreal-mind` itself, points it at
//! a disposable SurrealDB via `SURR_DB_URL`/`SURR_DB_NS`/`SURR_DB_DB`, and
//! feeds it an OBVIOUSLY-fake `OPENAI_API_KEY` so a live (non-dry) run
//! actually reaches (and is rejected by) the real OpenAI API instead of
//! silently short-circuiting on a placeholder-shaped key
//! (`embeddings::create_embedder`'s `is_placeholder` check only rejects
//! empty/`${...}`/`your-api-key-here`/`changeme` -- a fake-but-well-formed
//! key like `sk-fake-...` sails through construction and is only rejected
//! by the OpenAI API itself at request time, which is exactly the behavior
//! this test's negative control needs).
//!
//! Gated on `RUN_DB_TESTS=1` and refuses to run against anything that looks
//! like the production SurrealDB endpoint (`:8000`) -- mirrors
//! `tests/reembed_dry_run_contract.rs`'s `disposable_db()` guard
//! (duplicated here rather than shared, since `tests/*.rs` files are each
//! their own crate and this repo has no `tests/common` module).
//!
//! Run: `RUN_DB_TESTS=1 SURR_TEST_DB_URL=127.0.0.1:8100 cargo test --test
//! reembed_bin_dry_run -- --nocapture` (env var names match
//! `tests/reembed_dry_run_contract.rs` exactly). Without `RUN_DB_TESTS` set,
//! every test below returns `Ok(())` immediately after printing a skip
//! notice, so the DB-free `cargo test` suite stays green and never spawns
//! the binary or touches a socket.

use anyhow::{Context, Result};
use std::process::{Command, Output};
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;

/// Same default and production-endpoint refusal as
/// `tests/reembed_dry_run_contract.rs::disposable_db` (`:8000` is the live
/// production SurrealMind endpoint per AGENTS.md/CLAUDE.md; the throwaway
/// instance this repair pass uses lives on `127.0.0.1:8100`).
const DEFAULT_TEST_DB_URL: &str = "127.0.0.1:8100";

/// `None` means "do not run" -- callers return `Ok(())` so the DB-free suite
/// stays green and a misconfigured environment can never touch production.
fn test_db_url() -> Option<String> {
    if std::env::var("RUN_DB_TESTS").is_err() {
        eprintln!("skipping: set RUN_DB_TESTS=1 to run the reembed binary dry-run contract tests");
        return None;
    }
    let url = std::env::var("SURR_TEST_DB_URL").unwrap_or_else(|_| DEFAULT_TEST_DB_URL.to_string());
    if url.contains(":8000") {
        eprintln!(
            "refusing to run against {url}: that is the production SurrealDB endpoint. \
             Point SURR_TEST_DB_URL at a disposable instance (e.g. 127.0.0.1:8100)."
        );
        return None;
    }
    Some(url)
}

/// Fixed, well-formed-but-fake key. Deliberately NOT the literal `changeme`
/// that ships in this worktree's `.env` (that string is a placeholder in
/// name only -- `embeddings::create_embedder`'s `is_placeholder` check is an
/// exact case-insensitive match on `"changeme"`/`"your-api-key-here"`/empty/
/// `${...}`, so `changeme-fed734b8f-worktree` does NOT match it and would
/// be treated as a usable key). This key is shaped like a real OpenAI key
/// so it passes that same construction-time check and only fails at the
/// OpenAI API itself, which is what the negative control below needs.
const FAKE_OPENAI_KEY: &str = "sk-fake-fed734b8f";

/// Target model/dims this test seeds against: matches both this worktree's
/// `surreal_mind.toml` ([system] embedding_provider="openai",
/// embedding_model="text-embedding-3-small", embedding_dimensions=1536) and
/// `Config::default()`'s hardcoded fallback, so the OpenAI embedder the
/// binary constructs reports `dimensions() == TARGET_DIM` regardless of
/// whether a config file is present.
const TARGET_MODEL: &str = "text-embedding-3-small";
const TARGET_DIM: usize = 1536;

async fn connect(url: &str, ns: &str, db: &str) -> Result<Surreal<Client>> {
    let conn = Surreal::new::<Ws>(url)
        .await
        .with_context(|| format!("connect to disposable SurrealDB at {url}"))?;
    conn.signin(Root {
        username: "root".to_string(),
        password: "root".to_string(),
    })
    .await?;
    conn.query(format!("DEFINE NAMESPACE IF NOT EXISTS {ns}"))
        .await?
        .check()?;
    conn.use_ns(ns).await?;
    conn.query(format!("DEFINE DATABASE IF NOT EXISTS {db}"))
        .await?
        .check()?;
    conn.use_ns(ns).use_db(db).await?;
    Ok(conn)
}

/// Unique per-test-run ns/db pair (pid + nanosecond timestamp) so parallel
/// `cargo test` runs against the same shared throwaway instance never
/// collide and never need DELETE/REMOVE TABLE cleanup.
fn unique_ns_db(tag: &str) -> (String, String) {
    let suffix = format!(
        "{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_nanos()
    );
    (format!("reembed_bin_{tag}_{suffix}"), "scratch".to_string())
}

/// Seed 3 `thoughts` rows exercising all three `ThoughtReembedDecision`
/// branches the binary's selection predicate
/// (`classify_thought_reembed`, src/maintenance/reembed.rs:87-100) reads
/// off `array::len(embedding)` and `embedding_model`:
///   - `thoughts:missing`    -- embedding = NONE          -> Missing    (needs embedding)
///   - `thoughts:mismatched` -- 4-length embedding, wrong model -> Mismatched (needs embedding)
///   - `thoughts:current`    -- TARGET_DIM-length embedding, TARGET_MODEL -> UpToDate (must be skipped)
///
/// This is a fresh, never-schema'd namespace/database (see `connect`
/// above), so `thoughts` here is an ad-hoc SCHEMALESS table -- unlike the
/// real production `thoughts` table (SCHEMAFULL, see
/// src/server/schema.rs:28), this scratch table has none of the other
/// required-with-no-default fields (`injected_memories`, `injection_scale`,
/// `significance`, `access_count`, ...) that `tests/dimension_hygiene.rs`
/// has to supply when it seeds via `SurrealMindServer::new()` against a
/// schema'd namespace. The binary's own SELECT
/// (src/bin/reembed.rs:92) only ever reads `content`, `embedding`,
/// `embedding_model`, `embedding_provider`, `embedding_dim`, so that's all
/// this fixture needs to provide.
async fn seed(db: &Surreal<Client>) -> Result<()> {
    db.query(
        r#"
        CREATE thoughts:missing SET content = "reembed-bin-dryrun seed: missing", embedding = NONE;
        CREATE thoughts:mismatched SET content = "reembed-bin-dryrun seed: mismatched", embedding = array::range(0, 4), embedding_model = "old-model", embedding_provider = "openai", embedding_dim = 4;
        CREATE thoughts:current SET content = "reembed-bin-dryrun seed: current", embedding = array::range(0, $dim), embedding_model = $model, embedding_provider = "openai", embedding_dim = $dim;
        "#,
    )
    .bind(("dim", TARGET_DIM as i64))
    .bind(("model", TARGET_MODEL))
    .await?
    .check()?;
    Ok(())
}

/// Canonical snapshot of every embedding-bearing column the binary can
/// write. String comparison of this across a run is the "database did not
/// change" assertion, mirroring
/// `tests/reembed_dry_run_contract.rs::snapshot`.
async fn snapshot(db: &Surreal<Client>) -> Result<String> {
    let rows: Vec<serde_json::Value> = db
        .query(
            "SELECT meta::id(id) AS id, embedding, embedding_model, embedding_provider, \
             embedding_dim, embedded_at FROM thoughts ORDER BY id",
        )
        .await?
        .take(0)?;
    Ok(serde_json::to_string(&rows)?)
}

/// Spawn the compiled `reembed` binary (never the library function) against
/// the disposable ns/db, with a fixed fake OpenAI key and a fast retry
/// budget (`SURR_EMBED_RETRIES=1`) so a live-run negative control fails
/// after exactly one real HTTP round trip instead of `Config::default`'s 3
/// retries with exponential backoff. `extra_args`/`extra_env` let each test
/// add `--dry-run` or `DRY_RUN=1` without duplicating the whole spawn.
fn run_binary(
    url: &str,
    ns: &str,
    db: &str,
    extra_args: &[&str],
    extra_env: &[(&str, &str)],
) -> Result<Output> {
    let bin = env!("CARGO_BIN_EXE_reembed");
    let mut cmd = Command::new(bin);
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("SURR_DB_URL", url)
        .env("SURR_DB_NS", ns)
        .env("SURR_DB_DB", db)
        .env("SURR_DB_USER", "root")
        .env("SURR_DB_PASS", "root")
        .env("OPENAI_API_KEY", FAKE_OPENAI_KEY)
        .env("SURR_EMBED_RETRIES", "1")
        .args(extra_args);
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd.output()
        .with_context(|| format!("failed to spawn reembed binary at {bin}"))
}

/// THE CONTRACT (`--dry-run` flag form): a dry run must not write anything,
/// and must say so on stdout.
#[tokio::test]
async fn reembed_bin_dry_run_flag_makes_no_writes() -> Result<()> {
    let Some(url) = test_db_url() else {
        return Ok(());
    };
    let (ns, db) = unique_ns_db("flag");
    let conn = connect(&url, &ns, &db).await?;
    seed(&conn).await?;
    let before = snapshot(&conn).await?;

    let out = run_binary(&url, &ns, &db, &["--dry-run"], &[])?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success(),
        "a --dry-run invocation must exit 0 (dry-run makes no provider call, so nothing about a \
         fake key should fail it); status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        out.status
    );
    assert!(
        stdout.contains("DRY RUN"),
        "stdout must carry a dry-run marker; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Would re-embed: 2 thoughts"),
        "dry run should report exactly the 2 candidate rows (missing + mismatched); stdout:\n{stdout}"
    );

    let after = snapshot(&conn).await?;
    assert_eq!(
        before, after,
        "--dry-run must not mutate any row of thoughts; stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    Ok(())
}

/// THE CONTRACT (`DRY_RUN` env-var form): same guarantees as the flag form,
/// via the other honored entry point (`dry_run_requested`,
/// src/bin/reembed.rs:15-20).
#[tokio::test]
async fn reembed_bin_dry_run_env_makes_no_writes() -> Result<()> {
    let Some(url) = test_db_url() else {
        return Ok(());
    };
    let (ns, db) = unique_ns_db("env");
    let conn = connect(&url, &ns, &db).await?;
    seed(&conn).await?;
    let before = snapshot(&conn).await?;

    let out = run_binary(&url, &ns, &db, &[], &[("DRY_RUN", "1")])?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success(),
        "a DRY_RUN=1 invocation must exit 0; status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        out.status
    );
    assert!(
        stdout.contains("DRY RUN"),
        "stdout must carry a dry-run marker; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Would re-embed: 2 thoughts"),
        "dry run should report exactly the 2 candidate rows (missing + mismatched); stdout:\n{stdout}"
    );

    let after = snapshot(&conn).await?;
    assert_eq!(
        before, after,
        "DRY_RUN=1 must not mutate any row of thoughts; stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    Ok(())
}

/// NEGATIVE CONTROL: the same fixture, same fake key, with dry-run NOT
/// requested. Without this half, the two tests above would pass equally
/// well against a binary that silently never calls the provider at all --
/// this proves the binary genuinely attempts a live embed when not told to
/// skip it. The snapshot is still expected to be unchanged after this run,
/// but for a DIFFERENT reason than the dry-run tests: not because the code
/// refused to try, but because the fake key makes the OpenAI API reject
/// every attempt (measured live: `curl` against
/// `https://api.openai.com/v1/models` with this key returns HTTP 401), so
/// no `UPDATE` is ever reached (src/bin/reembed.rs:222-225, the `Err(e)`
/// arm of `embedder.embed(...)`, increments `error_count` and `continue`s
/// -- it never runs the `UPDATE` query).
#[tokio::test]
async fn reembed_bin_live_run_with_fake_key_attempts_provider_and_writes_nothing() -> Result<()> {
    let Some(url) = test_db_url() else {
        return Ok(());
    };
    let (ns, db) = unique_ns_db("live");
    let conn = connect(&url, &ns, &db).await?;
    seed(&conn).await?;
    let before = snapshot(&conn).await?;

    let out = run_binary(&url, &ns, &db, &[], &[])?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // main() -> anyhow::Result<()> only ever returns Err for a failure in
    // the DB connect/query calls made with `?` -- a per-row embed failure
    // is caught inline (error_count += 1; continue) and never propagated,
    // so exit 0 here is expected and is NOT evidence the provider call
    // succeeded or was skipped. The real evidence is the stderr lines
    // below, which src/bin/reembed.rs only ever prints from inside the
    // `Err(e)` arm of an actual `embedder.embed(...).await` call.
    assert!(
        out.status.success(),
        "a live run's per-row embed failures are caught, not propagated, so exit should still be \
         0; status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        out.status
    );
    assert!(
        !stdout.contains("DRY RUN"),
        "a live run must not print the dry-run marker; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("Failed to embed content for"),
        "stderr must show the binary actually attempted (and failed) an embedder.embed(...) call \
         for a candidate row -- this is the proof of a genuine provider attempt, independent of \
         whether the failure looks like an HTTP 401 body or a network-layer send error; \
         stderr:\n{stderr}"
    );
    // Best-effort, not load-bearing: with real internet reachability
    // (verified live against api.openai.com during development of this
    // test -- HTTP 401 for this fake key), the underlying error also
    // carries OpenAI's own error text. Not asserted as a hard requirement
    // because a sandboxed CI runner without outbound internet would
    // instead see a "Failed to send embedding request" transport error,
    // which is equally valid evidence of an attempt.
    if stderr.contains("OpenAI API error") {
        eprintln!(
            "(informational) reached OpenAI and got a non-success HTTP status, as expected for a fake key"
        );
    }
    assert!(
        stdout.contains("❌ Errors: 2 thoughts"),
        "both candidate rows (missing + mismatched) should have failed to embed; stdout:\n{stdout}"
    );

    let after = snapshot(&conn).await?;
    assert_eq!(
        before, after,
        "every embed attempt failed against the fake key, so no UPDATE should ever have been \
         reached -- the snapshot staying identical here is a side effect of the provider \
         rejecting the fake key, NOT evidence dry-run logic fired (dry-run was not requested in \
         this test); stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    Ok(())
}
