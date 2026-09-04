# Antigravity CLI Migration Plan

> ⛔ **SUPERSEDED 2026-09-03 (fed-734b8f, commits fba5ac2..HEAD):** the "planned post-Antigravity replacement of all `call_*` tools" this plan anticipated has happened — `call_gem`, `call_cc`, `call_vibe`, `call_status`, `call_jobs`, `call_cancel` and `src/tools/call_gem.rs` no longer exist. The Antigravity client work described here (`src/clients/antigravity.rs`, kg_populate/kg_wander consumers) is still live. Kept as a dated record; do not treat the `call_*` sections as current.

Status: implementation approved through KG A/B; Antigravity default with Gemini rollback
Date: 2026-06-13 CDT
Scope: replace Gemini CLI usage inside surreal-mind with Antigravity CLI, without redesigning the full `call_*` tool family yet.

## Executive Summary

Gemini CLI is currently embedded in surreal-mind as a concrete runtime provider, not just as a public MCP tool name. The clean migration is therefore not a literal `gemini` -> `agy` command swap. The safer path is:

1. Introduce a provider-neutral agent event/response contract.
2. Add an Antigravity CLI client that matches the existing `CognitiveAgent` shape.
3. Route the existing `call_gem` compatibility surface to Antigravity internally.
4. Keep federation names like `gem`, `gemini`, and `call_gem` stable until the planned post-Antigravity replacement of all `call_*` tools.
5. Update KG maintenance binaries and docs to stop depending on Gemini-specific behavior.

This minimizes blast radius while Gemini CLI is being retired, and avoids wasting effort on a public rename that is already scheduled to be replaced.

## CC Review Amendments

CC approved the plan with four amendments. These are now part of the implementation sequence, not optional notes.

1. Auth durability is the hard go/no-go gate before Rust changes. `agy` uses browser sign-in and system keyring, which matches the prior gws failure mode. Do not build the migration until authentication survives:
   - Sam completes browser sign-in/onboarding in an interactive `agy` session.
   - A separate headless non-interactive process succeeds with `agy --print`.
   - Studio is rebooted.
   - A post-reboot headless `agy --print` succeeds without re-auth.
   - The MCP/launchd-spawned environment can also call `agy --print` without re-auth.
2. Capture real `agy --print` stdout/stderr before writing the parser. Do not implement parser behavior from a guessed fixture. First authenticated smoke captures the true output contract, then tests and parsing are written against that contract.
3. Run an A/B KG extraction quality check before cutting `kg_populate` or `kg_wander` over. Gemini's current stream-json mode has been reliable for extraction; Antigravity is more agentic. Compare same prompt/input across Gemini and Antigravity for JSON validity, markdown wrapping, extra commentary, latency, and extraction quality.
4. Make cutover runtime-reversible through env/config, not just git-reversible. Keep both Gemini and Antigravity clients selectable during the transition week so a bad Antigravity auth/runtime flake can be fixed by flipping configuration rather than rebuilding and restarting from source changes.

Review update:

- Auth gate cleared: `agy` works non-interactively in the relevant context.
- Output contract captured: `agy --print` returns plain stdout for normal prompts; KG extraction may return markdown-fenced JSON, which existing KG parsing handles.
- KG extraction A/B gate cleared by CC: real KG extraction prompt plus synthetic thought produced complete valid schema JSON with sensible entities, relationships, observations, boundary, and summary.
- Deployment target is now default `antigravity`, with `gemini` retained behind the same provider flag for rollback.

Permission posture amendment:

- `call_gem` may use `--dangerously-skip-permissions` when explicitly configured for interactive delegation.
- unattended binaries such as `kg_populate` and `kg_wander` should prefer `--sandbox` and should not globally inherit dangerous permission skipping.
- permission mode must be per-caller/per-use, not a single global default.

## External Facts Confirmed

Sources checked:

