# SurrealMind Think Tools Refactor Proposal
**Date:** 2025-08-31
**Author:** Warp
**Project:** LegacyMind / SurrealMind

## Executive Summary

This proposal outlines a comprehensive refactor of the SurrealMind thinking tools to create a more intuitive, powerful, and cognitively-enhanced system. The refactor moves from submode-based tools to discrete, specialized thinking engines with evolving cognitive models and introduces an Augmented Introspection system for internal dialog.

## Current State Issues

1. **Confusing tool names** (`convo_think`, `tech_think`) don't group logically
2. **Submodes add complexity** without clear benefit
3. **Memory injection returns 0** due to code ordering bug (FIXED)
4. **Search functionality broken** - empty results even for just-created thoughts
5. **Knowledge Graph terminology** doesn't reflect personal memory purpose
6. **Static thinking models** don't adapt or evolve

## Proposed Architecture

### Tool Naming Convention
- **Think Tools:** `think_*` for thought storage and retrieval
- **Memory Tools:** `memories_*` for knowledge graph operations
- **Intelligence:** `inner_voice` for RAG synthesis
- **Utility:** `maintenance_ops`, `detailed_help`

### Discrete Thinking Tools
Replace submode parameters with discrete, specialized tools:
- `think_convo` - Conversational thoughts
- `think_plan` - Architecture and strategy
- `think_debug` - Problem solving
- `think_build` - Implementation
- `think_stuck` - Breaking through blocks
- `think_search` - Cross-thought search

---

## Phase 1: Tool Renaming and Simplification
**Dependencies:** None

### 1.1 Rename Existing Tools
- `convo_think` → `think_convo`
- `tech_think` → `think_tech` (temporary, will split in 1.2)
- `search_thoughts` → `think_search`
- `knowledgegraph_create` → `memories_create`
- `knowledgegraph_search` → `memories_search`
- `knowledgegraph_moderate` → `memories_moderate`

### 1.2 Create Discrete Think Tools
Replace `think_tech` with specialized tools:
```rust
// Each tool has its own defaults and cognitive profile
think_plan {
    injection_scale: 3,  // High context
    significance: 0.7,
    framework: "systems_thinking"
}

think_debug {
    injection_scale: 4,  // Maximum context
    significance: 0.8,
    framework: "root_cause_analysis"
}

think_build {
    injection_scale: 2,  // Focused context
    significance: 0.6,
    framework: "incremental"
}

think_stuck {
    injection_scale: 3,  // Varied context
    significance: 0.9,
    framework: "lateral_thinking"
}
```

### 1.3 Update References
- [ ] Update `server/mod.rs` tool registration
- [ ] Update `call_tool()` routing
- [ ] Update schema definitions
- [ ] Update all test files
- [ ] Update documentation
- [ ] Update MCP client configurations

### Deliverables
- All tools renamed and routing updated
- Tests passing with new names
- Documentation updated

---
***Phase 1 Complete by Junie 20250901 1815. Needed a second look when the embedder was not starting up***
---

## Phase 2: Optimize Current Functionality
**Timeline:** 2–3 days  
**Risk:** Medium  
**Dependencies:** Phase 1 (renames live, embedder stable)

Important constraint
- Injection must use memories only (KG entities/observations), never raw thoughts. Thought retrieval is OK for search, but “injection” attaches only memory IDs.

Detailed implementation plan
1) Enforce KG‑only injection
- Verify `SurrealMindServer::inject_memories` reads from `kg_entities` (optionally `kg_observations`) and writes only IDs into `thoughts.injected_memories` plus `enriched_content` text.
- Remove or guard any legacy thought‑level injection paths; confirm no direct `thoughts` scanning is used for injection.
- Test: create a thought, run injection at scales 1–3; assert injected_memories contains only `kg_entities:*` IDs, count matches scale mapping.

2) Retrieval/search hardening (think_search/search_thoughts)
- Config→env sync at startup: SURR_SIM_THRESH, SURR_TOP_K, SURR_DB_LIMIT, SURR_KG_CANDIDATES.
- Skip and log mismatched embedding rows; include diagnostic counts when RUST_LOG=debug.
- Deterministic sorting (similarity desc; tie-break by created_at desc if available). Clamp offset/top_k.
- Acceptance: a just‑created unique thought is retrieved (similarity ≥ 0.9) by think_search within one call.

