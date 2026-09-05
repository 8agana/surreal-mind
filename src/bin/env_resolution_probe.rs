//! Test-only helper (fed-93bfee #216/#222/#225): reports env-resolution
//! RESULTS for whichever loader policy is selected via argv, never the
//! resolved values themselves. Never connects to a database, never calls a
//! real provider, never reads `surreal_mind.toml`. Spawned as a subprocess
//! -- with an isolated `current_dir` and an explicit, `env_clear()`-ed env
//! map -- by the dotenv-resolution tests in `tests/
//! config_env_resolution.rs`, so those tests can exercise the real loader
//! functions in `src/config.rs` without ever writing a file under this
//! checkout or mutating this crate's own test process's env
//! (`std::env::set_var`/`remove_var`).
//!
//! NOT A PRODUCTION TARGET: gated behind the non-default `test-probe`
//! Cargo feature (`required-features = ["test-probe"]` on its `[[bin]]`
//! entry in `Cargo.toml`) -- a plain `cargo build`/`cargo build --release`
//! never produces this binary at all (verified: `ls target/release | grep
//! env_resolution_probe` is empty after a default-features release build).
//! Codex's #225 review caught an earlier version of this file shipping as
//! an ordinary, always-built `[[bin]]` that printed `OPENAI_API_KEY` (and
//! `SURR_DB_URL`) IN FULL -- a credential-leak vector in a shipped binary,
//! even though nothing in this crate ever invoked it outside a sanitized
//! test. Both problems are fixed here: the feature gate closes the build
//! surface, and this file now prints ONLY presence and equality VERDICTS,
//! never a value, so even running it manually against a real `.env` cannot
//! leak a credential through its own stdout.
//!
//! Usage: env_resolution_probe <policy> [NAME=expected]...
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
//! Output (stdout), one line per var, NEVER a value:
//!   For a fixed set of names this helper always checks:
//!     NAME=PRESENT | NAME=ABSENT
//!   For each optional trailing `NAME=expected` argv pair (an equality
//!   check the CALLER supplies -- `expected` is always a synthetic test
//!   value chosen by the test itself, never read from a real environment):
//!     NAME_EQ=MATCHES_EXPECTED | NAME_EQ=DIFFERS | NAME_EQ=ABSENT

/// Fixed set of names this probe always reports presence for. Extend this
/// list, not ad-hoc calls, if a future test needs another var.
const PRESENCE_NAMES: &[&str] = &[
    "SURR_DB_URL",
    "OPENAI_API_KEY",
    "PROBE_MARKER",
    "PARENT_ONLY_MARKER",
    "ALLOW_NETWORK_EMBED",
];

fn print_presence(name: &str) {
    match std::env::var(name) {
        Ok(_) => println!("{name}=PRESENT"),
        Err(_) => println!("{name}=ABSENT"),
    }
}

/// Prints `<name>_EQ=MATCHES_EXPECTED|DIFFERS|ABSENT` -- never the actual
/// resolved value, only whether it equals the caller-supplied `expected`
/// (itself always a synthetic test value, never derived from a real
/// environment by this program).
fn print_equality(name: &str, expected: &str) {
    let verdict = match std::env::var(name) {
        Ok(actual) if actual == expected => "MATCHES_EXPECTED",
        Ok(_) => "DIFFERS",
        Err(_) => "ABSENT",
    };
    println!("{name}_EQ={verdict}");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let policy = args.next().unwrap_or_default();
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

    for name in PRESENCE_NAMES {
        print_presence(name);
    }

    for arg in args {
        if let Some((name, expected)) = arg.split_once('=') {
            print_equality(name, expected);
        } else {
            eprintln!(
                "env_resolution_probe: ignoring malformed equality arg {arg:?} (expected NAME=expected)"
            );
        }
    }
}
