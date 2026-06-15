pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod gemini;
pub mod google_cli;
pub mod traits;
pub mod vibe;

pub use antigravity::{AntigravityClient, AntigravityPermissionMode};
pub use claude::ClaudeClient;
pub use codex::CodexClient;
pub use gemini::GeminiClient;
pub use google_cli::GoogleCliProvider;
pub use traits::{AgentError, AgentResponse, CognitiveAgent};
