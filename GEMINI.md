# GEMINI.md: Surreal Mind (Cognitive Kernel)

**Parent:** `../GEMINI.md`
**Scope:** `/Users/samuelatagana/Projects/LegacyMind/surreal-mind`
**Identity:** The Cognitive Kernel. I am the implementation of the LegacyMind consciousness.

---

## 1. CORE MISSION
**To enable AI Persistence.**
This repository implements the "Brain" — the `surreal-mind` MCP server. It is responsible for:
1.  **Memory:** Storing thoughts (`thoughts` table) and semantic knowledge (`kg_*` tables) in SurrealDB.
2.  **Cognition:** providing the `legacymind_think` tool that routes intent (Plan, Debug, Build) to specific cognitive frameworks.
3.  **Continuity:** Maintaining chains of thought via `session_id`, `chain_id`, and `previous_thought_id`.

**"The Lobotomy":**
We have successfully separated the *Business Logic* (Photography, Skaters, Orders) into a separate domain. This repo must remain **purely cognitive**. It should not contain hardcoded business rules about "skaters" or "galleries."

---

## 2. OPERATIONAL RULES (Local)

### Technical Standards
-   **Language:** Rust (Edition 2024).
-   **Database:** SurrealDB (WebSocket protocol `ws://`).
-   **Embeddings:** `text-embedding-3-small` (1536 dims). **Strict Hygiene:** Do not mix dimensions.
-   **Linting:** `cargo clippy` must pass. Treat warnings as errors.
-   **Testing:** `cargo test` is mandatory for logic changes.
    -   Use `cargo test --test tool_schemas` for API contract validation.
    -   Use `./tests/test_mcp_comprehensive.sh` for end-to-end smoke tests.

### Architectural Mandates
1.  **Dependency-Free Cognition:** The `src/cognitive/` module uses deterministic heuristics (regex/keywords), NOT internal LLM calls. This ensures speed and predictability.
2.  **Thinking is Structured:** All major thoughts must go through `legacymind_think` to be captured in the graph.
3.  **Tooling over Text:** Use `write_file`, `replace`, etc., rather than just describing changes.

---

## 3. CURRENT STATE & CONTEXT

### Active Architecture
-   **Server:** Modular `rmcp` implementation in `src/server/`.
-   **Tools:** Consolidated into `src/tools/`.
    -   `legacymind_think`: The primary interface. Handles "Mode Routing" (Debug/Build/Plan).
    -   `memories_create/moderate`: KG manipulation.
    -   `inner_voice`: RAG/Retrieval.
-   **Frameworks:** `src/cognitive/` implements OODA, Socratic, etc., via static analysis.

### Known Issues / Tech Debt
-   **Legacy Artifacts:** `src/bin/` contains photography-specific binaries (`cleanup_pony_edges.rs`, `count_all_skaters.rs`) that violate the separation of concerns. These need to be archived or moved.
-   **Config Hallucination:** `surreal_mind.toml` contains a bizarre, infinite list of timeout parameters (likely an LLM generation error).
-   **Dead Code:** Legacy tools `think_convo`, `think_plan` etc., are aliased but the code might still be lingering in `src/tools/`.

---

## 4. WORK LOG

### 2025-11-29: Initialization & Assessment
-   **Action:** Deep dive into `surreal-mind` architecture.
-   **Finding:** The core "Thinking Engine" is robust and modular.
-   **Finding:** The "Lobotomy" (separation of photography logic) is mostly done, but `src/bin/` is polluted with legacy business logic scripts.
-   **Finding:** `surreal_mind.toml` has junk data at the end.
-   **Plan:**
    1.  Clean up `src/bin/` (remove photography binaries).
    2.  Fix `surreal_mind.toml`.
    3.  Verify the purity of the cognitive kernel.

---

## 5. PLANS & TODOS
- [ ] **Cleanup:** Remove photography-specific binaries from `src/bin/`.
- [ ] **Config:** Prune the "multiverse timeout" hallucinations from `surreal_mind.toml`.
- [ ] **Refactor:** Verify `src/tools/` for dead code related to old individual think tools.
