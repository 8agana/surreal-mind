//! fed-93bfee #216/#222/#225: dotenv-resolution controls for
//! `src/config.rs`'s `load_env_file_or` / `load_env_file` /
//! `load_env_file_config_load_unset_rule`.
//!
//! Gated behind the `test-probe` feature (matching the `env_resolution_probe`
//! binary's own `required-features` gate in `Cargo.toml`) -- under default
//! features, `env!("CARGO_BIN_EXE_env_resolution_probe")` would not resolve
//! at all, since that binary is never built without `--features
//! test-probe`. Run with `cargo test --features test-probe` (the wrapper
//! passes both `db_integration` and `test-probe`).
//!
//! Every test builds its own private `mktemp`-style temp tree (never a path
//! under this checkout), spawns the compiled `env_resolution_probe` binary
//! with `Command::env_clear()` plus only the entries the test explicitly
//! wants, and `current_dir` pointed at a directory inside that private
//! tree -- exercising the REAL loader functions in an isolated child
//! process. No file is ever written under this checkout; no
//! `std::env::set_var`/`remove_var` runs in this test process; no Mutex is
//! needed. The probe itself never prints a resolved value, only
//! PRESENT/ABSENT/MATCHES_EXPECTED/DIFFERS verdicts (fed-93bfee #225 --
//! see `src/bin/env_resolution_probe.rs`'s own doc comment for why).
#![cfg(feature = "test-probe")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn probe_bin() -> &'static str {
    env!("CARGO_BIN_EXE_env_resolution_probe")
}

/// Spawn the probe with a fully-controlled child environment (`env_clear()`
/// then only `env`'s entries), an isolated `current_dir`, and optional
/// trailing `NAME=expected` equality-check args. Returns the probe's
/// `NAME=PRESENT|ABSENT` / `NAME_EQ=MATCHES_EXPECTED|DIFFERS|ABSENT` stdout
/// lines as a lookup map. Never sees or returns an actual resolved value.
fn run_probe(
    policy: &str,
    cwd: &Path,
    env: &[(&str, &str)],
    equality_checks: &[(&str, &str)],
) -> HashMap<String, String> {
    let mut cmd = Command::new(probe_bin());
    cmd.arg(policy).current_dir(cwd).env_clear();
    for (k, v) in env {
        cmd.env(k, v);
    }
    for (name, expected) in equality_checks {
        cmd.arg(format!("{name}={expected}"));
    }
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn env_resolution_probe: {e}"));
    assert!(
        output.status.success(),
        "probe exited non-zero: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn write(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap_or_else(|e| panic!("write {path:?}: {e}"));
}

/// A fresh, private temp directory this test owns exclusively -- never a
/// path under this checkout.
fn temp_tree(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "config_env_resolution_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create private temp tree");
    dir
}

/// A path that is guaranteed not to exist: inside a private temp dir this
/// test owns, with a filename that is never created.
fn nonexistent_path(dir: &Path) -> PathBuf {
    dir.join(format!(
        "does-not-exist-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_nanos()
    ))
}

// ===== Config::load's own pre-#216 unset-rule, pinned exactly (#222 item 1) =====
// local `.env`, then `../.env` ONLY if neither SURR_DB_URL nor
// OPENAI_API_KEY ended up present -- see git show 73d5831:src/config.rs.
// Every case below builds its own grandparent/parent/local tree so "does it
// search past the parent" is directly, physically provable.

#[test]
fn config_load_rule_local_incomplete_falls_back_to_parent() {
    let gp = temp_tree("case1_gp");
    let parent = gp.join("parent");
    let local = parent.join("local");
    std::fs::create_dir_all(&local).unwrap();
    write(&local.join(".env"), "PROBE_MARKER=from_local\n");
    write(&parent.join(".env"), "SURR_DB_URL=probe://parent\n");

    let result = run_probe("config-load", &local, &[], &[]);
    let _ = std::fs::remove_dir_all(&gp);

    assert_eq!(
        result.get("PROBE_MARKER").map(String::as_str),
        Some("PRESENT"),
        "local .env must still be loaded"
    );
    assert_eq!(
        result.get("SURR_DB_URL").map(String::as_str),
        Some("PRESENT"),
        "local lacked both core vars, so ../.env must be consulted"
    );
}

