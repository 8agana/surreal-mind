# Integrated runner source review

Candidate: Studio `surreal-mind-wt-fed6d00e5`, branch
`fed-6d00e5/tool-free-wander`, uncommitted changes on `359aa18`.

Independent Sol review: PASS after one repair. Initial blocker: the optional
Antigravity runner also overrode Gemini provider selection. The repair rejects
Gemini plus a configured runner before server initialization, resolves the
runner setting once, and preserves direct behavior when unset.

Parent independently ran `cargo test --bin kg_wander decision_runner`:
one selected test passed, four filtered out. It covers Gemini rejection,
Antigravity acceptance, and unset preservation. Source hash matched reviewer:
`77be1ebb15d1479a066d2a78c1c66f5a9431390527e85e94095e13f8315a4b12`.

Reviewed boundaries: Python parses structured output and validates action
parameters; unexpected tool events are rejected after observation. Agent
configuration supplies the tool restriction. Rust supervises the process group
and output; group escape remains outside its cleanup guarantee. Existing KG
action errors can still be logged without a failing process exit.

This review does not establish a successful live model/KG invocation. Acceptance
still needs the actual subscription runner and selected model, no unexpected
tool event, a bounded KG action/end-state witness, and deployment provenance.
No provider or DB call, production build, merge, or deployment in this review.
