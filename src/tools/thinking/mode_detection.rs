//! Mode detection heuristics for legacymind_think
//!
//! This module contains the logic for detecting thinking mode from content
//! when no explicit hint is provided. Uses keyword matching heuristics.

use super::types::ThinkMode;

/// Keywords that indicate debug mode (error investigation, troubleshooting)
pub const DEBUG_KEYWORDS: &[&str] = &[
    "error",
    "bug",
    "stack trace",
    "failed",
    "exception",
    "panic",
];

/// Keywords that indicate build mode (implementation, construction)
pub const BUILD_KEYWORDS: &[&str] = &[
    "implement",
    "create",
    "add function",
    "build",
    "scaffold",
    "wire",
];

/// Keywords that indicate plan mode (architecture, design)
pub const PLAN_KEYWORDS: &[&str] = &[
    "architecture",
    "design",
    "approach",
    "how should",
    "strategy",
    "trade-off",
];

/// Keywords that indicate stuck mode (confusion, blockage)
pub const STUCK_KEYWORDS: &[&str] = &[
    "stuck",
    "unsure",
    "confused",
    "not sure",
    "blocked",
];

/// Detect thinking mode from content using keyword heuristics.
///
/// Scans the content for mode-specific keywords and returns the mode
/// with the highest keyword match count. Returns `ThinkMode::Question`
/// if no keywords match.
///
/// # Arguments
/// * `content` - The thought content to analyze
///
/// # Returns
/// The detected `ThinkMode` based on keyword matching
///
/// # Examples
/// ```ignore
/// let mode = detect_mode("I'm getting a stack trace error");
/// assert_eq!(mode, ThinkMode::Debug);
///
/// let mode = detect_mode("How should we approach this architecture?");
/// assert_eq!(mode, ThinkMode::Plan);
/// ```
pub fn detect_mode(content: &str) -> ThinkMode {
    let content_lower = content.to_lowercase();
    
    let keyword_sets = [
        ("debug", DEBUG_KEYWORDS),
        ("build", BUILD_KEYWORDS),
        ("plan", PLAN_KEYWORDS),
        ("stuck", STUCK_KEYWORDS),
    ];
    
    let mut best_mode = "question";
    let mut best_score = 0;
    
    for (mode, keywords) in keyword_sets.iter() {
        let score = keywords.iter().filter(|k| content_lower.contains(*k)).count();
        if score > best_score {
            best_score = score;
            best_mode = mode;
        }
    }
    
    if best_score == 0 {
        ThinkMode::Question
    } else {
        match best_mode {
            "debug" => ThinkMode::Debug,
            "build" => ThinkMode::Build,
            "plan" => ThinkMode::Plan,
            "stuck" => ThinkMode::Stuck,
            _ => ThinkMode::Question,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_debug_mode() {
        assert_eq!(detect_mode("I'm getting an error when compiling"), ThinkMode::Debug);
        assert_eq!(detect_mode("There's a bug in the parser"), ThinkMode::Debug);
        assert_eq!(detect_mode("The function panic on invalid input"), ThinkMode::Debug);
    }

    #[test]
    fn test_detect_build_mode() {
        assert_eq!(detect_mode("I need to implement a new feature"), ThinkMode::Build);
        assert_eq!(detect_mode("Let's create a helper function"), ThinkMode::Build);
        assert_eq!(detect_mode("We should scaffold the module"), ThinkMode::Build);
    }

    #[test]
    fn test_detect_plan_mode() {
        assert_eq!(detect_mode("What's the best architecture for this?"), ThinkMode::Plan);
        assert_eq!(detect_mode("How should we design the API?"), ThinkMode::Plan);
        assert_eq!(detect_mode("Let's discuss the strategy"), ThinkMode::Plan);
    }

    #[test]
    fn test_detect_stuck_mode() {
        assert_eq!(detect_mode("I'm stuck on this problem"), ThinkMode::Stuck);
        assert_eq!(detect_mode("I'm not sure how to proceed"), ThinkMode::Stuck);
        assert_eq!(detect_mode("Feeling confused and unsure"), ThinkMode::Stuck);
    }

    #[test]
    fn test_detect_question_mode_fallback() {
        assert_eq!(detect_mode("What is the meaning of life?"), ThinkMode::Question);
        assert_eq!(detect_mode("Random thoughts about coding"), ThinkMode::Question);
        assert_eq!(detect_mode(""), ThinkMode::Question);
    }

    #[test]
    fn test_highest_score_wins() {
        // Multiple keywords for debug should beat single keyword for build
        assert_eq!(
            detect_mode("There's an error, a bug, and the function failed"),
            ThinkMode::Debug
        );
    }
}