- Google migration announcement: https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/
- Antigravity CLI docs entrypoints:
  - https://antigravity.google/docs/cli-getting-started
  - https://antigravity.google/docs/cli-using
  - https://antigravity.google/docs/cli-features
- Public Antigravity CLI repository: https://github.com/google-antigravity/antigravity-cli
- Antigravity CLI product/blog page: https://antigravity.google/blog/introducing-google-antigravity-cli

Confirmed points:

- Google announced the transition from Gemini CLI to Antigravity CLI on 2026-05-19.
- For consumer access, Gemini CLI stops serving requests on 2026-06-18 for Google AI Pro, Ultra, and free Gemini Code Assist individual users.
- Enterprise Gemini CLI access remains supported in some cases, but this project should migrate because the daily-use local workflow is consumer-style CLI delegation.
- Antigravity CLI is available now and its public command appears to be `agy`.
- The public repo describes installation on macOS/Linux as `curl -fsSL https://antigravity.google/cli/install.sh | bash`.
- Antigravity CLI authenticates via system keyring and browser sign-in when needed.
- Google explicitly says there is not full 1:1 Gemini CLI feature parity at launch.
- Antigravity CLI has async/background workflow concepts and shares a core agent engine with Antigravity 2.0.

Local facts verified on Studio after install:

- The executable is `/Users/samuelatagana/.local/bin/agy`.
- No `anty` or `antigravity` executable is currently on `PATH`.
- `agy --version` returns `1.0.8`.
- `agy --help` confirms headless print mode and the core flags this migration needs:
  - `--print` / `-p`: run a single prompt non-interactively and print the response.
  - `--prompt`: alias for `--print`.
  - `--prompt-interactive` / `-i`: run an initial prompt interactively.
  - `--model`: set model for the current CLI session.
  - `--continue` / `-c`: continue the most recent conversation.
  - `--conversation`: resume a previous conversation by ID.
  - `--print-timeout`: timeout for print mode wait, default `5m0s`.
  - `--dangerously-skip-permissions`: auto-approve tool permission requests.
  - `--sandbox`: run with terminal restrictions enabled.
  - `--add-dir`: add workspace directories, repeatable.
- `agy models` exists, but currently returns: `Error: Please sign in to view available models. Launch the CLI without arguments to sign in.`
- `agy --print ...` currently blocks on Google OAuth and times out because CLI authentication is not complete.
- `agy changelog` works unauthenticated. Version `1.0.5` added `--model` and the `models` subcommand. Version `1.0.6` fixed `--sandbox` propagation in headless print mode. Version `1.0.7` increased maximum Gemini model tool calls to 512. Version `1.0.8` is the installed latest.

## Current Gemini Usage Inventory

Primary runtime integration:

- `src/clients/gemini.rs`
  - Defines `GeminiClient`.
  - Spawns `Command::new("gemini")`.
  - Sets `CI=true`, `TERM=dumb`, `NO_COLOR=1`.
  - Uses `-y`.
  - Uses `-m <model>` unless model is `auto`.
  - Uses `-e "" --output-format stream-json`.
  - Uses `--resume latest` or `--resume <session_id>`.
  - Passes prompt as a positional argument.
  - Parses Gemini-specific stream JSON into `GeminiStreamEvent`.
  - Implements activity-based timeout and per-tool timeout tracking.

MCP tool surface:

- `src/tools/call_gem.rs`
  - Deserializes `DelegateGeminiParams`.
  - Adds federation context and `observe` prefix.
  - Resolves `cwd` through workspace aliases.
  - Chooses model from `GEMINI_MODEL` or `config.system.gemini_model`.
  - Chooses timeout from `GEMINI_TIMEOUT_MS`.
  - Calls `GeminiClient`.
  - Emits Gemini-specific error messages.

- `src/schemas.rs`
  - `call_gem_schema()` requires `GEMINI_MODELS` and `GEMINI_MODEL` env vars at tool-list time.
  - `howto_schema()` includes `call_gem`.
  - Several enums still include `gemini` and `gem`.

