//! Build-time provenance embedding.
//!
//! Embeds the git short commit hash and a dirty-tree flag into the binary via
//! `cargo:rustc-env`, consumed by `src/version.rs`. Mirrors the shape of
//! `comm`'s (federation-cli) provenance build.rs, with two deliberate
//! differences documented inline below:
//!
//!   1. No `.expect()`/`panic!` anywhere. A build with no `git` on PATH, or run
//!      from a source archive with no `.git` directory (e.g. `cargo package`,
//!      a minimal container `COPY src/` stage), must still SUCCEED. On any
//!      failure this falls back to `unknown` and emits a single
//!      `cargo:warning` explaining why, rather than failing the build or
//!      fabricating a value.
//!   2. Dirty-tree detection, which `comm`'s build.rs does not do at all.
//!
//! Rerun-if-changed: narrowly watches `.git/HEAD` and the resolved ref path
//! (added only when a commit was actually found), matching `comm`'s pattern.
//! This means the embedded dirty flag reflects git state as of the most
//! recent FULL/FORCED build, not necessarily the instant `--version` runs —
//! see the design doc
//! (docs/tasks/20260823-rmcp-3.1.4-upgrade/rmcp-3.1.4-version-provenance-design.md)
//! for the alternative (broad rerun-if-changed over the whole tree) and why
//! this is the deliberately chosen default, not an oversight.

use std::process::Command;

fn git_output(manifest_dir: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(manifest_dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());

    // Commit hash and dirty-tree state are independent lookups: a `git status`
    // failure after a successful `rev-parse` must not discard a valid commit
    // hash by falling through a coupled match arm.
    let commit = git_output(&manifest_dir, &["rev-parse", "--short=7", "HEAD"]);

    let dirty = git_output(
        &manifest_dir,
        &["status", "--porcelain", "--untracked-files=no"],
    )
    .map(|s| !s.is_empty());

    match &commit {
        Some(sha) => {
            println!("cargo:rustc-env=SURR_GIT_COMMIT={}", sha);
            // Narrow rerun-if-changed: only watch git ref state, only once we
            // know we're in a git checkout at all.
            if let Some(git_dir) = git_output(&manifest_dir, &["rev-parse", "--git-path", "HEAD"]) {
                println!("cargo:rerun-if-changed={}", git_dir);
            }
        }
        None => {
            println!("cargo:rustc-env=SURR_GIT_COMMIT=unknown");
            println!(
                "cargo:warning=surreal-mind: could not determine git commit (git missing from PATH, or no .git directory present — e.g. a source-archive build); --version will report a bare package version with no commit suffix"
            );
        }
    }

    match dirty {
        Some(true) => println!("cargo:rustc-env=SURR_GIT_DIRTY=1"),
        Some(false) => println!("cargo:rustc-env=SURR_GIT_DIRTY=0"),
        None => println!("cargo:rustc-env=SURR_GIT_DIRTY=unknown"),
    }
}
