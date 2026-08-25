#!/usr/bin/env bash
# Bounded executable proof for build.rs provenance behavior. It operates only
# in a temporary directory, never touches the invoking checkout, and removes
# all fixtures on exit.
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
package_version=$(awk -F '"' '/^version = / { print $2; exit }' "$repo_root/Cargo.toml")
fixture_root=$(mktemp -d "${TMPDIR:-/tmp}/surreal-mind-provenance.XXXXXX")
trap 'rm -rf "$fixture_root"' EXIT
shared_target="$fixture_root/target"

build_and_expect() {
    local tree=$1
    local target_dir=$2
    local expected=$3
    CARGO_TARGET_DIR="$target_dir" cargo build --quiet --locked --manifest-path "$tree/Cargo.toml" --bin surreal-mind
    local actual
    actual=$("$target_dir/debug/surreal-mind" --version)
    if [[ "$actual" != "surreal-mind $expected" ]]; then
        printf 'expected %q, got %q\n' "surreal-mind $expected" "$actual" >&2
        return 1
    fi
}

make_git_fixture() {
    local destination=$1
    mkdir -p "$destination"
    git -C "$repo_root" archive --format=tar HEAD | tar -x -C "$destination"
    git -C "$destination" init --quiet
    git -C "$destination" config user.email 'fixture@example.invalid'
    git -C "$destination" config user.name 'Provenance Fixture'
    git -C "$destination" add --all
    git -C "$destination" commit --quiet -m fixture
}

# 1. Clean canonical checkout.
clean_tree="$fixture_root/clean"
make_git_fixture "$clean_tree"
clean_commit=$(git -C "$clean_tree" rev-parse --short=7 HEAD)
build_and_expect "$clean_tree" "$shared_target" "$package_version+$clean_commit"

# 2. A tracked source edit must re-run build.rs and serialize as dirty.
cp "$clean_tree/src/version.rs" "$clean_tree/src/version.rs.fixture-clean"
printf '\n// provenance fixture: tracked source made dirty\n' >> "$clean_tree/src/version.rs"
build_and_expect "$clean_tree" "$shared_target" "$package_version+$clean_commit-dirty"

# 3. Restore the exact tracked source and prove the same target directory
# refreshes back to clean, rather than retaining stale dirty metadata.
mv "$clean_tree/src/version.rs.fixture-clean" "$clean_tree/src/version.rs"
build_and_expect "$clean_tree" "$shared_target" "$package_version+$clean_commit"

# 4. The dirty query covers every tracked file, so a non-source document edit
# must also invalidate build.rs instead of producing a false-clean identity.
cp "$clean_tree/CHANGELOG.md" "$clean_tree/CHANGELOG.md.fixture-clean"
printf '\n<!-- provenance fixture: tracked documentation made dirty -->\n' >> "$clean_tree/CHANGELOG.md"
build_and_expect "$clean_tree" "$shared_target" "$package_version+$clean_commit-dirty"
mv "$clean_tree/CHANGELOG.md.fixture-clean" "$clean_tree/CHANGELOG.md"
build_and_expect "$clean_tree" "$shared_target" "$package_version+$clean_commit"

# 5. A canonical checkout with Git lookup forced unavailable must not look
# clean. Cargo and rustc stay on PATH; only `git` is shadowed by this shim.
git_unavailable="$fixture_root/git-unavailable"
mkdir -p "$git_unavailable"
printf '#!/usr/bin/env sh\nexit 127\n' > "$git_unavailable/git"
chmod +x "$git_unavailable/git"
PATH="$git_unavailable:$PATH" \
    build_and_expect \
    "$clean_tree" \
    "$fixture_root/target-git-unavailable" \
    "$package_version+unknown-dirty-unknown"

# 6. A status-only failure must preserve the known commit while marking its
# dirty state unknown. Use a fresh target so Cargo cannot reuse a prior
# build-script output merely because PATH changed.
real_git=$(command -v git)
git_status_unavailable="$fixture_root/git-status-unavailable"
mkdir -p "$git_status_unavailable"
printf '%s\n' '#!/usr/bin/env sh' > "$git_status_unavailable/git"
printf '%s\n' 'for arg in "$@"; do' >> "$git_status_unavailable/git"
printf '%s\n' '    if [ "$arg" = "status" ]; then exit 127; fi' >> "$git_status_unavailable/git"
printf '%s\n' 'done' >> "$git_status_unavailable/git"
printf '%s\n' 'exec "${REAL_GIT:?}" "$@"' >> "$git_status_unavailable/git"
chmod +x "$git_status_unavailable/git"
PATH="$git_status_unavailable:$PATH" REAL_GIT="$real_git" \
    build_and_expect \
    "$clean_tree" \
    "$fixture_root/target-status-unavailable" \
    "$package_version+$clean_commit-dirty-unknown"

# 7. A genuine source archive has no .git metadata and must say so explicitly.
archive_tree="$fixture_root/source-archive"
mkdir -p "$archive_tree"
git -C "$repo_root" archive --format=tar HEAD | tar -x -C "$archive_tree"
build_and_expect "$archive_tree" "$shared_target" "$package_version+unknown-dirty-unknown"

# 8. A source archive nested inside some unrelated Git repository must not
# borrow its ancestor's identity.
outer_repo="$fixture_root/unrelated-ancestor"
mkdir -p "$outer_repo/vendor/surreal-mind"
git -C "$outer_repo" init --quiet
git -C "$outer_repo" config user.email 'fixture@example.invalid'
git -C "$outer_repo" config user.name 'Unrelated Ancestor'
printf 'not surreal-mind\n' > "$outer_repo/README"
git -C "$outer_repo" add README
git -C "$outer_repo" commit --quiet -m unrelated
git -C "$repo_root" archive --format=tar HEAD | tar -x -C "$outer_repo/vendor/surreal-mind"
build_and_expect \
    "$outer_repo/vendor/surreal-mind" \
    "$shared_target" \
    "$package_version+unknown-dirty-unknown"

printf 'build provenance fixture passed\n'