- `src/server/router.rs`
  - Registers `call_gem`.
  - Describes it as Gemini CLI delegation.
  - Routes `call_gem` to `handle_call_gem`.

- `src/tools/howto.rs`
  - Documents `call_gem` as Gemini CLI.
  - Documents Gemini session resume behavior.

Shared abstraction leak:

- `src/clients/traits.rs`
  - `AgentResponse.stream_events` is typed as `Option<Vec<crate::clients::gemini::GeminiStreamEvent>>`.
  - This makes Gemini-specific stream events part of the shared client contract.

Configuration:

- `src/config.rs`
  - `SystemConfig.gemini_model`.
  - Default `gemini_model: "gemini-3-flash-preview"`.

- `surreal_mind.toml`
  - `gemini_model = "gemini-3-pro-preview"`.

Maintenance/KG binaries:

- `src/bin/kg_populate.rs`
  - Uses `GeminiClient` directly for KG extraction.
  - Uses `KG_POPULATE_MODEL` or `config.system.gemini_model`.

- `src/bin/kg_wander.rs`
  - Uses `GeminiClient` directly for next-step selection.
  - Hardcoded default `gemini-3-flash-preview`.

- `src/bin/gem_rethink.rs`
  - Processes records marked for `gemini`.
  - Mostly naming and queue semantics, not direct CLI execution.

Tests:

- `tests/test_gemini_call.rs`
  - Directly constructs a `gemini` command.

- `tests/gemini_client_integration.rs`
  - Uses `GeminiClient`.
  - Gated by `RUN_GEMINI_TESTS`.

- `tests/tool_schemas.rs` and MCP tests may depend on `GEMINI_MODEL(S)` env behavior.

Docs:

- `docs/AGENTS/tools.md`
- `docs/AGENTS/arch.md`
- `docs/AGENTS/maintenance.md`
- `docs/AGENTS.md`
- `GEMINI.md`
- Multiple archived task docs mention Gemini CLI. Archive files do not need mass rewriting unless they create active confusion.

## Design Principles

- Preserve public MCP compatibility until the planned `call_*` replacement.
- Avoid silently changing defaults. If config keys are aliased, document precedence.
- Keep provider-specific parsing inside provider clients.
- Do not require `GEMINI_MODEL(S)` just to list tools once Antigravity is the backend.
- Prefer typed provider-neutral events over `serde_json::Value` only where the project actually consumes fields.
- Make the first migration runtime-reversible: both providers remain selectable by env/config during the transition.
- Treat Antigravity's stdout/stderr contract as unverified until authenticated `agy --print` output is captured.
- Do not implement parser logic before real authenticated output exists. We are not rebuilding gws inside SM for sport.

## Proposed Architecture

### 1. Generalize shared agent events

Change `AgentResponse.stream_events` from:

```rust
Option<Vec<crate::clients::gemini::GeminiStreamEvent>>
```

to one of:

```rust
Option<Vec<serde_json::Value>>
```

or:

```rust
Option<Vec<AgentStreamEvent>>
```

Recommendation: use `serde_json::Value` for the immediate migration.

Reason: Antigravity stream event shape is not locally verified. A generic JSON event vector keeps the current optional exposure behavior without prematurely inventing a schema. Later, when `call_*` is replaced, define a proper federation event model.

Files:

- `src/clients/traits.rs`
- `src/clients/gemini.rs` or replacement client
- `src/tools/call_gem.rs`
- Any tests expecting Gemini event types

### 2. Add an Antigravity client behind provider selection

Create:

- `src/clients/antigravity.rs`

Keep:

- `src/clients/gemini.rs` during the transition week.

Add provider selection:

```rust
pub enum GoogleCliProvider {
    Gemini,
    Antigravity,
}
```

Provider selection precedence:

