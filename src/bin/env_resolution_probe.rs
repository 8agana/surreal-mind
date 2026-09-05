//! Test-only helper (fed-93bfee #222): prints the env-resolution result for
//! whichever loader policy is selected via argv, then exits immediately.
//! Never connects to a database, never calls a real provider, never reads
//! `surreal_mind.toml`. Spawned as a subprocess -- with an isolated
//! `current_dir` and an explicit, `env_clear()`-ed env map -- by the
//! dotenv-resolution tests in `tests/config_env_resolution.rs`, so those
//! tests can exercise the real loader functions in `src/config.rs` without
//! ever writing a file under this checkout or mutating this crate's own
//! test process's env (`std::env::set_var`/`remove_var`).
//!
//! Usage: env_resolution_probe <policy>
//!   config-load   -- exercise `load_env_file_config_load_unset_rule`, the
//!                     exact rule `Config::load` used for its unset-path
//!                     before fed-93bfee #216 (local `.env`, then
//!                     `../.env` ONLY if neither `SURR_DB_URL` nor
//!                     `OPENAI_API_KEY` ended up present), UNLESS
//!                     `SURR_ENV_FILE` is set, in which case only that
//!                     path is loaded (via `load_env_file_or`).
//!   bare-dotenv   -- exercise `load_env_file`, the plain `dotenvy::dotenv()`
//!                     fallback every OTHER reachable call site
//!                     (`embeddings::create_embedder`, `lib::load_env`,
//!                     `tests/gemini_client_integration.rs`,
//!                     `src/bin/reembed.rs`) uses when `SURR_ENV_FILE` is
//!                     unset, unified with the same explicit-pin handling.
//!
//! Prints one `NAME=value` line per env var the tests care about (or
//! `NAME=ABSENT` if unset) to stdout, in a fixed order, so tests can parse
//! it without any ambiguity:
//!   SURR_DB_URL=<value or ABSENT>
//!   OPENAI_API_KEY=<value or ABSENT>
//!   PROBE_MARKER=<value or ABSENT>
//!   PARENT_ONLY_MARKER=<value or ABSENT>
//!   ALLOW_NETWORK_EMBED=<value or ABSENT>

fn print_var(name: &str) {
    match std::env::var(name) {
        Ok(v) => println!("{name}={v}"),
        Err(_) => println!("{name}=ABSENT"),
    }
}

fn main() {
    let policy = std::env::args().nth(1).unwrap_or_default();
    match policy.as_str() {
        "config-load" => {
            surreal_mind::config::load_env_file_or(
                surreal_mind::config::load_env_file_config_load_unset_rule,
            );
        }
        "bare-dotenv" => {
            surreal_mind::config::load_env_file();
        }
        other => {
            eprintln!(
                "env_resolution_probe: unknown policy {other:?} (expected config-load or bare-dotenv)"
            );
            std::process::exit(2);
        }
    }
    print_var("SURR_DB_URL");
    print_var("OPENAI_API_KEY");
    print_var("PROBE_MARKER");
    print_var("PARENT_ONLY_MARKER");
    print_var("ALLOW_NETWORK_EMBED");
}
