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

# 4. A genuine source archive has no .git metadata and must say so explicitly.
archive_tree="$fixture_root/source-archive"
mkdir -p "$archive_tree"
git -C "$repo_root" archive --format=tar HEAD | tar -x -C "$archive_tree"
build_and_expect "$archive_tree" "$shared_target" "$package_version+unknown-dirty-unknown"

# 5. A source archive nested inside some unrelated Git repository must not
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
