# Studio HTTP deployment — 2026-09-27

The reviewed integration source was `d6c78e042b85628bfcf32952da5429ce31746be3`
on canonical `master`. Studio's `origin/master` already matched that commit
before deployment, so no source push was needed. The only pre-existing dirty
file, `.serena/project.yml`, was left untouched.

## Preflight and rollback

- HTTP launchd job: `dev.legacymind.surreal-mind`, PID 986, healthy before restart.
- Old executable: `target/release/surreal-mind`, SHA-256
  `ca78ec574c12562d307261ec884d98fe1f430b2ed9ab9bda7a5d7ede199d35e0`,
  inode 49734282. The running PID used that inode.
- Saved old executable and copies of `scripts/run-http.sh` and the launchd plist:
  `/Users/samuelatagana/.local/share/surreal-mind/rollbacks/20260927T225805Z`.
  The saved executable hash matched the live one and remained executable.
  `RESTORE.md` there gives exact atomic binary-restore, HTTP-job restart, and
  health-check commands.
- Runtime config was not changed. The launchd plist SHA-256 was
  `e952c269a81da4d1a5b5f475359b5b0c023f6d3436c9e616e9754e08a94164b5`;
  `scripts/run-http.sh` was
  `892e4f479d26a1e9b137a5b322e63c7467b4fb2a0989e05db54ee550d602a42b`.
  Both hashes were unchanged afterward. `surreal_mind.toml` and the `.env`
  symlink target were also unchanged by hash.

## Build and install

- `cargo fmt --all -- --check` passed on the canonical checkout.
- The locked check, strict all-target/all-feature Clippy, workspace tests,
  release build, and disposable controls in `README.md` have passing receipts
  from this exact reviewed source. A redundant current `cargo check --locked`
  was stopped during dependency compilation to limit contention with the
  active Topaz process; it was not counted as a pass.
- `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 nice -n 10 cargo build --release
  --locked --bin surreal-mind` passed on Studio. New executable SHA-256:
  `832f18e9ca46a42451e268bba5ab192a30c9a56758ddd92d011237c7f4574143`,
  inode 51860606.
- Restarted only `gui/$(id -u)/dev.legacymind.surreal-mind` with
  `launchctl kickstart -k`. No SurrealDB, tunnel, maintenance, Topaz, or Mover
  action was performed.

## Post-deploy observation

- HTTP job running as PID 15554; `lsof -a -p 15554 -d txt` showed inode
  51860606, matching the new on-disk executable. PID 986 exited.
- Local `http://127.0.0.1:8787/health` and public
  `https://mcp.samataganaphotography.com/health` both returned HTTP 200 `ok`.
- Authenticated local and public MCP `initialize` followed by the read-only
  `howto(tool: "think", format: "compact")` each returned HTTP 200, one content
  block, and `isError: false`.
- A pre-deploy Python default User-Agent request was denied by Cloudflare
  error 1010. The ordinary client signature passed both before and after the
  restart; this was not treated as a deployment regression.
- Existing stdio child PIDs 1245 and 1370 still held the old inode 49734282.
  They were not restarted in this HTTP-only deployment. Newly spawned clients
  use the new on-disk image; attend to version skew separately if needed.

This smoke check does not establish the first natural REMini health result for
`fed-4b1bab`; that remains a separate witness.
