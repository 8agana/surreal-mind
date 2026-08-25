//! Build-time provenance identity: package version + git commit + dirty flag.
//!
//! Values are embedded by `build.rs` via `cargo:rustc-env`. `build.rs` always
//! emits both env vars (falling back to the literal string `unknown` on any
//! failure), so `env!()` here can never fail to resolve at compile time.

/// Crate version from Cargo.toml, e.g. `"0.8.2"`.
pub const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Short (7-char) git commit hash as of the last full build, or the literal
/// string `"unknown"` if git metadata was unavailable at build time.
const GIT_COMMIT: &str = env!("SURR_GIT_COMMIT");

/// `"1"` if the tree had uncommitted tracked changes at build time, `"0"` if
/// clean, or `"unknown"` if git metadata was unavailable at build time.
const GIT_DIRTY: &str = env!("SURR_GIT_DIRTY");

/// Self-attesting version identity string.
///
/// Shape mirrors `comm`'s proven `0.1.0+0bc53d0` format, with an added
/// `-dirty` suffix:
///   - `"{PKG_VERSION}+{commit}-dirty"` when the commit is known and the tree
///     was dirty at build time.
///   - `"{PKG_VERSION}+{commit}"` when the commit is known and the tree was
///     clean (or dirty-state itself could not be determined).
///   - bare `PKG_VERSION` when no commit could be determined at all (e.g. a
///     source-archive build with no `.git` directory).
pub fn identity() -> String {
    if GIT_COMMIT == "unknown" {
        return PKG_VERSION.to_string();
    }
    if GIT_DIRTY == "1" {
        format!("{}+{}-dirty", PKG_VERSION, GIT_COMMIT)
    } else {
        format!("{}+{}", PKG_VERSION, GIT_COMMIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_always_starts_with_pkg_version() {
        // Whatever the build-time git state was, the identity string must
        // begin with the plain package version so callers can always at
        // least parse the semver prefix.
        assert!(identity().starts_with(PKG_VERSION));
    }

    #[test]
    fn identity_has_no_trailing_or_leading_whitespace() {
        let id = identity();
        assert_eq!(id.trim(), id);
    }
}
