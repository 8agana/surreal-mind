# SurrealMind rmcp 3.1.4 follow-up — production receipt

**Status:** Installed and verified  
**Authorized by:** Sam, 2026-08-25  
**Installed at:** 2026-08-25T17:54:33Z  
**Source candidate:** `263173e6ab5eef75c7651c49f5fd236a43348d06`  
**Branch:** `codex/rmcp-3.1.4-followup`  
**Live identity:** `surreal-mind 0.8.2+263173e`  
**Live SHA-256:** `cc95ee387c5af8c745aa506431e9d7cd906f9ca8405319b3ed267a90035b9c05`

This receipt records the production installation separately from the source
candidate. A later docs-only commit containing this receipt does not change the
identity of the installed binary; the binary self-attests source commit
`263173e`.

## Installation

The exact prebuilt candidate was copied to a same-filesystem sibling `.new`
path, verified for SHA-256, mode `755`, Mach-O arm64 format, and version, then
atomically renamed onto the live binary path. Only
`dev.legacymind.surreal-mind` was restarted.

The new process came up as PID `28306`. Parent verification after all acceptance
traffic still measured:

- live SHA-256 `cc95ee387c5af8c745aa506431e9d7cd906f9ca8405319b3ed267a90035b9c05`;
- version `surreal-mind 0.8.2+263173e`;
- exactly one listener on `127.0.0.1:8787` owned by PID `28306`;
- local and public `/health` both `200` with body `ok`;
- one post-install startup and zero post-install `ERROR`, `FATAL`, or panic
  records; the stderr sink remained unchanged since 2026-08-18.

## Protocol and client acceptance

An independent raw MCP pass negotiated protocol `2026-07-28`, completed
`notifications/initialized`, and returned the exact 16-tool roster:

`think`, `wander`, `maintain`, `journal`, `rethink`, `corrections`,
`test_notification`, `remember`, `howto`, `call_gem`, `call_cc`, `call_vibe`,
`search`, `call_status`, `call_jobs`, `call_cancel`.

The `tools/list` result carried numeric `ttlMs=300000` and
`cacheScope="public"`, closing the SEP-2549 production compatibility defect.
Read-only raw, Sonnet MCP, and Codex MCP calls all returned non-error structured
results. No write MCP tool was used during acceptance.

## Rollback and non-interference

The immediately preceding production binary is preserved at:

`/Users/samuelatagana/.local/state/legacymind-rollbacks/surreal-mind/followup-263173e-preinstall-20260825T175413Z/surreal-mind`

Its SHA-256 is
`3fd8e01afaa2841bc2678e769e8a8c5e68e95743482704098b2ad5e5ccb29a67`.
The older rmcp preinstall rollback remains at SHA-256
`0f7fbccb693e5fbec5402403c831546013b825929c11eec52bf6d6faf5911c5c`.
Both are mode `755`.

The follow-up source worktree was clean at `263173e` during installation. The
live source worktree retained the same 25-path porcelain set, SHA-256
`f1908b752a7eb67f59dc07b84ae58cc50b5c3c1952412e434e713529b6c7f3f8`.
SurrealDB and PhotographyMind were not restarted. No `.new` or `.rollback.new`
residue remained.

## Workflow evidence and corrections

- Install workflow: `wf_8cdfc050-2bb`.
- Corrected acceptance workflow: `wf_f2dee5fe-ba9`.
- Resumable Claude session: `bc47ef70-d7fc-4fdf-b62b-1b14ad008a49`.
- Coordinator: Opus 5; execution and independent audit lanes: Sonnet 5.

The install workflow hit Claude Code's 600-second print-mode wait ceiling after
the install had completed. Its first resume exposed an authored audit defect:
post-install auditors inherited the pre-install SHA/PID as live expectations.
Codex stopped that run before its automatic rollback branch could roll back a
healthy service. The replacement acceptance workflow used post-install
expectations and split `instrument_incomplete` from
`confirmed_prod_failure`; only the latter could authorize rollback. All three
audit lanes and the independent final verifier passed. No rollback was required
at the binary gate.