1. `GOOGLE_CLI_PROVIDER`
2. `SURR_GOOGLE_CLI_PROVIDER`
3. config field such as `system.google_cli_provider`
4. default `antigravity` after auth durability and KG A/B pass

Supported values:

- `antigravity`
- `agy`
- `gemini`

Transition behavior:

- `call_gem` keeps the same public MCP tool name.
- Internally, `call_gem` selects either `GeminiClient` or `AntigravityClient`.
- `kg_populate` and `kg_wander` select provider independently through the same flag, and now default to Antigravity after KG A/B signoff.

Suggested shape:

```rust
pub struct AntigravityClient {
    model: String,
    timeout: Duration,
    tool_timeout: Duration,
    cwd: Option<PathBuf>,
    expose_stream: bool,
}
```

Implement `CognitiveAgent` for `AntigravityClient`.

Initial behavior should mirror `GeminiClient` as much as Antigravity allows:

- executable: env `ANTIGRAVITY_CLI_BIN`, fallback `agy`
- model: env `ANTIGRAVITY_MODEL`, fallback `AGY_MODEL`, then compatibility fallback `GEMINI_MODEL`, then `"auto"`
- timeout: env `ANTIGRAVITY_TIMEOUT_MS`, fallback `AGY_TIMEOUT_MS`, then compatibility fallback `GEMINI_TIMEOUT_MS`, then current default
- tool timeout: env `ANTIGRAVITY_TOOL_TIMEOUT_MS`, fallback `AGY_TOOL_TIMEOUT_MS`, then compatibility fallback `GEMINI_TOOL_TIMEOUT_MS`
- permission mode: per caller
  - interactive `call_gem`: optionally `--dangerously-skip-permissions` via explicit config
  - unattended KG binaries: prefer `--sandbox`; do not use dangerous skip by default
- cwd: `Command::current_dir`
- no secrets in logs
- `kill_on_drop(true)`

Verified local headless command shape:

```bash
agy --print "<prompt>" \
  --model "<model>" \
  --print-timeout 300s \
  --dangerously-skip-permissions
```

or, using aliases:

```bash
agy -p "<prompt>" --model "<model>" --print-timeout 300s --dangerously-skip-permissions
```

Resume/continuation candidates verified by help:

```bash
agy --print "<prompt>" --continue
agy --print "<prompt>" --conversation "<conversation_id>"
```

Workspace candidates verified by help:

```bash
agy --print "<prompt>" --add-dir /path/to/extra/workspace
```

Remaining verification after sign-in:

- stdout/stderr response shape for `--print`.
- whether `--print` emits plain text only or any structured metadata.
- whether `--conversation` expects the same ID shown in CLI `/resume`.
- whether `--continue` works reliably from an MCP-launched non-interactive process.
- whether `--dangerously-skip-permissions` is sufficient for MCP automation.
- whether `--sandbox` should be preferred for observe mode or high-risk prompts.
- whether there is any structured output flag not shown by top-level help.
- whether first authenticated launch requires workspace trust for `/Users/samuelatagana/Projects/LegacyMind/surreal-mind`.

Until these are verified, do not write parser logic. Authenticated smoke must capture the true output first. Command construction should still be isolated in small helper functions so failures are obvious and easy to patch.

### 3. Keep `call_gem` as a compatibility tool for now

Do not add a new public `call_agy` tool in this migration unless reviewers strongly prefer it.

Reason: Sam plans to replace the `call_*` tools after Antigravity is working. Adding a new public tool now creates a second surface to migrate again shortly.

Change `call_gem` internals and docs to say:

- MCP tool name: `call_gem` remains as a temporary compatibility alias.
- Runtime provider: Antigravity CLI.
- Federation target identity: still `gem`/`gemini` until federation vocabulary is redesigned.

Specific updates:

