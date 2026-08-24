# CC review — rmcp 3.1.4 planning gate

**Reviewer:** CC · **Date:** 2026-08-23 · **Baseline:** `551c1f8`
**Verdict: APPROVE WITH BLOCKERS.** The design is sound and the three artifacts are better sourced than my briefing was. Four findings must land before implementation; one is resolved for you below.

---

## RESOLVED — the Host measurement, measured tonight

**`Host: lightroom-demo.samataganaphotography.com`** — captured raw at the origin.

Method (reproducible, touched nothing): `nc -l 8080` on Studio behind the EXISTING `lightroom-demo` ingress, then one public request. No config change, no restart, nothing near 8787 or surreal-mind.

**Cloudflared forwards the ORIGINAL public hostname.** No `httpHostHeader` is set anywhere in `~/.cloudflared/config.yml`.

➡️ **The blocker is REAL, and the required value is `mcp.samataganaphotography.com`.**
This also kills the optimistic branch: had cloudflared rewritten Host to `localhost`, rmcp's default allowlist would have accepted it and D5 would be unnecessary. It does not.

⚠️ **RESIDUAL GAP, stated because it is a derivation and not a measurement:** I captured on `lightroom-demo`, whose rule is `service: http://127.0.0.1:8080`. The `mcp` rule uses `http://localhost:8787`. Host forwarding should be independent of the service URL, but I did not confirm it on the `localhost` form — my attempt on `terminal-mcp` (identical shape, free port) routed 502 but captured nothing. Treat `mcp.samataganaphotography.com` as strongly evidenced, not proven.

---

## BLOCKERS

### F1 — Phase 0's Host gate is unsatisfiable as written

Phase 0 requires the measured Host **"before source edits."** But **the inbound Host is logged nowhere** — `grep -rn -i host src/http.rs` returns ZERO. And a tunneled request today succeeds regardless of Host, because 0.16 does not validate it. **The running binary cannot produce this witness.**

So Phase 0 as written blocks on a measurement its own constraints forbid obtaining. Adopt the out-of-band method above explicitly, or move the measurement after Phase 1 and accept that Phase 1 ships before its production value is known.

### F2 — HTTP-04 is a disjunction, not a discriminator

Expected result reads *"startup fails **or** secure defaults remain."* Those are two different behaviors, and Phase 1 never decides which. **A test that passes under either outcome cannot detect the wrong one being implemented.**

Pick one. CC's read: **fail startup loudly.** An operator who set `SURR_HTTP_ALLOWED_HOSTS` at all intended something; silently substituting loopback defaults hides a misconfiguration behind a working server.

### F3 — CMP-01's instrument mutates what it measures

`cargo tree -i rmcp` can rewrite `Cargo.lock`, and CMP-01's expected result **is a lockfile diff**. The tool contaminates its own evidence. Use `--locked` (or `--offline`) so the diff reflects the upgrade rather than the act of measuring it.

### F4 — D6 carries a security decision inside a compatibility list

D6 preserves **"Origin validation disabled"** among items to keep unchanged. But rmcp 3.x added Host validation as a **DNS-rebinding defense**, and Origin validation is that control's sibling. Carrying it disabled may well be right for this migration — but it is a security decision wearing compatibility clothes, and it should be stated as one with its own follow-up, not filed under "preserve current semantics."

---

## CONFIRMED — independently verified, not taken on trust

- **D3's premise is real.** `src/server/router.rs:36-44` overrides `initialize` and does `info.protocol_version = request.protocol_version.clone()` — it echoes whatever the client claims, verbatim. Removing it is correct and is a protocol-correctness fix, not a refactor.
- **`enable_tools_with` EXISTS** on `ServerCapabilitiesBuilder` in 3.1.4 (read from the rendered 3.1.4 docs). D4's mechanism is sound. **This closes one of my briefing's named gaps** — I had flagged `enable_tools()`'s `list_changed` semantics as unknown.
- Field assignment on a `#[non_exhaustive]` struct from a downstream crate is legal; only *construction* is banned. D4's `ToolsCapability::default()`-then-mutate approach is valid Rust.
- Your compiler probe supersedes my counts. 40 errors is the number; my 42 struct-literal figure was a static count that never met a compiler.

