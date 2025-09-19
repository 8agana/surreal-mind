//! Edge-case tests for inner_voice providers and planner parsing

use surreal_mind::tools::inner_voice::providers::allow_grok_from;
use surreal_mind::tools::inner_voice::parse_planner_json;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planner_parse_invalid_json_returns_err() {
        let bad = "{ not_json: true }";
        let res = parse_planner_json(bad);
        assert!(res.is_err());
    }

    #[test]
    fn test_planner_parse_empty_rewritten_query_is_err() {
        let empty_rq = r#"{
            "rewritten_query": "",
            "mix": 0.6
        }"#;
        let res = parse_planner_json(empty_rq);
        assert!(res.is_err());
    }

    #[test]
    fn test_allow_grok_from_false_gates() {
        assert!(!allow_grok_from(Some("false")));
        assert!(allow_grok_from(None)); // default true
    }
}
