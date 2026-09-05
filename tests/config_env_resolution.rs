//! fed-93bfee #216/#222: dotenv-resolution controls for `src/config.rs`'s
//! `load_env_file_or` / `load_env_file` / `load_env_file_config_load_unset_rule`.
//!
//! Rebuilt as private-temp SUBPROCESS controls per Codex's #222 review: an
//! earlier version of these tests wrote a decoy `.env` directly under
//! `CARGO_MANIFEST_DIR` and called `std::env::set_var`/`remove_var` on the
//! test process itself, requiring a Mutex to avoid racing other tests.
//! Every test here instead builds its own private `mktemp`-style temp tree,
//! spawns the compiled `env_resolution_probe` binary (`src/bin/
//! env_resolution_probe.rs`) with `Command::env_clear()` plus only the
//! entries the test explicitly wants, and `current_dir` pointed at a
//! directory inside that private tree -- exercising the REAL loader
//! functions in an isolated child process. No file is ever written under
//! this checkout; no `std::env::set_var`/`remove_var` runs in this test
//! process; no Mutex is needed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn probe_bin() -> &'static str {
    env!("CARGO_BIN_EXE_env_resolution_probe")
}

/// Spawn the probe with a fully-controlled child environment (`env_clear()`
/// then only `env`'s entries) and an isolated `current_dir`. Returns the
/// probe's `NAME=value` stdout lines as a lookup map.
fn run_probe(policy: &str, cwd: &Path, env: &[(&str, &str)]) -> HashMap<String, String> {
    let mut cmd = Command::new(probe_bin());
    cmd.arg(policy).current_dir(cwd).env_clear();
    for (k, v) in env {
        cmd.env(k, v);
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

// ===== Config::load's own pre-#216 unset-rule, pinned exactly (item 1) =====
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

    let result = run_probe("config-load", &local, &[]);
    let _ = std::fs::remove_dir_all(&gp);

    assert_eq!(
        result.get("PROBE_MARKER").map(String::as_str),
        Some("from_local"),
        "local .env must still be loaded"
    );
    assert_eq!(
        result.get("SURR_DB_URL").map(String::as_str),
        Some("probe://parent"),
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

    let result = run_probe("config-load", &local, &[]);
    let _ = std::fs::remove_dir_all(&gp);

    assert_eq!(
        result.get("SURR_DB_URL").map(String::as_str),
        Some("probe://local")
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

    let result = run_probe("config-load", &local, &[]);
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
    // closure rather than doing anything surprising.

    let result = run_probe("config-load", &local, &[]);
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

// ===== Controls (a) and (b): the shared explicit-pin rule, and the plain
// dotenvy::dotenv() rule every OTHER reachable caller uses when unset =====

#[test]
fn control_b_bare_dotenv_unset_still_discovers_local_env() {
    let dir = temp_tree("control_b");
    write(&dir.join(".env"), "PROBE_MARKER=default_discovery_value\n");

    let result = run_probe("bare-dotenv", &dir, &[]);
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        result.get("PROBE_MARKER").map(String::as_str),
        Some("default_discovery_value"),
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
    );
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        result.get("PROBE_MARKER").map(String::as_str),
        Some("ABSENT"),
        "decoy .env must never be read when SURR_ENV_FILE is pinned to an empty file"
    );
    assert_eq!(
        result.get("OPENAI_API_KEY").map(String::as_str),
        Some("sk-fake-testdb"),
        "OPENAI_API_KEY must remain the value already set in the child's env, never the decoy's"
    );
    assert_eq!(
        result.get("ALLOW_NETWORK_EMBED").map(String::as_str),
        Some("ABSENT"),
        "ALLOW_NETWORK_EMBED must not be reintroduced from the decoy .env"
    );
}
