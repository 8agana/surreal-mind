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
//! Rerun-if-changed: narrowly watches git ref state (`.git/HEAD`, the
//! resolved branch ref file e.g. `.git/refs/heads/<branch>`, and
//! `.git/packed-refs`), added only when a commit was actually found.
//! **Correctness note, measured live during implementation**: watching
//! `.git/HEAD` ALONE is not sufficient — `HEAD` is a symbolic ref
//! (`ref: refs/heads/<branch>`) whose file contents don't change on an
//! ordinary commit; only the branch's own ref file does. An earlier version
//! of this build.rs watched only `.git/HEAD` and, measured live, produced a
//! STALE embedded commit hash after committing new work with no full
//! rebuild in between (cargo correctly saw no rerun-if-changed trigger and
//! skipped build.rs entirely) — not just a stale dirty flag, but an
//! actively wrong commit hash. Watching the resolved branch ref file (and
//! packed-refs, in case the ref is packed rather than loose) closes that
//! gap. What remains a deliberate, documented limitation (not a bug): the
//! DIRTY flag still reflects working-tree state as of the most recent
//! build.rs invocation, not the exact instant `--version` runs, if files are
//! edited without triggering *any* rerun-if-changed path in between. The
//! broader alternative — `cargo:rerun-if-changed` over the whole working
//! tree, so every file edit forces a rebuild — is deliberately not taken
//! here: it would make every `cargo build`/`cargo check` re-run this script
//! on any source edit, which is a real, measurable build-latency cost for a
//! provenance flag that is already `git status`-observable by any caller who
//! needs it precisely. No separate design document exists for this
//! trade-off; it is recorded here in full, in this comment, as the only
//! copy.

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
            if let Some(head_path) = git_output(&manifest_dir, &["rev-parse", "--git-path", "HEAD"])
            {
                println!("cargo:rerun-if-changed={}", head_path);
            }
            // HEAD's own file content only changes on `checkout`/detached-HEAD
            // moves, not on an ordinary commit — watch the resolved branch ref
            // file too, so a new commit on the current branch actually
            // triggers a rerun (see the module-level doc comment above).
            if let Some(symbolic_ref) = git_output(&manifest_dir, &["symbolic-ref", "-q", "HEAD"])
                && let Some(ref_path) =
                    git_output(&manifest_dir, &["rev-parse", "--git-path", &symbolic_ref])
            {
                println!("cargo:rerun-if-changed={}", ref_path);
            }
            // Also watch packed-refs, in case the branch ref has been packed
            // (e.g. by `git gc`) rather than left as a loose ref file.
            if let Some(packed_refs) =
                git_output(&manifest_dir, &["rev-parse", "--git-path", "packed-refs"])
            {
                println!("cargo:rerun-if-changed={}", packed_refs);
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
