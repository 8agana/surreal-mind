//! Build-time provenance for `surreal-mind --version`.
//!
//! The embedded identity is deliberately an observation of this build, not a
//! binary hash or a deployment receipt. A measured artifact SHA-256 and an
//! external deployment receipt are what bind a specific commit to a deployed
//! binary. See `scripts/verify-build-provenance-fixture.sh` for the bounded
//! executable witness of the state transitions below.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git_output(manifest_dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(manifest_dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let value = stdout.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Git commands invoked from a source archive nested in another repository
/// resolve to that ancestor repository. That ancestor is not this crate's
/// source identity, so metadata is accepted only when Git's top-level path is
/// exactly this package's canonical manifest directory.
fn canonical_git_root(manifest_dir: &Path) -> Option<PathBuf> {
    let top_level = git_output(manifest_dir, &["rev-parse", "--show-toplevel"])?;
    let manifest_dir = std::fs::canonicalize(manifest_dir).ok()?;
    let top_level = std::fs::canonicalize(top_level).ok()?;
    (top_level == manifest_dir).then_some(top_level)
}

fn emit_rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
}

/// Source-archive fallback for when no canonical Git checkout exists. It keeps
/// provenance reactive to the same relevant Rust/source inputs without trying
/// to infer an identity from an unrelated ancestor repository.
fn emit_source_tree_reruns(path: &Path) {
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            emit_source_tree_reruns(&path);
        } else if path.is_file() {
            emit_rerun_if_changed(&path);
        }
    }
}

/// Cargo switches from its default broad build-script invalidation to only the
/// explicit paths printed here. Therefore Git ref watches alone are
/// insufficient: a tracked `src/**/*.rs` change can make a tree dirty while
/// leaving HEAD unchanged. Emit every relevant tracked compile/source input so
/// a normal source edit re-runs this script and refreshes the dirty state.
fn emit_tracked_input_reruns(manifest_dir: &Path) {
    let Some(files) = git_output(
        manifest_dir,
        &[
            "ls-files",
            "--cached",
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "build.rs",
            "src",
        ],
    ) else {
        for name in ["Cargo.toml", "Cargo.lock", "build.rs"] {
            emit_rerun_if_changed(&manifest_dir.join(name));
        }
        emit_source_tree_reruns(&manifest_dir.join("src"));
        return;
    };

    for file in files.lines() {
        emit_rerun_if_changed(&manifest_dir.join(file));
    }
}

fn emit_git_ref_reruns(manifest_dir: &Path) {
    if let Some(head_path) = git_output(manifest_dir, &["rev-parse", "--git-path", "HEAD"]) {
        emit_rerun_if_changed(Path::new(&head_path));
    }
    if let Some(symbolic_ref) = git_output(manifest_dir, &["symbolic-ref", "-q", "HEAD"])
        && let Some(ref_path) = git_output(
            manifest_dir,
            &["rev-parse", "--git-path", symbolic_ref.as_str()],
        )
    {
        emit_rerun_if_changed(Path::new(&ref_path));
    }
    if let Some(packed_refs) = git_output(manifest_dir, &["rev-parse", "--git-path", "packed-refs"])
    {
        emit_rerun_if_changed(Path::new(&packed_refs));
    }
}

fn main() {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let canonical_checkout = canonical_git_root(&manifest_dir).is_some();
    emit_tracked_input_reruns(&manifest_dir);

    if !canonical_checkout {
        println!("cargo:rustc-env=SURR_GIT_COMMIT=unknown");
        println!("cargo:rustc-env=SURR_GIT_DIRTY=unknown");
        println!(
            "cargo:warning=surreal-mind: no Git checkout rooted exactly at CARGO_MANIFEST_DIR; --version will report explicit unknown provenance"
        );
        return;
    }

    // Commit and dirty state are independent observations. A status failure
    // must not turn a valid commit into an apparently clean identity.
    let commit = git_output(&manifest_dir, &["rev-parse", "--short=7", "HEAD"]);
    let dirty = git_output(
        &manifest_dir,
        &["status", "--porcelain", "--untracked-files=no"],
    )
    .map(|status| !status.is_empty());

    match commit {
        Some(commit) => {
            println!("cargo:rustc-env=SURR_GIT_COMMIT={commit}");
            emit_git_ref_reruns(&manifest_dir);
        }
        None => {
            println!("cargo:rustc-env=SURR_GIT_COMMIT=unknown");
            println!(
                "cargo:warning=surreal-mind: canonical Git checkout found but HEAD was unreadable; --version will report explicit unknown provenance"
            );
        }
    }

    match dirty {
        Some(true) => println!("cargo:rustc-env=SURR_GIT_DIRTY=1"),
        Some(false) => println!("cargo:rustc-env=SURR_GIT_DIRTY=0"),
        None => println!("cargo:rustc-env=SURR_GIT_DIRTY=unknown"),
    }
}
