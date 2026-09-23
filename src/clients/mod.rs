pub mod antigravity;
pub mod codex;
pub mod gemini;
pub mod google_cli;
pub mod traits;

pub use antigravity::{AntigravityClient, AntigravityPermissionMode};
// CodexClient: retained, not orphaned by omission.
//
// fed-0a2109 review finding 7 / fed-61d26d audited this export because the
// active router wires no call_* tools to it and the extraction-provider
// enum only dispatches Gemini/Antigravity. Audit result (fed-61d26d,
// 2026-09-16): no internal execution reference exists beyond this
// declaration/impl/re-export (verified against a known-consumed sibling as
// a control); the crate is not published on crates.io; no other repository
// on the searched hosts (Studio, MBP) depends on it via path or git.
//
// It is kept anyway because fed-734b8f (the call_codex tool removal)
// explicitly decided to retain the underlying CognitiveAgent implementation
// "for potential future use" rather than delete it — see CHANGELOG.md and
// docs/tasks/complete/20260124-remove-call_codex-plan-v2.md. That prior
// decision is a real, documented purpose, not an oversight, so this audit
// did not remove it.
//
// Unresolved compatibility reason for NOT removing outright: this repo is
// public on GitHub without a crates.io listing, so a git-dependency
// consumer outside the audited filesystem universe cannot be ruled out from
// here. Remaining retirement work if this is revisited: there is no test
// coverage for CodexClient (unlike GeminiClient/AntigravityClient, which
// have integration tests) and no concrete current caller — if "potential
// future use" has not materialized by the time this is next reviewed, that
// absence of both a test and a caller is the strongest evidence for
// removal.
pub use codex::CodexClient;
pub use gemini::GeminiClient;
pub use google_cli::GoogleCliProvider;
pub use traits::{AgentError, AgentResponse, CognitiveAgent};