#[test]
fn config_load_rule_local_complete_never_consults_parent() {
    let gp = temp_tree("case2_gp");
    let parent = gp.join("parent");
    let local = parent.join("local");
    std::fs::create_dir_all(&local).unwrap();
    write(&local.join(".env"), "SURR_DB_URL=probe://local\n");
    write(&parent.join(".env"), "PARENT_ONLY_MARKER=1\n");

    let result = run_probe("config-load", &local, &[], &[]);
    let _ = std::fs::remove_dir_all(&gp);

    assert_eq!(
        result.get("SURR_DB_URL").map(String::as_str),
        Some("PRESENT")
    );
    assert_eq!(
        result.get("PARENT_ONLY_MARKER").map(String::as_str),
        Some("ABSENT"),
        "local already set a core var, so ../.env must never be consulted at all"
    );
}

#[test]
fn config_load_rule_no_local_no_parent_never_searches_grandparent() {
    let gp = temp_tree("case3_gp");
    let parent = gp.join("parent");
    let local = parent.join("local");
    std::fs::create_dir_all(&local).unwrap();
    // No local .env, no parent .env -- only a grandparent one level further up.
    write(&gp.join(".env"), "SURR_DB_URL=probe://grandparent\n");

    let result = run_probe("config-load", &local, &[], &[]);
    let _ = std::fs::remove_dir_all(&gp);

    assert_eq!(
        result.get("SURR_DB_URL").map(String::as_str),
        Some("ABSENT"),
        "the old rule only ever looks at local + the immediate parent, never further up"
    );
}

#[test]
fn config_load_rule_explicit_empty_pin_ignores_the_whole_tree() {
    let gp = temp_tree("case4_gp");
    let parent = gp.join("parent");
    let local = parent.join("local");
    std::fs::create_dir_all(&local).unwrap();
    write(&local.join(".env"), "SURR_DB_URL=probe://local\n");
    write(&parent.join(".env"), "SURR_DB_URL=probe://parent\n");
    write(&gp.join(".env"), "SURR_DB_URL=probe://grandparent\n");
    let pinned = gp.join("pinned_empty.env");
    write(&pinned, "");

    let result = run_probe(
        "config-load",
        &local,
        &[("SURR_ENV_FILE", pinned.to_str().unwrap())],
        &[],
    );
    let _ = std::fs::remove_dir_all(&gp);

    assert_eq!(
        result.get("SURR_DB_URL").map(String::as_str),
        Some("ABSENT"),
        "SURR_ENV_FILE pinned to an empty file must ignore local/parent/grandparent entirely, no fallback"
    );
}

#[test]
fn config_load_rule_missing_explicit_pin_with_nothing_on_disk_resolves_nothing() {
    let gp = temp_tree("case5_gp");
    let parent = gp.join("parent");
    let local = parent.join("local");
    std::fs::create_dir_all(&local).unwrap();
    // No files anywhere in the tree, and SURR_ENV_FILE genuinely unset --
    // proves the dispatch correctly falls through to the unset_fallback
    // closure rather than doing anything surprising. Distinct from the
    // "explicit nonexistent pin" case below: here SURR_ENV_FILE is never
    // set at all, not set-to-a-path-that-happens-not-to-exist.

    let result = run_probe("config-load", &local, &[], &[]);
    let _ = std::fs::remove_dir_all(&gp);

    assert_eq!(
        result.get("SURR_DB_URL").map(String::as_str),
        Some("ABSENT")
    );
    assert_eq!(
        result.get("PROBE_MARKER").map(String::as_str),
        Some("ABSENT")
    );
}