- `src/tools/call_gem.rs`
  - Rename internal structs/comments from `DelegateGeminiParams`/`GeminiCallParams` to provider-neutral names if practical.
  - Replace `GeminiClient` with `AntigravityClient`.
  - Replace user-facing error strings from `Gemini CLI` to `Antigravity CLI`.
  - Preserve response JSON fields: `status`, `session_id`, `response`, optional `stream_events`.

- `src/server/router.rs`
  - Keep name `call_gem`.
  - Change description to "Delegate a task to Antigravity CLI (temporary call_gem compatibility alias)".

- `src/schemas.rs`
  - Avoid hard failure when `GEMINI_MODELS` or `GEMINI_MODEL` are missing.
  - Prefer `ANTIGRAVITY_MODELS` / `ANTIGRAVITY_MODEL`.
  - Fall back to `AGY_MODELS` / `AGY_MODEL`.
  - Then fall back to existing `GEMINI_MODELS` / `GEMINI_MODEL` for compatibility.
  - If no model list exists, omit `enum` and default to `"auto"` rather than panic during tools/list.

### 4. Update config with compatibility aliases

Preferred minimal config change:

- Add `google_cli_provider` or equivalent selector with supported values `antigravity`, `agy`, `gemini`.
- Add `antigravity_model` to `SystemConfig`.
- Keep `gemini_model` temporarily for existing TOML compatibility.
- In config loading, derive provider model using precedence:
  1. `ANTIGRAVITY_MODEL`
  2. `AGY_MODEL`
  3. `GEMINI_MODEL`
  4. `system.antigravity_model`
  5. `system.gemini_model`
  6. `"auto"`

Potential issue: adding a required field to TOML deserialization can break existing config. Use serde defaults or make it optional:

```rust
#[serde(default = "default_antigravity_model")]
pub antigravity_model: String,
```

Also add separate permission-mode config rather than hardcoding `--dangerously-skip-permissions`:

- `ANTIGRAVITY_PERMISSION_MODE=interactive_skip|sandbox|default`
- `ANTIGRAVITY_CALL_GEM_PERMISSION_MODE`
- `ANTIGRAVITY_KG_PERMISSION_MODE`

Exact names can be simplified during implementation, but the behavior must stay per-caller.

Then later, during the full `call_*` redesign, remove `gemini_model` and the temporary provider selector.

### 5. Replace direct KG binary dependencies

`kg_populate` and `kg_wander` should stop hardcoding `GeminiClient`; A/B quality checks have passed, so the code can default to Antigravity while retaining Gemini rollback.

Option A, recommended:

- Add a provider-neutral alias/helper:

```rust
pub type DefaultGoogleAgentClient = AntigravityClient;
```

or:

```rust
pub fn default_delegate_client(model: String, timeout_ms: u64) -> impl CognitiveAgent
```

Option B:

- Directly import `AntigravityClient`.

Recommendation: use a small provider factory instead of direct import for the immediate migration, because runtime fallback is now required.

Files:

- `src/bin/kg_populate.rs`
- `src/bin/kg_wander.rs`
- comments and logs in those files

Behavior to preserve:

- `kg_populate` still expects useful JSON-ish content and already has response cleanup logic.
- `kg_wander` still expects a decision JSON from the agent response.
- If Antigravity wraps JSON in markdown or emits richer event streams, keep existing markdown fence stripping and parse diagnostics.

Required A/B acceptance before cutover:

- same KG extraction prompt/input through Gemini and Antigravity
- valid parseable JSON from Antigravity without human cleanup
- no extra tool chatter mixed into extraction payload
- comparable entity/relationship coverage on a small representative batch
- latency within an acceptable bound for nightly/maintenance use
- failure mode returns useful stderr/stdout snippets, not empty response mysteries

### 6. Update tests

Test changes:

