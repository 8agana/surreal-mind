---
date: 2025-12-27
type: technical-debt-audit
status: complete 
conducted by: Codex CLI (5.2-codex:High)
---

# SurrealMind Technical Debt Audit (2025-12-27)

## Executive Summary
- High-impact debt centers on stale tool schema entries for removed tools.
- Several modules appear unused (gemini, sessions, kg_extractor, prompt_metrics, prompt_critiques) plus an unused DB table (tool_sessions).
- Re-embed tooling is duplicated across binaries and library helpers with inconsistent DB access patterns.

## High Priority
- Stale tool enum still advertises removed tools (`legacymind_update`, `memories_moderate`, `memories_populate`). Location: `src/schemas.rs:94-105`. Suggested fix: remove removed tools from the enum and update any callers/tests/docs relying on it. Estimated impact: prevents clients requesting invalid help targets and reduces support/debug churn.

## Medium Priority
- Dead module: Gemini CLI client unused after removal of memories_populate. Locations: `src/gemini.rs:1-133`, `src/lib.rs:8`. Suggested fix: remove module and export if no longer planned, or gate behind feature flags. Estimated impact: smaller binary and less confusion about supported providers.
- Dead module: tool session storage unused. Locations: `src/sessions.rs:1-66`, `src/lib.rs:17`, `src/server/schema.rs:93-99`. Suggested fix: remove sessions module and drop `tool_sessions` table definition, or reintroduce usage explicitly. Estimated impact: reduces schema clutter and unused DB work.
- Dead/unused modules: `kg_extractor`, `prompt_metrics`, `prompt_critiques`. Locations: `src/kg_extractor.rs:1-200`, `src/prompt_metrics.rs:1-234`, `src/prompt_critiques.rs:1-220`, exports in `src/lib.rs:10-12`. Suggested fix: remove or move behind feature flags until reintroduced. Estimated impact: lower compile time and fewer stale APIs to maintain.
- Unused inner_voice scaffolding. Locations: `src/tools/inner_voice.rs:239-255` (Candidate struct), `src/tools/inner_voice.rs:1187-1225` (call_grok). Suggested fix: delete if planner/synth no longer uses them, or wire them in with tests. Estimated impact: reduces confusion and keeps inner_voice focused.
- Duplicate re-embed/update logic across binaries and lib. Locations: `src/bin/reembed.rs:30-152`, `src/bin/fix_dimensions.rs:30-135`, `src/lib.rs:83-174`, `src/server/db.rs:300-322`. Suggested fix: extract shared helper for embedding updates and have binaries call it; prefer one canonical query format. Estimated impact: reduces drift and future bug surface.
- Inconsistent DB access patterns for embedding updates. Locations: `src/lib.rs:83-165` (raw SQL via HTTP with string interpolation) vs `src/bin/reembed.rs:121-130` / `src/bin/fix_dimensions.rs:107-116` (SDK + bound params). Suggested fix: standardize on SDK with `type::thing` binding or a shared HTTP helper to avoid ID-format mismatches. Estimated impact: lowers risk of silent update failures.
- Missing error handling / silent fallback in run_reembed. Locations: `src/lib.rs:101-109` (unwrap_or_default on response parsing). Suggested fix: return explicit error if response shape is unexpected. Estimated impact: prevents silent no-op runs.
- Best-effort embedding persistence ignores errors. Location: `src/server/db.rs:312-322` (awaited but result ignored). Suggested fix: log failures at debug/warn to surface DB issues. Estimated impact: better observability during retrieval.

## Low Priority
- Unused dependencies. Locations: `Cargo.toml:20` (chrono-tz), `Cargo.toml:42` (rusqlite), `Cargo.toml:46` (serde_qs) with no code references. Suggested fix: remove or gate behind features if still planned. Estimated impact: smaller build graph and faster compiles.
- Commented-out code. Location: `src/bin/reembed.rs:2` (`// use chrono::Utc;`). Suggested fix: delete. Estimated impact: tiny cleanup.
- TODOs awaiting follow-up. Locations: `tests/tool_schemas.rs:220`, `scripts/validate_contacts.py:144,153`. Suggested fix: implement or move to tracker. Estimated impact: reduces uncertainty around tests and data hygiene.
- Minor dead_code allowances masking unused fields. Locations: `src/cognitive/profile.rs:25-55`, `src/bin/kg_dedupe_plan.rs:16-18`. Suggested fix: remove allow(dead_code) or use fields in scoring. Estimated impact: small clarity win.
- Hardcoded tool-count log is stale. Location: `src/main.rs:89-91`. Suggested fix: build from actual tool registry or remove count. Estimated impact: avoids operator confusion.
- Unwrap/expect in CLI-only utilities can panic on malformed input. Locations: `src/bin/migration.rs:218`, `src/bin/sanity_cosine.rs:44-70`. Suggested fix: convert to error returns with context. Estimated impact: safer diagnostics; low production risk.
- Docs drift for removed tools. Locations: `docs/AGENTS/tools.md:9-11`, `README.md:8-70`. Suggested fix: remove `memories_moderate`/`memories_populate` references. Estimated impact: reduces onboarding confusion.