/// #225: the promised "explicit nonexistent pin" control was missing --
/// the test above supplies no SURR_ENV_FILE at all (genuinely unset), which
/// tests the UNSET path, not an explicit pin to a path that doesn't exist.
/// This one sets SURR_ENV_FILE to a path this test guarantees was never
/// created, with real decoy `.env` files sitting in both local and parent,
/// and asserts NEITHER decoy is ever read for either loader policy: once
/// SURR_ENV_FILE is set (to anything), `dotenvy::from_path` on the
/// nonexistent path fails silently (swallowed, matching every call site)
/// and there is still NO fallback to local/parent -- the explicit-pin
/// branch never even considers them.
#[test]
fn config_load_rule_explicit_nonexistent_pin_ignores_local_and_parent_decoys() {
    let gp = temp_tree("nonexistent_pin_config_load");
    let parent = gp.join("parent");
    let local = parent.join("local");
    std::fs::create_dir_all(&local).unwrap();
    write(&local.join(".env"), "PROBE_MARKER=should_never_appear\n");
    write(
        &parent.join(".env"),
        "SURR_DB_URL=probe://should-never-appear\n",
    );
    let missing = nonexistent_path(&gp);
    assert!(
        !missing.exists(),
        "test hazard: the 'nonexistent' path actually exists"
    );

    let result = run_probe(
        "config-load",
        &local,
        &[("SURR_ENV_FILE", missing.to_str().unwrap())],
        &[],
    );
    let _ = std::fs::remove_dir_all(&gp);

    assert_eq!(
        result.get("PROBE_MARKER").map(String::as_str),
        Some("ABSENT"),
        "local decoy must not be read once SURR_ENV_FILE is pinned, even to a nonexistent path"
    );
    assert_eq!(
        result.get("SURR_DB_URL").map(String::as_str),
        Some("ABSENT"),
        "parent decoy must not be read either -- no fallback once pinned"
    );
}

/// Same as above, for the OTHER loader policy (`bare-dotenv`, used by
/// embeddings.rs/lib.rs/bin/reembed.rs/the gemini test) -- the explicit-pin
/// branch is the SAME shared code (`load_env_file_or`) regardless of which
/// unset_fallback closure a caller supplies, so both policies must agree.
#[test]
fn bare_dotenv_explicit_nonexistent_pin_ignores_local_decoy() {
    let dir = temp_tree("nonexistent_pin_bare_dotenv");
    write(&dir.join(".env"), "PROBE_MARKER=should_never_appear\n");
    let missing = nonexistent_path(&dir);
    assert!(
        !missing.exists(),
        "test hazard: the 'nonexistent' path actually exists"
    );

    let result = run_probe(
        "bare-dotenv",
        &dir,
        &[("SURR_ENV_FILE", missing.to_str().unwrap())],
        &[],
    );
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        result.get("PROBE_MARKER").map(String::as_str),
        Some("ABSENT"),
        "local decoy must not be read once SURR_ENV_FILE is pinned, even to a nonexistent path"
    );
}

/// #225: already-present core env (supplied directly in the CHILD's own
/// environment, not via a local `.env`) must suppress the parent lookup
/// exactly like a local `.env` setting the core vars does -- both are just
/// "SURR_DB_URL/OPENAI_API_KEY already present by the time the core_present
/// check runs". Local `.env` here sets an unrelated marker but deliberately
/// lacks both core vars, so the ONLY reason parent should be skipped is the
/// pre-set child env.
#[test]
fn config_load_rule_core_env_already_present_in_child_env_suppresses_parent() {
    let gp = temp_tree("core_env_present");
    let parent = gp.join("parent");
    let local = parent.join("local");
    std::fs::create_dir_all(&local).unwrap();
    write(&local.join(".env"), "PROBE_MARKER=from_local\n");
    write(&parent.join(".env"), "PARENT_ONLY_MARKER=1\n");

    let result = run_probe(
        "config-load",
        &local,
        &[
            ("SURR_DB_URL", "probe://already-present"),
            ("OPENAI_API_KEY", "sk-already-present"),
        ],
        &[
            ("SURR_DB_URL", "probe://already-present"),
            ("OPENAI_API_KEY", "sk-already-present"),
        ],
    );
    let _ = std::fs::remove_dir_all(&gp);

    assert_eq!(
        result.get("PROBE_MARKER").map(String::as_str),
        Some("PRESENT"),
        "local .env must still be loaded regardless of the pre-set core vars"
    );
    assert_eq!(
        result.get("PARENT_ONLY_MARKER").map(String::as_str),
        Some("ABSENT"),
        "core vars were already present in the child's own env, so ../.env must never be consulted"
    );
    assert_eq!(
        result.get("SURR_DB_URL_EQ").map(String::as_str),
        Some("MATCHES_EXPECTED"),
        "the pre-set core var must survive unchanged (dotenvy never overrides an already-set var)"
    );
    assert_eq!(
        result.get("OPENAI_API_KEY_EQ").map(String::as_str),
        Some("MATCHES_EXPECTED")
    );
}