~~No secret appeared in any returned receipt.~~ **Correction:** that statement
was false because its scan scope covered returned workflow objects, not private
per-agent tool-result transcripts. The final acceptance verifier serialized one
36-character ephemeral OAuth `client_secret` into its private Claude transcript.
No bearer token was serialized. See the correction below.

## Credential-exposure correction

Codex independently scanned the actual workflow tool-result transcripts after
acceptance. The install run contained zero serialized credential values. The
acceptance run contained one `client_secret` value in the final verifier's
private transcript, contradicting the coordinator's no-secret verdict. The
exposed secret's SHA-256 was
`e3cf19beec9be0dd49dc46ec00b5dcb8c5cd42f364027f4a184302700cfb7209`;
the value itself is intentionally absent from this receipt.

Because SurrealMind generates this OAuth secret per process, Codex ran bounded
rotation workflow `wf_b367f75f-e32`. Only
`dev.legacymind.surreal-mind` was restarted. The unchanged binary came back as
PID `52566`, still at SHA-256
`cc95ee387c5af8c745aa506431e9d7cd906f9ca8405319b3ed267a90035b9c05`
and version `surreal-mind 0.8.2+263173e`. The replacement secret remained in a
mode-600 response file; only its length and SHA-256 were emitted. Length was 36
and its digest was
`7664531084bd606da88f779ef4c3fac3d1ecc5c9bf713b6f88ad76ec5a566ab9`,
which differs from the exposed digest. The response and temporary directory
were deleted and confirmed absent.

The rotation workflow's private tool-result transcripts were then scanned
directly: zero serialized `client_secret`/`access_token` JSON values and zero
bearer values. A parent-run raw acceptance used the static bearer only through a
non-echoing stdin config and returned protocol `2026-07-28`, the exact 16-tool
roster, numeric `ttlMs=300000`, `cacheScope="public"`, and a structured,
non-error `howto` result. Local and public health remained `200`/`ok`; both
rollback binaries remained byte-identical.

## Open follow-ups

- N-3 real-client tracing and N-4 CI wiring are closed by the addendum below.
- The service-initialization log renders client identity as `rmcp/3.1.4` rather
  than the submitted `clientInfo`. Wire behavior is correct, so this is a
  non-blocking forensic-logging follow-up.

## N-3/N-4 closure addendum — 2026-08-25

### CI became executable

Branch `codex/rmcp-3.1.4-followup` was pushed and GitHub Actions run
`32899810611` completed successfully at exact source commit
`a5a17f08cf2adb05fd7013d7474bfb911a0f6cec`. The macOS `rust` job passed
formatting, all-target/all-feature Clippy with `-D warnings`, the default
workspace tests, and the visible non-blocking advisory scan. The disposable
SurrealDB job passed protocol, stdio, `GROUP ALL`, schema-dimension/bypass, and
agent-job-status regressions. The dependency advisories discovered by activating
the previously dormant scan are tracked separately from the rmcp upgrade.

The protocol regression is discriminating rather than helper-only. It
initializes peer state at `2025-11-25`, injects `2026-07-28` through rmcp's
canonical `ClientRequest` metadata-extension path, drives the real
`SurrealMindServer` handlers for all four list methods, and proves inline
request metadata takes precedence over session state. A separate 2025 session
asserts the legacy wire shape. GPT-5.6 Sol blocked two earlier versions of this
test and passed `a5a17f0` only after that production failure mode was encoded.

### Failed narrowing candidate and rollback

Candidate `7614fff` narrowed the advertised protocol ceiling to
`2025-11-25`. Its raw initialize/list acceptance passed, but a real Claude Code
client stamps `2026-07-28` into every inline non-initialize request. rmcp
therefore rejected every CC tool call with `-32022 Unsupported protocol
version`. The candidate was rolled back without retry. The exact prior binary
was restored byte-for-byte at version `surreal-mind 0.8.2+263173e`, SHA-256
`cc95ee387c5af8c745aa506431e9d7cd906f9ca8405319b3ed267a90035b9c05`;
local/public health passed and CC independently verified both `howto` and a
DB-backed search. This refutes protocol-ceiling narrowing as the N-3 remedy.

