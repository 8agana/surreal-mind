# Phase B — Thought Continuity Links (Safe, Additive)

Owner: Codex • Date: 2025-09-07

Goal
- Persist lightweight continuity for thoughts (session/linkage/confidence) without changing retrieval (KG-only) or tool surface beyond optional args.

Scope
- Thoughts gain optional fields: session_id, chain_id, previous_thought_id, revises_thought, branch_from, confidence.
- legacymind_think accepts/returns these; writes them when provided.
- No changes to injection or search behavior.

Schema (SurrealDB)
- DEFINE FIELD session_id         ON TABLE thoughts TYPE string NULL;
- DEFINE FIELD chain_id           ON TABLE thoughts TYPE string NULL;
- DEFINE FIELD previous_thought_id ON TABLE thoughts TYPE record(thoughts) | string NULL;
- DEFINE FIELD revises_thought     ON TABLE thoughts TYPE record(thoughts) | string NULL;
- DEFINE FIELD branch_from         ON TABLE thoughts TYPE record(thoughts) | string NULL;
- DEFINE FIELD confidence          ON TABLE thoughts TYPE float NULL;
- DEFINE INDEX idx_thoughts_session ON TABLE thoughts FIELDS session_id, created_at;
- DEFINE INDEX idx_thoughts_chain   ON TABLE thoughts FIELDS chain_id, created_at;

Migration/Bootstrap
- Add maintenance_ops subcommand: {"subcommand": "ensure_continuity_fields"} that executes the DEFINE FIELD/INDEX statements. Idempotent.
- Optionally provide a one-off bin `migrate_continuity_links.rs` that calls the same SQL.

Tool/API Changes (legacymind_think)
- Args (all optional, additive): session_id, chain_id, previous_thought_id, revises_thought, branch_from, confidence (0..1).
- Result (additive): links { session_id?, chain_id?, previous_thought_id?, revises_thought?, branch_from?, confidence? }.
- Behavior: if link ids resolve to an existing thought, store as record(thoughts); otherwise store string as-is.

Validation Rules
- Clamp confidence to [0.0, 1.0].
- Reject self-link (new_thought_id equals any provided link id); drop silently with note in result.telemetry.
- If multiple link fields reference the same id, keep first and remove duplicates.

Testing
- Unit: args parsing, confidence clamp, self-link rejection, duplicate link handling.
- Integration (gated): ensure fields exist; write a thought with session_id + previous_thought_id; fetch and verify; confirm indexes usable via query by session_id with ORDER BY created_at.

Acceptance Criteria
- legacymind_think accepts optional continuity args and echoes them in result.links.
- New thoughts persist link fields when provided.
- tools/list unchanged; inner_voice and knowledgegraph_* unaffected.
- Running ensure_continuity_fields twice causes no errors (idempotent).

Rollout
- Land maintenance_ops subcommand and legacymind_think arg/result updates.
- Add brief README note documenting optional continuity args.
- No feature flags required.

Notes
- This phase does not inject raw thoughts; KG-only retrieval remains in place.
- All fields are optional; existing data remains valid without backfill.