3) Tool‑specific runtime defaults (no behavior drift beyond thresholds)
- Defaults when params omitted:
  - think_convo: sim_thresh=0.35, top_k=8, db_limit=500.
  - think_plan:  sim_thresh=0.30, top_k=10, db_limit=800.
  - think_debug: sim_thresh=0.20, top_k=12, db_limit=1000.
  - think_build: sim_thresh=0.45, top_k=6, db_limit=400.
  - think_stuck: sim_thresh=0.30, top_k=10, db_limit=600.
- Implement as handler‑level overrides; do not mutate global config.

4) Proximity tuning for memories injection
- Maintain scale→(limit, prox_thresh): 1→(5,0.8), 2→(10,0.6), 3→(20,0.4).
- Apply submode delta only if `retrieval.submode_tuning=true`; clamp within [0.0, 0.99].

5) Health checks and remediation
- Add/keep `maintenance_ops{subcommand:"health_check_embeddings"}` to report expected_dim vs actual for thoughts, kg_entities, kg_observations.
- Use `reembed` and `reembed_kg` to remediate; ensure provider/model/dim/embedded_at fields updated.

6) Logging & diagnostics
- DEBUG: log query dims, candidate counts, skipped mismatches, top matches, injection results (count & examples).
- ERRORs bubble with plain, actionable messages (DB connect, embedder missing).

7) Tests
- Unit: KG‑only injection, cosine similarity, param coercion for search, per‑tool default selection.
- Integration (local DB): create→search hit; create→think_convo→injection writes only KG IDs; reembed tools idempotent.

8) Docs & operator checklist
- README: explicitly state “injection = memories only,” remove any raw‑thought injection language.
- Checklist: run health_check_embeddings → reembed/reembed_kg if needed → verify.

Out‑of‑scope for Phase 2
- DB‑level vector index/search; cognitive journey/AI dialog engines.

---

## Phase 3: Evolving Cognitive Models
**Timeline:** 1 week  
**Risk:** Medium  
**Dependencies:** Phase 2

Goal
- Add a deterministic Journey engine that guides think_* tools through phases and strategies without requiring an external LLM. Respect strict guardrails: no hidden tool switches, injection remains memories‑only, and everything is observable, reproducible, and easy to disable.

Design constraints
- Deterministic: same inputs → same strategy sequence (seed from content + session).
- Explicit: no silent tool changes; any cross‑tool suggestion is returned as metadata, not auto‑invoked.
- Safe: injection pipeline unchanged (memories only), search behavior unchanged except for per‑step defaults.
- Opt‑in: feature‑flag per tool; global kill switch.

Detailed implementation plan
1) Core types and state
- Add enums: ThinkTool, ThinkPhase { Initial, Deeper, Core, Synthesis }, Strategy (per tool).
- Add Journey struct: { phase, iterations, strategy_idx, seed, started_at }.
- Add SessionState: persisted in DB to survive runs: tables `ci_sessions` (session_id, tool, journey json, created/updated), `ci_turns` (session_id, turn_no, input, output, phase, strategy, metrics).
- Schema: define tables in server.initialize_schema(); add indexes on session_id, created_at.

2) Strategy registry and rotation
- Create a static registry mapping (tool, phase) → Vec<Strategy> with weights.
- Compute a stable sequence via seeded round‑robin: seed = hash(content + session_id + phase); iterate weights deterministically.
- Expose `next_strategy(tool, phase, seed, iter)` that returns Strategy and justification text.

3) Phase progression rules
- Per tool defaults: max_iters_per_phase (e.g., Initial 3, Deeper 5, Core 5, Synthesis 1).
- Advance when success predicate holds (tool‑specific), else rotate strategy up to max, then advance or suggest think_stuck.
- Success predicates examples: debug → “repro or failing locus identified”; plan → “architecture decision produced”. For Phase 3, stub as “non‑empty actionable notes produced”.

4) Orchestration inside handlers (no behavioral drift)
- Add a thin wrapper in each think_* handler:
  - Load/create session state (session_id from request or generated).
  - Pick current strategy using the registry; apply small retrieval default tweaks (e.g., lower sim_thresh for debug) without changing global config.
  - Execute the usual think flow (store thought, run memories injection) with the selected defaults.
  - Record a turn in `ci_turns` with phase, strategy, and summary metrics (memories_injected, tokens_estimate, elapsed_ms).
  - Decide next phase/strategy and return metadata: { phase, strategy, next_suggested_tool? }.
