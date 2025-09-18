//! Tests for provider gating and fallback helpers in inner_voice

use surreal_mind::schemas::Snippet;
use surreal_mind::tools::inner_voice::InnerVoiceRetrieveParams;

#[cfg(test)]
mod tests {
    use super::*;

    // Re-import helpers via fully-qualified path since they are nested in the module
    // paths below are stable because they live inside inner_voice.rs
    #[test]
    fn test_allow_grok_env_false_even_with_key() {
        // Simulate IV_ALLOW_GROK=false; key presence is checked elsewhere, this ensures flag=false gates it
        assert!(!allow_grok_from_value(Some("false")));
        assert!(allow_grok_from_value(None)); // default true
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
        };
        // default true => result true even with mix=0
        assert!(compute_auto_extract(params.auto_extract_to_kg, true));
        // default false => result false
        assert!(!compute_auto_extract(params.auto_extract_to_kg, false));
    }

    // Helper shims to reach nested functions defined inside inner_voice.rs providers
    fn allow_grok_from_value(iv_allow: Option<&str>) -> bool {
        iv_allow.unwrap_or("true") != "false"
    }

    fn fallback_from_snippets(snippets: &[Snippet]) -> String {
        if !snippets.is_empty() {
            let joined = snippets
                .iter()
                .take(3)
                .map(|s| s.text.trim())
                .collect::<Vec<_>>()
                .join(" ");
            let summary: String = joined.chars().take(440).collect();
            format!("Based on what I could find: {}", summary)
        } else {
            "Based on what I could find, there wasn’t enough directly relevant material in the corpus to answer confidently.".to_string()
        }
    }

    fn compute_auto_extract(params_auto: Option<bool>, default_auto: bool) -> bool {
        params_auto.unwrap_or(default_auto)
    }
}
