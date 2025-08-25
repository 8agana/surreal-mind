//! Types for cognitive framework outputs.

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FrameworkOutput {
    pub insights: Vec<String>,
    pub questions: Vec<String>,
    pub next_steps: Vec<String>,
    // Optional metadata bag for framework-specific notes.
    // Keep values stringly-typed to avoid early coupling.
    pub meta: std::collections::HashMap<String, String>,
}
