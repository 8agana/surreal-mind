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

- N-3 real-client tracing and N-4 CI wiring remain deliberately open; neither
  blocked this installation.
- The service-initialization log renders client identity as `rmcp/3.1.4` rather
  than the submitted `clientInfo`. Wire behavior is correct, so this is a
  non-blocking forensic-logging follow-up.
