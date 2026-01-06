//! Mode routing for legacymind_think
//!
//! This module handles the resolution of thinking mode from hints and content,
//! along with generation of routing metadata (reason, trigger, heuristics).

use super::mode_detection::{detect_mode, DEBUG_KEYWORDS, BUILD_KEYWORDS, PLAN_KEYWORDS, STUCK_KEYWORDS};
use super::types::ThinkMode;

/// Metadata about how a mode was selected
#[derive(Debug, Clone)]
pub struct ModeRoutingResult {
    pub mode: ThinkMode,
    pub mode_selected: String,
    pub reason: String,
    pub trigger_matched: Option<String>,
    pub heuristics: Option<(Vec<String>, usize)>,
}

/// Route to the appropriate thinking mode based on hint and content.
///
/// Priority:
/// 1. Explicit hint (if matches known mode)
/// 2. Trigger phrases ("debug time", "building time", etc.)
/// 3. Keyword heuristics (from mode_detection)
///
/// Returns metadata about how the mode was selected for transparency.
pub fn route_mode(hint: Option<&str>, content: &str) -> ModeRoutingResult {
    let content_lower = content.to_lowercase();
    
    // First: determine the mode
    let mode = if let Some(h) = hint {
        match h {
            "debug" => ThinkMode::Debug,
            "build" => ThinkMode::Build,
            "plan" => ThinkMode::Plan,
            "stuck" => ThinkMode::Stuck,
            "question" => ThinkMode::Question,
            "conclude" => ThinkMode::Conclude,
            _ => detect_mode(content),
        }
    } else if content_lower.contains("debug time") {
        ThinkMode::Debug
    } else if content_lower.contains("building time") {
        ThinkMode::Build
    } else if content_lower.contains("plan time") || content_lower.contains("planning time") {
        ThinkMode::Plan
    } else if content_lower.contains("i'm stuck") || content_lower.contains("stuck") {
        ThinkMode::Stuck
    } else if content_lower.contains("question time") {
        ThinkMode::Question
    } else if content_lower.contains("wrap up") || content_lower.contains("conclude") {
        ThinkMode::Conclude
    } else {
        detect_mode(content)
    };
    
    // Second: generate metadata about how the mode was selected
    let (mode_selected, reason, trigger_matched, heuristics) = generate_metadata(
        &mode, 
        hint, 
        &content_lower
    );
    
    ModeRoutingResult {
        mode,
        mode_selected,
        reason,
        trigger_matched,
        heuristics,
    }
}

/// Generate metadata explaining how a mode was selected
fn generate_metadata(
    mode: &ThinkMode,
    hint: Option<&str>,
    content_lower: &str,
) -> (String, String, Option<String>, Option<(Vec<String>, usize)>) {
    let mode_name = mode.as_str();
    
    // Check if hint was used
    if let Some(h) = hint {
        if h == mode_name {
            return (mode_name.to_string(), "hint specified".to_string(), None, None);
        }
    }
    
    // Check trigger phrases
    let trigger = match mode {
        ThinkMode::Debug if content_lower.contains("debug time") => 
            Some("debug time".to_string()),
        ThinkMode::Build if content_lower.contains("building time") => 
            Some("building time".to_string()),
        ThinkMode::Plan if content_lower.contains("plan time") || content_lower.contains("planning time") => 
            Some("plan/planning time".to_string()),
        ThinkMode::Stuck if content_lower.contains("stuck") => 
            Some("stuck".to_string()),
        ThinkMode::Question if content_lower.contains("question time") => 
            Some("question time".to_string()),
        ThinkMode::Conclude if content_lower.contains("wrap up") || content_lower.contains("conclude") => 
            Some("wrap up/conclude".to_string()),
        _ => None,
    };
    
    if let Some(t) = trigger {
        return (mode_name.to_string(), "trigger phrase".to_string(), Some(t), None);
    }
    
    // Check if hint caused heuristic override
    if let Some(h) = hint {
        return (
            mode_name.to_string(), 
            format!("heuristic override from hint {}", h),
            None,
            None
        );
    }
    
    // Must be keyword heuristics
    let keywords = match mode {
        ThinkMode::Debug => keyword_match(DEBUG_KEYWORDS, content_lower),
        ThinkMode::Build => keyword_match(BUILD_KEYWORDS, content_lower),
        ThinkMode::Plan => keyword_match(PLAN_KEYWORDS, content_lower),
        ThinkMode::Stuck => keyword_match(STUCK_KEYWORDS, content_lower),
        ThinkMode::Question => (vec![], 0),
        ThinkMode::Conclude => (vec![], 0),
    };
    
    if keywords.1 > 0 {
        (mode_name.to_string(), "heuristic keyword match".to_string(), None, Some(keywords))
    } else {
        (mode_name.to_string(), "default for general content".to_string(), None, None)
    }
}

/// Find matching keywords in content
fn keyword_match(keywords: &[&str], content_lower: &str) -> (Vec<String>, usize) {
    let matched: Vec<String> = keywords
        .iter()
        .filter(|k| content_lower.contains(*k))
        .map(|s| s.to_string())
        .collect();
    let score = matched.len();
    (matched, score)
}

impl ThinkMode {
    /// Get string representation for metadata
    pub fn as_str(&self) -> &'static str {
        match self {
            ThinkMode::Debug => "debug",
            ThinkMode::Build => "build",
            ThinkMode::Plan => "plan",
            ThinkMode::Stuck => "stuck",
            ThinkMode::Question => "question",
            ThinkMode::Conclude => "conclude",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_with_explicit_hint() {
        let result = route_mode(Some("debug"), "some random content");
        assert_eq!(result.mode, ThinkMode::Debug);
        assert_eq!(result.reason, "hint specified");
    }

    #[test]
    fn test_route_with_trigger_phrase() {
        let result = route_mode(None, "debug time: let's figure this out");
        assert_eq!(result.mode, ThinkMode::Debug);
        assert_eq!(result.trigger_matched, Some("debug time".to_string()));
    }

    #[test]
    fn test_route_with_heuristics() {
        let result = route_mode(None, "There's an error in the stack trace");
        assert_eq!(result.mode, ThinkMode::Debug);
        assert!(result.heuristics.is_some());
    }

    #[test]
    fn test_route_question_default() {
        let result = route_mode(None, "What is the meaning of life?");
        assert_eq!(result.mode, ThinkMode::Question);
    }
}