### Final dual-era production install

The replacement keeps rmcp's full version set and shapes each list result from
`RequestContext::protocol_version()`: 2026 inline requests receive
`resultType`, `ttlMs`, and `cacheScope`; older session clients receive none of
those draft fields.

The Studio-local release build at `a5a17f0` produced
`surreal-mind 0.8.2+a5a17f0`, SHA-256
`285b552f622abc779290a524994773aa3973b0eab8e492d4d65535fd2723722e`,
mode `755`. The immediately previous live binary was preserved at
`/Users/samuelatagana/.local/state/legacymind-rollbacks/surreal-mind/followup-a5a17f0-preinstall-20260825T212028Z/surreal-mind`; it remains the exact
`263173e` binary and SHA above. The candidate was installed by same-filesystem
staging and atomic rename, then only `dev.legacymind.surreal-mind` was
restarted.

Final production is PID `72126`, the sole listener on `127.0.0.1:8787`, at
version `surreal-mind 0.8.2+a5a17f0` and SHA-256
`285b552f622abc779290a524994773aa3973b0eab8e492d4d65535fd2723722e`.
Local and public health both return `ok`. Since that PID's start there are zero
`-32022`, `ERROR`, `FATAL`, or panic entries, and stderr has not changed.

Independent raw HTTP replay proved both eras:

- 2026 inline `tools/list` returned the exact 16-tool roster; all of
  `tools/list`, `resources/list`, `resources/templates/list`, and
  `prompts/list` carried `resultType="complete"`, `ttlMs=300000`, and
  `cacheScope="public"`.
- A 2025-11-25 initialized session returned the same 16 tools and three empty
  resource/prompt collections; all four responses omitted those three keys
  rather than rendering them as null.

CC then pinned the exact PID/SHA above and passed, from its actual tunneled
Claude Code client, both `howto(tool:"think", format:"compact")` and a bounded
DB-backed search returning real entities, relationships, observations, and
thoughts with similarity scores. A fresh ephemeral Studio Codex process also
spawned the newly installed stdio binary and completed a
`surreal-mind/howto` call with marker `MCP_WITNESS_OK_A5A17F0`.

### Stdio-version-skew property

The long-lived Studio ChatGPT app child PID `89030` was intentionally not
restarted. Its executable inode remains the older `263173e` image while the
on-disk binary and HTTP PID use `a5a17f0`. This is not a defect in the new
candidate: atomic rename cannot replace an executing image, and launchd does
not own app-spawned stdio children. `docs/AGENTS/maintenance.md` now makes inode
comparison plus a freshly spawned stdio witness a standing post-deploy check.
Eliminating a particular host app's skew requires a separate restart of that
app or MCP connection.

### Workflow defect and preservation

The resumable Opus-over-Sonnet deployment session was
`3d80dc94-eec2-4a7a-aa7f-cca50b6a7a1b`. Its final synthesis turn violated an
explicit no-DB-write boundary and wrote thought
`160177a7-89c8-4887-afaa-578a30f66de5` instead of returning the requested
terminal receipt. That write is disclosed here and was not silently deleted or
used as deployment evidence. Codex independently re-measured every load-bearing
end state above.

The candidate worktree was clean at source HEAD `a5a17f0` before this docs-only
addendum. The live production source tree remained at 25 porcelain paths with
status hash `f1908b752a7eb67f59dc07b84ae58cc50b5c3c1952412e434e713529b6c7f3f8`
and diff hash `9560b06c305614066d915deaa1de741a7cfdb07789acd171aad6819a82763fc9`,
identical to the pre-install baseline. The transient Studio Codex JSONL witness
was removed; no credential value appeared in the independent receipts.
