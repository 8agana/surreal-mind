# Code Conventions: SurrealMind

## General Standards
- **Language:** Rust 2024.
- **Linting:** `cargo clippy` must pass with zero warnings (warnings treated as errors).
- **Formatting:** `cargo fmt` mandatory.
- **Error Handling:** Use custom error types (see `src/error.rs`).

## Architectural Rules
- **Lobotomy:** No business logic (photography, etc.) in this repo. It must remain purely cognitive.
- **Structured Thinking:** All major AI thoughts must pass through `legacymind_think` to be graphed.
- **Deterministic Cognition:** The `src/cognitive/` module uses static analysis (regex/keywords), not LLM calls, for speed.
- **Embedding Hygiene:** Strictly use 1536 dimensions. Reembed if providers or models change.

## Naming & Style
- Mimic existing idiomatic Rust in the codebase.
- Keep files modular and focused.
- Documentation should focus on "why" rather than "what".
