//! Tests for provider gating and fallback helpers in inner_voice

use surreal_mind::schemas::Snippet;
use surreal_mind::tools::inner_voice::InnerVoiceRetrieveParams;
use surreal_mind::tools::inner_voice::providers::{
    allow_grok, compute_auto_extract, fallback_from_snippets,
};

#[cfg(test)]
mod tests {
    use super::*;

    // Re-import helpers via fully-qualified path since they are nested in the module
    // paths below are stable because they live inside inner_voice.rs
    #[test]
    fn test_allow_grok_default_true() {
        // Default behavior should allow Grok unless explicitly disabled
        assert!(allow_grok());
    }

    #[test]
    fn test_fallback_from_snippets_zero() {
        let snippets: Vec<Snippet> = vec![];
        let s = fallback_from_snippets(&snippets);
        assert!(s.starts_with("Based on what I could find"));
    }

    #[test]
    fn test_compute_auto_extract_independent_of_mix() {
        let params = InnerVoiceRetrieveParams {
            query: "q".into(),
            top_k: None,
            floor: None,
            mix: Some(0.0),
            include_private: None,
            include_tags: vec![],
            exclude_tags: vec![],
            auto_extract_to_kg: None,
            previous_thought_id: None,
            include_feedback: None,
            feedback_max_lines: None,
            recency_days: None,
            prefer_recent: None,
        };
        // default true => result true even with mix=0
        assert!(compute_auto_extract(params.auto_extract_to_kg, true));
        // default false => result false
        assert!(!compute_auto_extract(params.auto_extract_to_kg, false));
    }
}
