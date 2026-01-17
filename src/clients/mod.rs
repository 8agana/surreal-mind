pub mod claude;
pub mod codex;
pub mod gemini;
pub mod traits;

pub use claude::ClaudeClient;
pub use codex::CodexClient;
pub use gemini::GeminiClient;
pub use traits::{AgentError, AgentResponse, CognitiveAgent};
