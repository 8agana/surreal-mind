---
date: 2025-12-27
type: Technical Debt Audit
status: Draft
scope: surreal-mind codebase
conducted by: Codex CLI (5.1-codex-max:high)
---

# Executive Summary
The core MCP surface is healthy, but several vestiges from recently removed tools and outdated instrumentation remain. The biggest risks are schema/help surfaces that still advertise non-existent tools and a stale tool-count log that under-reports active endpoints. Cleaning these will prevent client confusion, reduce support noise, and surface real dead-code warnings.

# Findings

## High Priority
- Removed tools still exposed in schemas and docs  
  - `src/schemas.rs:94-104` lists `legacymind_update`, `memories_moderate`, `memories_populate` in the `detailed_help_schema` enum even though handlers were removed.  
  - `docs/AGENTS/tools.md:10-11`, `README.md:8-9,65-76`, `CHANGELOG.md:4-9` still describe those tools.  
  - **Fix**: drop the three names from the enum and purge doc references; keep only live tools (legacymind_think, memories_create, legacymind_search, maintenance_ops, inner_voice, curiosity_* , detailed_help).  
  - **Impact**: Prevents clients from calling tools that 404 at runtime; reduces support churn. Medium effort, high clarity.

- Stale tool-count log on startup  
  - `src/main.rs:88-91` logs “Loaded 6 MCP tools” and omits the three curiosity tools now exposed in `list_tools`.  
  - **Fix**: align the log string with the current roster (9 tools) or build it dynamically from the list_tools vector.  
  - **Impact**: Avoids false alarms during ops/monitoring and keeps telemetry trustworthy. Trivial change, high clarity.

- Broken alias in detailed_help  
  - `src/tools/detailed_help.rs:183-185` advertises `knowledgegraph_search => memories_search`, but `memories_search` no longer exists. Requests for that alias return validation errors.  
  - **Fix**: remove the alias or repoint to `legacymind_search`; add curiosity_* entries so help output matches list_tools.  
  - **Impact**: Eliminates a hard failure path for callers; restores parity between help and actual tools. Low effort, high usefulness.

## Medium Priority
- Unused dependencies bloating builds  
  - `Cargo.toml:20` `chrono-tz` and `Cargo.toml:48` `strsim` have zero references in `src/`.  
  - **Fix**: remove from Cargo.toml and run `cargo update`; if needed later, reintroduce behind a feature.  
  - **Impact**: Faster builds and smaller attack surface. Low effort, medium gain.

- Dead-code allowances masking drift  
  - `src/server/db.rs:167-171` marks `cosine_similarity` with `#[allow(dead_code)]` though it is used;  
  - `src/tools/inner_voice.rs:1189-1221` keeps an unused `call_grok` path under `#[allow(dead_code)]`; similar allow on `Candidate` struct (`:242-257`).  
  - **Fix**: remove the allowances and delete/feature-gate unused helpers; let the compiler surface truly dead code.  
  - **Impact**: Restores lint signal and trims binary size. Moderate effort (surgical edits), medium gain.

- Detailed_help schema omits curiosity tools  
  - `src/schemas.rs:94-105` enum doesn’t list `curiosity_add/get/search`, so schema-aware clients can’t request help for them even though they are exposed.  
  - **Fix**: add the three curiosity tool names to the enum and to the help roster.  
  - **Impact**: Consistent client experience; small patch, medium clarity.

- Placeholder integration test never implemented  
  - `tests/tool_schemas.rs:207-221` contains a TODO integration test for list_tools under `db_integration`.  
  - **Fix**: either implement a real list_tools round-trip or delete the placeholder to avoid false confidence.  
  - **Impact**: Improves test signal; small effort, medium reliability.

## Low Priority
- Outdated doc claims about tool surface size  
  - README still says “Tool Surface (7)” and omits curiosity tools; AGENTS/tools repeats removed ones.  
  - **Fix**: refresh counts and lists after code-side cleanup.  
  - **Impact**: Documentation accuracy; low effort.

- Defensive unwraps in embeddings provider selection  
  - `src/embeddings.rs:189-207` uses `dims.unwrap()` on an Option derived from config; safe today but will panic if a future refactor drops the default.  
  - **Fix**: replace with explicit fallback (`unwrap_or(1536)`) or propagate a config error.  
  - **Impact**: Minor robustness gain; very low effort.

# Suggested Next Steps
1) Tidy schemas/help/logs and remove unused crates (highest signal, lowest risk).  
2) Drop `#[allow(dead_code)]` shields and prune unused helpers, then run `cargo clippy --all-targets`.  
3) Refresh README/AGENTS to match the live tool surface and add/trim integration coverage where promised.