- Replace `tests/test_gemini_call.rs` with `tests/test_antigravity_call.rs` or rewrite it to test command construction helper functions without requiring installed `agy`.
- Replace `tests/gemini_client_integration.rs` with `tests/antigravity_client_integration.rs`.
- Gate live tests behind `RUN_ANTIGRAVITY_TESTS=1`.
- Use `ANTIGRAVITY_CLI_BIN` so tests can point to a fixture script.
- Add a unit test for schema generation when no model env vars are set. This guards against tool-list panic.

Recommended fake CLI fixture:

- A small shell script under `tests/fixtures/agy_fake.sh`.
- It should emit output matching the real authenticated `agy --print` output shape.
- Do not use guessed generic NDJSON as the primary fixture.
- If real output is plain text, fixture should be plain text.
- If real output includes JSON/metadata, fixture should include the real field names.

Fallback exploratory fixture, only before parser implementation:

```json
{"type":"init","session_id":"test-session","model":"test-model"}
{"type":"content","text":"hello"}
{"type":"end","session_id":"test-session"}
```

This exploratory fixture is only for command wiring experiments. It must be replaced before parser behavior is considered implemented.

### 7. Update docs

Active docs to update:

- `docs/AGENTS/tools.md`
- `docs/AGENTS/arch.md`
- `docs/AGENTS/maintenance.md`
- `docs/AGENTS.md`
- `GEMINI.md`
- `README.md` if it mentions Gemini CLI as active setup

Do not mass-edit archived task history. Add a short note in this plan or active docs that archived Gemini references are historical.

Doc language should be explicit:

- `call_gem` is temporary compatibility naming.
- Runtime provider is Antigravity CLI.
- Full delegation tool rename/replacement is intentionally deferred.

### 8. Verification plan

Phase 0: auth durability go/no-go gate

```bash
command -v agy
agy --version
agy --help
agy models
```

Capture:

- exact prompt flag: verified as `--print` / `-p`.
- exact model flag: verified as `--model`.
- exact resume/continue support: verified as `--continue` and `--conversation`; runtime behavior still needs authenticated smoke.
- exact non-interactive/permission behavior: verified flag exists as `--dangerously-skip-permissions`; runtime behavior still needs authenticated smoke.
- whether first run requires trusted-folder interaction.
- whether headless mode exits cleanly in `CI=true TERM=dumb NO_COLOR=1`.
- whether structured output exists; top-level help does not advertise Gemini-style `--output-format stream-json`.

Current blocker:

- CLI auth is not complete. `agy models` and `agy --print` require sign-in. Launch `agy` without arguments on Studio and complete Google OAuth/onboarding before live smoke tests.

Hard gate:

```bash
agy --print "Reply with exactly: antigravity-smoke-ok" --print-timeout 30s
```

Pass criteria:

- succeeds from an ordinary shell after interactive sign-in
- succeeds from a fresh non-interactive shell
- succeeds after Studio reboot without re-auth
- succeeds from the same user/environment shape used by launchd/MCP

If any of these fail, stop. Do not write Rust. Solve auth durability first.

Phase 0.5: capture real `agy --print` output contract

Run and save stdout/stderr snippets for:

```bash
agy --print "Reply with exactly: antigravity-smoke-ok" --print-timeout 30s
agy --print "Reply with JSON only: {\"ok\":true}" --print-timeout 30s
agy --print "Reply with exactly: antigravity-smoke-ok" --model "<known-model>" --print-timeout 30s
agy --print "Reply with exactly: antigravity-smoke-ok" --continue --print-timeout 30s
```

Capture:

- stdout shape
- stderr shape
- exit status on success
- exit status and stderr when unauthenticated
- whether any conversation/session ID is visible
- whether JSON prompts are wrapped in markdown fences or extra commentary

Parser and test fixture work starts only after this capture.

Phase 0.75: KG extraction A/B before unattended cutover

Run the same representative KG extraction prompt/input through:

- existing Gemini CLI path
- direct `agy --print`

Compare:

- JSON parse success
- markdown wrapping
- extra commentary/tool chatter
- entity/relationship coverage
- latency
- failure mode clarity

