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

/// Serialize build metadata without collapsing an uncertain observation into a
/// clean-looking identity. `dirty-unknown` means Git supplied a commit but its
/// dirty-state query failed; it is intentionally distinct from both clean and
/// dirty. `+unknown` is equally explicit when no commit was available.
fn identity_from_metadata(package_version: &str, commit: &str, dirty: &str) -> String {
    match (commit, dirty) {
        ("unknown", "0") => format!("{}+unknown", package_version),
        ("unknown", "1") => format!("{}+unknown-dirty", package_version),
        ("unknown", _) => format!("{}+unknown-dirty-unknown", package_version),
        (commit, "0") => format!("{}+{}", package_version, commit),
        (commit, "1") => format!("{}+{}-dirty", package_version, commit),
        (commit, _) => format!("{}+{}-dirty-unknown", package_version, commit),
    }
}

/// Build identity string. This is a build-time self-attestation, not a binary
/// hash or deployment proof; an external measured artifact receipt binds a
/// commit to a deployed SHA-256.
pub fn identity() -> String {
    identity_from_metadata(PKG_VERSION, GIT_COMMIT, GIT_DIRTY)
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

    #[test]
    fn identity_serializes_clean_dirty_and_dirty_unknown_as_distinct_states() {
        let clean = identity_from_metadata("1.2.3", "abcdef0", "0");
        let dirty = identity_from_metadata("1.2.3", "abcdef0", "1");
        let dirty_unknown = identity_from_metadata("1.2.3", "abcdef0", "unknown");

        assert_eq!(clean, "1.2.3+abcdef0");
        assert_eq!(dirty, "1.2.3+abcdef0-dirty");
        assert_eq!(dirty_unknown, "1.2.3+abcdef0-dirty-unknown");
        assert_ne!(clean, dirty);
        assert_ne!(clean, dirty_unknown);
        assert_ne!(dirty, dirty_unknown);
    }

    #[test]
    fn identity_makes_unknown_commit_explicit() {
        assert_eq!(
            identity_from_metadata("1.2.3", "unknown", "unknown"),
            "1.2.3+unknown-dirty-unknown"
        );
        assert_ne!(identity_from_metadata("1.2.3", "unknown", "0"), "1.2.3");
    }
}