/// The inverse of the above: no core vars anywhere (not in the child's own
/// env, and no local `.env` at all -- not even one lacking them), only a
/// parent `.env` with a marker. Parent MUST be consulted and its marker
/// must appear. (Distinct from `config_load_rule_local_incomplete_falls_
/// back_to_parent` above, which has a local `.env` present but incomplete;
/// this one has no local `.env` on disk at all.)
#[test]
fn config_load_rule_no_core_env_and_no_local_falls_back_to_parent_marker() {
    let gp = temp_tree("no_core_env_no_local");
    let parent = gp.join("parent");
    let local = parent.join("local");
    std::fs::create_dir_all(&local).unwrap();
    // No local .env at all.
    write(&parent.join(".env"), "PARENT_ONLY_MARKER=1\n");

    let result = run_probe("config-load", &local, &[], &[]);
    let _ = std::fs::remove_dir_all(&gp);

    assert_eq!(
        result.get("PARENT_ONLY_MARKER").map(String::as_str),
        Some("PRESENT"),
        "no core vars anywhere and no local .env at all -- ../.env must be consulted"
    );
}

// ===== Controls (a) and (b): the shared explicit-pin rule, and the plain
// dotenvy::dotenv() rule every OTHER reachable caller uses when unset =====

#[test]
fn control_b_bare_dotenv_unset_still_discovers_local_env() {
    let dir = temp_tree("control_b");
    write(&dir.join(".env"), "PROBE_MARKER=default_discovery_value\n");

    let result = run_probe("bare-dotenv", &dir, &[], &[]);
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        result.get("PROBE_MARKER").map(String::as_str),
        Some("PRESENT"),
        "with SURR_ENV_FILE unset, load_env_file() must still discover an ordinary local .env \
         (embeddings::create_embedder / lib::load_env / gemini_client_integration.rs / \
         bin/reembed.rs's unchanged unset behavior)"
    );
}

#[test]
fn control_a_shared_pin_ignores_decoy_and_keeps_already_set_key() {
    let dir = temp_tree("control_a");
    write(
        &dir.join(".env"),
        "PROBE_MARKER=should_never_appear\nOPENAI_API_KEY=sk-decoy-should-never-be-used\nALLOW_NETWORK_EMBED=1\n",
    );
    let pinned = dir.join("pinned_empty.env");
    write(&pinned, "");

    let result = run_probe(
        "bare-dotenv",
        &dir,
        &[
            ("SURR_ENV_FILE", pinned.to_str().unwrap()),
            // Mirrors scripts/test_db.sh always exporting a fake key itself
            // BEFORE cargo test runs -- proves dotenvy's own
            // never-override-an-already-set-var rule holds even though the
            // decoy tries to set a different value, entirely independent of
            // the SURR_ENV_FILE pin (which alone already stops the decoy
            // from being read at all).
            ("OPENAI_API_KEY", "sk-fake-testdb"),
        ],
        &[("OPENAI_API_KEY", "sk-fake-testdb")],
    );
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        result.get("PROBE_MARKER").map(String::as_str),
        Some("ABSENT"),
        "decoy .env must never be read when SURR_ENV_FILE is pinned to an empty file"
    );
    assert_eq!(
        result.get("OPENAI_API_KEY_EQ").map(String::as_str),
        Some("MATCHES_EXPECTED"),
        "OPENAI_API_KEY must remain the value already set in the child's env, never the decoy's"
    );
    assert_eq!(
        result.get("ALLOW_NETWORK_EMBED").map(String::as_str),
        Some("ABSENT"),
        "ALLOW_NETWORK_EMBED must not be reintroduced from the decoy .env"
    );
}