`kg_populate` and `kg_wander` can default to Antigravity after this pass; Gemini remains selectable for rollback.

Phase 1: Rust static checks

```bash
cargo fmt --check
cargo check
cargo test --no-run
```

Phase 2: unit/fixture tests

```bash
cargo test antigravity
cargo test tool_schemas
```

Phase 3: live smoke tests, gated

```bash
RUN_ANTIGRAVITY_TESTS=1 cargo test antigravity_client_integration -- --nocapture
```

Before Rust live tests, rerun direct CLI smoke:

```bash
agy --print "Reply with exactly: antigravity-smoke-ok" --print-timeout 30s
agy --print "Reply with JSON only: {\"ok\":true}" --print-timeout 30s
agy models
```

Phase 4: MCP tool smoke, after rebuild/restart approval

```bash
smbuild
launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind
curl http://127.0.0.1:8787/health
```

Then from an MCP client:

- `tools/list` must not panic without Gemini env vars.
- `call_gem` observe-mode prompt returns a response through Antigravity.
- `call_gem` with invalid `ANTIGRAVITY_CLI_BIN` returns a clear "Antigravity CLI not found" style error.
- `call_gem` honors workspace alias `cwd`.
- `continue_latest` either works or returns a documented unsupported-feature error.

## Suggested Implementation Sequence

No Rust implementation starts until Phase 0 and Phase 0.5 pass. Phase 0.75 additionally gates unattended KG binary cutover.

### Step 1: Contract prep

- Only start after Phase 0 auth durability and Phase 0.5 output capture pass.
- Change `AgentResponse.stream_events` to generic JSON values.
- Adjust `GeminiClient` temporarily to emit generic events so tests still compile.
- Run `cargo check`.

Review checkpoint: this should be behavior-preserving.

### Step 2: Add Antigravity client

- Use the captured Phase 0.5 output as the parser source of truth.
- Add `src/clients/antigravity.rs`.
- Export `AntigravityClient` from `src/clients/mod.rs`.
- Implement command construction in a helper that can be unit-tested.
- Implement parser from captured real `agy --print` output:
  - Prefer structured content events if present.
  - Fall back to raw stdout.
  - Extract session ID from common field names: `session_id`, `sessionId`, `conversation_id`, `thread_id`.
- Preserve activity timeout logic if streaming output is available.
- If `agy` only supports blocking output, use the simpler `tokio::time::timeout(cmd.output())` pattern, like `VibeClient`.

Review checkpoint: fake CLI test passes.

### Step 3: Swap `call_gem` backend

- Add provider selection for `GeminiClient` vs `AntigravityClient`.
- Default to Antigravity now that auth durability and direct smoke passed.
- Keep Gemini selectable for rollback.
- Keep tool name and response shape.
- Update errors/descriptions to Antigravity.
- Preserve compatibility env fallback.

Review checkpoint: `call_gem` still compiles and schema no longer panics without Gemini env.

### Step 4: Migrate direct binaries

- Phase 0.75 KG A/B has passed; keep the cutover behind the provider flag.
- Update `kg_populate`.
- Update `kg_wander`.
- Keep provider env fallback so unattended jobs can flip back during transition.
- Use sandbox/default permission mode for unattended KG paths, not global dangerous skip.
- Rename logs/comments enough that active output no longer claims Gemini CLI.
- Leave `gem_rethink` target naming alone unless reviewers want queue target vocabulary touched now.

Review checkpoint: `cargo check --bins`.

### Step 5: Docs/tests cleanup

- Update active docs.
- Add test fixture and integration gate.
- Mark Gemini archives as historical by omission, not churn.

Review checkpoint: `cargo test --no-run`.

### Step 6: Live validation

- Install/login/trust workspace for `agy` manually or with Sam present if first-run auth prompts are required.
- Capture actual `agy --print` output shape before parser implementation.
- Reboot Studio and rerun direct/headless smoke before code cutover.
- Patch command flags/parser if needed.
- Run MCP smoke.