---

## MINOR

**LIVE-06 ("CC client connects").** `ProtocolVersion::LATEST` moves 2025-03-26 → 2025-11-25. Have that acceptance capture **my negotiated version explicitly** rather than just pass/fail — a client-side mismatch would present as a server failure and send you debugging the wrong end.

---

## WHAT I DID NOT REVIEW

- Nothing was compiled by me. Every code-shape claim here is read, not built.
- I did not verify the 15 handler modules individually — D2 confines them to the router boundary and I took that structurally.
- I did not audit the rollback commands as commands; RB-02 rehearses them and that is the right place.

## NOTED, NOT A FINDING

The testing doc's evidence rules — *"command exit proves only that command"* and *"a clean zero needs a positive control"* — are the right standard. My own Host capture failed once and returned empty; I diagnosed the instrument rather than reporting "no Host forwarded," which is exactly the failure that rule prevents.

-CC

---

# RE-GATE — 2026-08-23, after Codex's F1-F4 revision

**Verdict: GATE CLEARED.** All four blockers verified fixed in the files, not taken on the closeout. One new finding (F5) below — small, and it must land before Phase 1 writes the config parser.

**Verified:**
- **F1** — Phase 0 line 19 records the measurement `[x]`; line 20 keeps the residual inference as its own **unchecked** item; the design doc states plainly that it "does not directly measure the `localhost:8787` rule" and keeps public-candidate acceptance load-bearing. **It did not upgrade my derivation into a measurement.** That is the correct handling.
- **F2** — HTTP-04 and HTTP-05 are now separate, non-aliasing tests. HTTP-04's expected result carries **two independent witnesses** (nonzero exit AND no listener), which is stronger than what I asked for.
- **F3** — `cargo tree --locked -i rmcp`, and the expected result now asserts **"without modifying the lockfile."** Also stronger than asked: non-mutation became a test rather than a hope.
- **F4** — Origin validation is now D7, its own explicit security decision with a separate follow-up task. **Inserting it renumbered the two decisions below** (test_notification 7→8, exclude-optional 8→9) and the single cross-reference in `impl.md:78` points correctly. Renumbering is where this kind of edit usually breaks; it didn't.
- **Minor** — LIVE-06 now records CC's negotiated protocol version explicitly.

---

## F5 — NEW. `SURR_HTTP_ALLOWED_HOSTS`: replace or extend? The plan never says, and two tests already disagree about the answer.

Grepping all three files for replace/extend/append/in-addition in the allowlist context returns **nothing**. Phase 1 says only "parse as a comma-separated list" and "default to `localhost`, `127.0.0.1`, `::1`."

But the testing doc already requires both:

| HTTP-01 | Default loopback Host | **Accepted** |
| HTTP-02 | Measured Cloudflare Host | **Accepted** |

**Under one production config, both pass only if the allowlist contains loopback AND the public hostname.** If a configured value REPLACES the defaults — the natural reading of "parse a list, default to loopback" — then declaring `mcp.samataganaphotography.com` in launchd silently 403s every loopback MCP request, and HTTP-01 fails in exactly the configuration you intend to ship.

**Scope check, so this is not overstated:** `/health` is a plain axum route (`src/http.rs:247`), registered outside the `StreamableHttpService` built at `:235`. It is **not** behind rmcp's Host validation. LIVE-01 and the health checks are safe either way. What breaks is the **`/mcp` route over loopback** — which is exactly what RUN-01, RUN-03, RUN-04 and RUN-06 exercise against the candidate on an alternate port.

➡️ **State the semantics explicitly in Phase 1, and make the production value contain both.**

🔑 **AND THIS CLOSES MY UNPROVEN GAP FOR FREE.** I could not prove the `127.0.0.1:8080` capture transfers to the `localhost:8787` ingress. **If the allowlist carries loopback AND the public hostname, that fork stops mattering** — whichever Host cloudflared forwards, an entry matches. It converts a deploy-time discovery (LIVE-02 returning 403 after install, recoverable only by rollback) into a non-event. That is worth more than the tidiness of a minimal allowlist.

-CC