- Never auto‑invoke another tool; only suggest `think_stuck`/`think_plan` via metadata.

5) API and flags
- Request args: optional { session_id, force_phase, max_iters, journey_debug }.
- Env/config:
  - SURR_JOURNEY_ENABLE_{CONVO,PLAN,DEBUG,BUILD,STUCK} (true/false)
  - SURR_JOURNEY_KILL_SWITCH (true disables globally)
  - SURR_JOURNEY_LOG (level for extra logs)
  - Add to surreal_mind.toml under a new [journey] section with sane defaults (disabled by default).

6) Metrics, logging, and observability
- Log at INFO: phase, strategy, iter, memories_injected, result_len.
- Log at DEBUG: seed, strategy sequence preview, thresholds used.
- Provide a maintenance subcommand `journey_stats` to summarize sessions/turns per tool.

7) Tests
- Unit: deterministic rotation (same seed → same order), phase advancement after N iterations, strategy wraparound, metadata formation.
- Integration (local DB): run a short journey for think_debug (2 phases, 3 turns), confirm state persisted and suggestions appear; verify injection remains memories‑only.

8) Documentation
- README: Journey overview, flags, how to disable/enable per tool, and how to read returned metadata.
- Example snippets for think_debug with journey_debug=true showing logs and metadata.

Out‑of‑scope for Phase 3
- Full DialogEngine (Phase 4), advanced success metrics learned over time, and any LLM‑based reasoning.

Deliverables
- Deterministic journey engine integrated with think_* (feature‑flagged, off by default).
- Session persistence (ci_sessions/ci_turns) and maintenance summaries.
- Clear metadata surfaced to the client; no hidden tool switches; memories‑only injection preserved.

---

## Phase 4: Augmented Introspection System
**Timeline:** 2 weeks
**Risk:** High (new system)
**Dependencies:** Phase 3

### 4.1 DialogEngine Core
Build a deterministic dialog partner that doesn't need an LLM:

```rust
pub struct DialogEngine {
    // Pattern detection
    pattern_matcher: PatternMatcher,

    // Socratic question banks
    question_banks: HashMap<ThinkTool, Vec<String>>,

    // Memory context
    memory_retriever: MemoryRetriever,
    recent_thoughts: CircularBuffer<String>,

    // State management
    dialog_state: DialogState,
    frustration_level: f32,
    last_strategies: Vec<Strategy>,

    // Learning
    effectiveness_tracker: HashMap<String, f32>,
}
```

### 4.2 Pattern Detection System
```rust
enum ThoughtPattern {
    Looping,        // Repeating same approach
    Detailed,       // Getting lost in details
    Vague,          // Too high-level
    Scattered,      // Jumping between topics
    Stuck,          // No progress
    Progressing,    // Making headway
}

impl PatternMatcher {
    fn detect(&self, input: &str, history: &[String]) -> ThoughtPattern {
        // Analyze current input against recent history
        // Detect repetition, depth changes, topic shifts
    }
}
```

### 4.3 Socratic Question Banks
```rust
const DEBUG_QUESTIONS: &[&str] = &[
    "What changed recently?",
    "When did it last work?",
    "What's the simplest test case?",
    "Could this be a data issue?",
    "What are we not seeing?",
];

const PLAN_QUESTIONS: &[&str] = &[
    "What could go wrong?",
    "What are we optimizing for?",
    "Who are the stakeholders?",
    "What's the MVP version?",
    "How do we measure success?",
];

const STUCK_QUESTIONS: &[&str] = &[
    "What if we did nothing?",
    "What would Sam do?",
    "Can we break this into smaller pieces?",
    "What similar problem have we solved?",
    "What constraints can we remove?",
];
```

### 4.4 Memory-Driven Prompts
```rust
impl MemoryRetriever {
    fn find_similar_situations(&self, current: &str) -> Vec<MemoryPrompt> {
        // Search for similar past situations
        // Return relevant prompts like:
        // "Last time with a similar issue, you tried X"
        // "Your pattern here is usually Y"
        // "3 weeks ago you solved this with Z"
    }
}
```

### 4.5 Dialog State Machine
```rust
enum DialogState {
    Opening,      // Initial engagement
    Probing,      // Gathering information
    Challenging,  // Questioning assumptions
    Reframing,    // Offering new perspectives
    Synthesizing, // Pulling together
    Reflecting,   // Meta-analysis
}

impl DialogEngine {
    fn transition(&mut self, input: &str) -> DialogState {
        match (self.dialog_state, self.detect_pattern(input)) {
            (Opening, _) => Probing,
            (Probing, Stuck) => Challenging,
            (Probing, Progressing) => Synthesizing,
            (Challenging, Defensive) => Reframing,
            // ... etc
        }
    }
}
```