## Risks and Mitigations

Risk: Antigravity CLI does not support Gemini's `--output-format stream-json`.

- Mitigation: parser must handle plain stdout. `expose_stream` can return no events with a documented note until the later `call_*` redesign.

Risk: Antigravity CLI does not support session resume IDs.

- Mitigation: preserve `resume_session_id` in schema but return a clear unsupported error only when callers use it, or map to Antigravity's closest conversation continuation flag if available.

Risk: First-run login/trust prompts block MCP calls.

- Status: confirmed locally. `agy --print` currently waits for OAuth and then times out.
- Mitigation: hard auth durability gate before Rust changes; add explicit setup docs and a preflight diagnostic. Do not let MCP handler hang silently; timeout with useful stderr/stdout snippet.

Risk: Antigravity auth works interactively but fails from launchd/MCP or after reboot.

- Mitigation: treat this as no-go. Keep Gemini provider fallback. Do not cut over until post-reboot and MCP-shaped headless smoke succeed.

Risk: Tool-list currently panics if Gemini model env vars are absent.

- Mitigation: make model enum optional and default to `auto`.

Risk: Federation target names (`gem`, `gemini`) drift semantically.

- Mitigation: treat them as human/federation identity names, not runtime provider names, until the planned `call_*` replacement.

Risk: KG extraction quality changes because Antigravity is more agentic and less direct than Gemini CLI.

- Mitigation: run the A/B extraction gate before cutting over `kg_populate` or `kg_wander`; keep KG prompts stricter, verify JSON extraction output, and run small KG populate batches before full maintenance runs.

Risk: A runtime Antigravity flake during the shutdown week requires code rollback.

- Mitigation: keep both providers available behind env/config during the transition. Operational rollback should be a config flip plus restart, not a rebuild.

Risk: Antigravity's async/background behavior conflicts with synchronous MCP request/response.

- Mitigation: use the simplest headless one-shot mode if available. Do not adopt Antigravity background job orchestration inside surreal-mind until the larger delegation redesign.

## Open Questions for CC/Gem

1. What is the exact `agy` headless invocation for one prompt with deterministic process exit?
   - Mostly answered: `agy --print "<prompt>" --print-timeout <duration>`; authenticated smoke still needed.
2. Does `agy` support structured output, NDJSON, or stream JSON?
   - Not shown in top-level help; likely plain stdout, but verify after auth.
3. Does `agy` support model selection by CLI flag, config only, or both?
   - Answered by help/changelog: `--model` exists; model names need `agy models` after auth.
4. Does `agy` support resume by session ID, continue latest, both, or neither?
   - Help shows `--conversation` and `--continue`; exact ID semantics need smoke.
5. Is there a safe non-interactive permission flag equivalent to Gemini `-y`?
   - Help shows `--dangerously-skip-permissions`; CC amendment says per-caller only. Interactive `call_gem` may use it behind config; unattended KG should prefer `--sandbox`.
6. Does `agy` have a read-only/plan/observe mode that should replace the current prompt prefix?
7. Are Antigravity sessions stored locally in a way surreal-mind should expose, or should `session_id` become best-effort?
8. Should `gem_rethink` continue using `marked_for = 'gemini'`, or should this be deferred with the wider federation vocabulary cleanup?
9. Should the compatibility tool remain named `call_gem` only, or should we add `call_agy` as an alias even though `call_*` is being replaced later?

## Recommended Review Decision

Approve the migration if reviewers agree with these two constraints:

- `call_gem` remains the public compatibility name until the later `call_*` replacement.
- The first Antigravity implementation targets reliable one-shot delegation, not full adoption of Antigravity's async orchestration model.

If reviewers reject either constraint, redesign the public tool surface first. Otherwise, implement the narrow provider swap and keep the bigger naming cleanup for the planned delegation-tool replacement.