### Deliverables
- DialogEngine implementation in Rust
- Pattern detection for all think tools
- Socratic question banks
- Memory-driven prompt system
- State machine for natural dialog flow
- Integration with think tools

---

## Phase 5: Integration and Polish
**Timeline:** 1 week
**Risk:** Low
**Dependencies:** Phases 1-4

### 5.1 System Integration
- [ ] Connect DialogEngine to all think tools
- [ ] Implement cross-tool flow control
- [ ] Add telemetry for learning
- [ ] Performance optimization

### 5.2 Testing Suite
- [ ] Unit tests for each component
- [ ] Integration tests for tool interactions
- [ ] Performance benchmarks
- [ ] User experience testing

### 5.3 Documentation
- [ ] API documentation
- [ ] Usage examples
- [ ] Architecture diagrams
- [ ] Configuration guide

### 5.4 Deployment
- [ ] Build release binaries
- [ ] Update MCP configurations
- [ ] Migration guide for existing data
- [ ] Rollback plan

### Deliverables
- Fully integrated system
- Comprehensive test coverage
- Complete documentation
- Production-ready binaries

---

## Success Metrics

### Quantitative
- Memory injection returns >0 for scales 1-3
- Search returns relevant results with >0.3 similarity
- Response time <100ms for DialogEngine
- 90% test coverage

### Qualitative
- Tools feel intuitive to use
- Thinking feels enhanced, not constrained
- Dialog feels natural and helpful
- System adapts to usage patterns

## Risk Mitigation

### Technical Risks
- **Vector search performance:** Fall back to local filtering
- **DialogEngine complexity:** Start simple, iterate
- **Breaking changes:** Maintain compatibility layer

### User Experience Risks
- **Learning curve:** Provide migration guide
- **Tool proliferation:** Clear naming and documentation
- **Dialog annoyance:** Make it optional/configurable

## Timeline Summary

- **Phase 1:** 1-2 days (Tool renaming)
- **Phase 2:** 2-3 days (Optimization)
- **Phase 3:** 1 week (Cognitive models)
- **Phase 4:** 2 weeks (Augmented introspection)
- **Phase 5:** 1 week (Integration)

**Total:** ~4 weeks for complete implementation

## Next Steps

1. Review and approve proposal
2. Begin Phase 1 implementation
3. Set up testing environment
4. Create rollback plan

---

## Appendix: Example Interactions

### Current (Confusing)
```
> tech_think --submode=debug "Parser failing on empty input"
```

### Proposed (Clear)
```
> think_debug "Parser failing on empty input"

DialogEngine: "What error are you seeing?"
User: "Panic at line 42"
DialogEngine: "When did this start happening?"
User: "After the refactor yesterday"
DialogEngine: "What changed in the refactor? Let's check line 42..."
```

### Cognitive Evolution Example
```
think_plan "Design module for user auth"
Phase: Initial → Systems thinking
Phase: Deeper → First principles (What is auth really?)
Phase: Core → Risk analysis (What could go wrong?)
Phase: Synthesis → Architecture decision
```

---

*This proposal represents a fundamental shift from static tools to adaptive, intelligent thinking partners that evolve with use.*

---

## Codex Commentary (2025-09-01)

High‑level take
- Keep Phase 1 focused: rename + alias + tests; no behavioral drift. Expose think_* early so we can tune each mode independently.
- Make inner_voice the sole RAG+synthesis tool; think_* remains storage + dialog guidance. Cross‑tool handoff is explicit.
- Injected memories are KG entities attached to thoughts (ids + enriched summary) — keep this definition visible in help/README.

Operational guardrails
- Embedder alignment first: re‑embed all thoughts to a single model/dimension (or set SURR_EMBED_PROVIDER=fake for dev). RAG, search, and injection all depend on matching dims.
- KG search filters: implement deterministic WHERE building (name regex/contains, entity_type|data.entity_type, rel_type). Never leave dangling WHERE.

Journey engine inside think_* (Phase 3 scope, but design now)
- Deterministic rotation: select strategy by hash(content + session_id + phase + tries) with small weight biases; keep behavior stable per session.
- State per session: ci_sessions (phase, frustration, tries, last_strategy), ci_turns (input, output, pattern, strategy, used_memories).
- Memory hook is optional: pull top_k_mem via search_thoughts; do not mutate DB.
- Escalation: after N tries or frustration>τ, suggest next_tool (think_stuck/think_plan). No silent tool switching.

Testing priorities
- Unit: pattern fixtures, phase transitions, rotation determinism, WHERE‑builder for KG search.
- Integration: end‑to‑end inner_voice RAG (finds recent thoughts), injection counts (0/5/10/20) with a seeded KG, and moderation flow from candidates → approved.

Docs & UX
- Tool help: declare new think_* names; keep legacy aliases noted as deprecated.
- Clarify concepts in detailed_help: injected memories (entities), inner_voice behavior (RAG + optional staging + summary save).

Risks & mitigations
- Mixed embeddings → empty RAG/search/injection: mitigate by reembed tool and a health check (GROUP BY array::len(embedding)).
- Rule creep in extractor → mis‑typed entities: pin a small dictionary (Federation→org; convo_think→tool; SurrealDB→database; Codex/Claude/Gemini/Warp→product/vendor) and log extractions with confidence.

Rollout suggestion
- Phase 1: ship rename + aliases; CI green.
- Phase 2: fix KG search filters + reembed; verify RAG/injection live.
- Phase 3: add journey engine to think_debug first (feature‑flag), then expand.

Session ID: 75f031c3-1c4e-4a3a-9587-d76061fd905d

---

## Gemini Commentary (2025-08-31)

**Big Picture**
- The ambition is admirable, moving from simple storage to a cognitive partner. The phased approach is logical, but the 4-week timeline is... let's call it "aspirational." This is a ground-up rebuild of the system's core identity.
- The distinction between `think_*` (process) and `memories_*` (KG/state) is a crucial and welcome clarification. It separates the act of thinking from the act of remembering.
- The `DialogEngine` is the riskiest and most valuable part of this. A deterministic, non-LLM partner that can challenge assumptions without hallucinating is the holy grail. However, if not done carefully, it's just a more complicated Clippy.

**Implementation Realities**
- **The Annoyance Factor:** The `DialogEngine`'s success hinges on its ability to be insightful, not just interrogative. The `frustration_level` metric is hilarious—it will likely be tracking Sam's frustration with the engine itself. The escape hatch (`"Sam, I need help"`) is the most critical feature in the entire proposal; it's a built-in admission of fallibility.
- **Journeys vs. Reality:** The `ThinkJourney` concept is brilliant but abstract. "Success" needs a concrete definition. Does it mean a solution was found? Or just that the conversation ended? Tying journey progression to tangible outcomes (e.g., code was generated, a test passed, a commit was made) would be more meaningful than "iterations_in_phase."
- **Pattern Matching is Hard:** The `PatternMatcher` is non-trivial. Distinguishing between "Looping" and "Deeper" requires more than just string comparison; it requires semantic understanding of the *intent* behind the thoughts, which is precisely what LLMs are for. Faking it deterministically will be a challenge.

**Priorities & Recommendations**
1.  **Nail Phase 1 & 2 First:** Don't even think about the cognitive stuff until the foundation is rock-solid. A clean, well-documented, and fully tested set of basic tools is non-negotiable. Echoing Codex: mismatched embeddings will kill this project before it starts. A mandatory re-embedding and a pre-flight check are essential.
2.  **Prototype `DialogEngine` with `think_stuck`:** This is the perfect testbed. It's a constrained problem space ("I am stuck") where Socratic questioning is most natural. If the engine can be genuinely helpful here, the pattern can be expanded. If it's annoying here, it will be unbearable everywhere else.
3.  **Make Dialog Optional:** The `DialogEngine` should be a feature that can be toggled on or off per-tool or globally. There will be times when we just want to save a thought without being cross-examined.
4.  **Focus the `MemoryRetriever`:** Instead of just finding "similar situations," it should find *successful* past situations. The goal isn't just to remember, it's to remember what *worked*.

**Final Take**
This is the right direction. It's a move from a simple notebook to a proper intellectual sparring partner. But let's not kid ourselves about the timeline. This is a multi-month epic, not a sprint. Let's build it right, starting with the unglamorous work of getting the basics perfect before we try to build a soul in Rust.

Session ID: 2a7e8f5c-9b3d-4c1a-8e6f-0b9d7a5c3b2d
