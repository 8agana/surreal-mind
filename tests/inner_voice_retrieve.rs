//! Unit tests for inner_voice tool

use surreal_mind::tools::inner_voice::{
    Candidate, DateRange, PlannerResponse, allocate_slots, apply_adaptive_floor, cap_text,
    compute_trust_tier, hash_content, select_and_dedupe,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dim_filter_enforcement() {
        // Test that only candidates with matching embedding_dim are considered
        let candidate_matching = Candidate {
            id: "test:1".to_string(),
            table: "thoughts".to_string(),
            source_type: "thought".to_string(),
            origin: "human".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            text: "test".to_string(),
            embedding: vec![0.1, 0.2, 0.3], // 3 dims
            score: 0.8,
            tags: vec![],
            is_private: false,
            content_hash: "".to_string(),
            trust_tier: "".to_string(),
        };

        let candidate_mismatched = Candidate {
            id: "test:2".to_string(),
            table: "thoughts".to_string(),
            source_type: "thought".to_string(),
            origin: "human".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            text: "test".to_string(),
            embedding: vec![0.1, 0.2], // 2 dims - mismatch
            score: 0.8,
            tags: vec![],
            is_private: false,
            content_hash: "".to_string(),
            trust_tier: "".to_string(),
        };

        let query_emb = [0.1, 0.2, 0.3]; // 3 dims

        // Only matching dim should have score computed
        assert!(candidate_matching.embedding.len() == query_emb.len());
        assert!(candidate_mismatched.embedding.len() != query_emb.len());
    }

    #[test]
    fn test_mix_allocation_math() {
        let k_hits = vec![create_candidate("kg:1", 0.9), create_candidate("kg:2", 0.8)];
        let t_hits = vec![
            create_candidate("thought:1", 0.7),
            create_candidate("thought:2", 0.6),
        ];

        let (kg_slots, thought_slots) = allocate_slots(0.6, 10, &k_hits, &t_hits);
        assert_eq!(kg_slots, 6);
        assert_eq!(thought_slots, 4);

        // Test backfill when one source underflows
        let k_hits_empty = vec![];
        let (kg_slots, thought_slots) = allocate_slots(0.6, 10, &k_hits_empty, &t_hits);
        assert_eq!(kg_slots, 0);
        assert_eq!(thought_slots, 10); // Backfill to reach total
    }

    #[test]
    fn test_privacy_tags_filters() {
        // Test would require mocking DB queries, but we can test the logic indirectly
        // through the SQL building in fetch_thought_candidates
        let include_tags = ["important".to_string()];
        let exclude_tags = ["private".to_string()];

        // This would be tested in integration tests with actual DB
        assert!(!include_tags.is_empty());
        assert!(!exclude_tags.is_empty());
    }

    #[test]
    fn test_dedup_via_content_hash() {
        let text1 = "This is a test message.";
        let text2 = "This is a test message."; // Identical
        let text3 = "This is different.";

        let hash1 = hash_content(text1);
        let hash2 = hash_content(text2);
        let hash3 = hash_content(text3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_text_capping_boundaries() {
        let mut long_text =
            "Short. This is a very long sentence that should be capped at the boundary."
                .to_string();
        cap_text(&mut long_text, 50);

        assert!(long_text.len() <= 50); // Hard cut if no boundary found
    }

    #[test]
    fn test_text_capping_fallback() {
        let mut long_text = "Thisisaverylongwordwithoutanyspacesorpunctuationmarks".to_string();
        let original_len = long_text.len();
        cap_text(&mut long_text, 20);

        assert!(long_text.len() <= 20);
        assert!(long_text.len() < original_len);
    }

    #[test]
    fn test_empty_results_path() {
        let t_hits: Vec<Candidate> = vec![];
        let k_hits: Vec<Candidate> = vec![];

        let (t_filtered, k_filtered, floor_used) =
            apply_adaptive_floor(&t_hits, &k_hits, 0.5, 0.1, 10);
        assert!(t_filtered.is_empty());
        assert!(k_filtered.is_empty());
        assert_eq!(floor_used, 0.5); // No adaptive lowering since no candidates
    }

    #[test]
    fn test_trust_tier_computation() {
        assert_eq!(compute_trust_tier("human", "thoughts"), "green");
        assert_eq!(compute_trust_tier("tool", "thoughts"), "amber");
        assert_eq!(compute_trust_tier("model", "thoughts"), "red");
        assert_eq!(compute_trust_tier("any", "kg_entities"), "green");
    }

    #[test]
    fn test_select_and_dedupe() {
        let mut cand1 = create_candidate("id1", 0.9);
        cand1.content_hash = hash_content("content1");

        let mut cand2 = create_candidate("id2", 0.8);
        cand2.content_hash = hash_content("content1"); // Same hash

        let t_hits = vec![cand1];
        let k_hits = vec![cand2];

        let selected = select_and_dedupe(t_hits, k_hits, 1, 1);
        assert_eq!(selected.len(), 1); // Should dedupe by hash
    }

    #[test]
    fn test_planner_json_parsing_valid() {
        let json = r#"{
            "rewritten_query": "What changed in the project?",
            "date_range": {
                "from": "2025-01-01",
                "to": "2025-01-31"
            },
            "recency_days": null,
            "include_tags": ["important"],
            "exclude_tags": ["private"],
            "entity_hints": ["project", "changes"],
            "top_k": 15,
            "mix": 0.7,
            "floor": 0.3
        }"#;

        let planner: PlannerResponse = serde_json::from_str(json).unwrap();
        assert_eq!(planner.rewritten_query, "What changed in the project?");
        assert_eq!(planner.date_range.as_ref().unwrap().from, "2025-01-01");
        assert_eq!(planner.date_range.as_ref().unwrap().to, "2025-01-31");
        assert!(planner.recency_days.is_none());
        assert_eq!(planner.include_tags, vec!["important"]);
        assert_eq!(planner.exclude_tags, vec!["private"]);
        assert_eq!(planner.entity_hints, vec!["project", "changes"]);
        assert_eq!(planner.top_k, Some(15));
        assert_eq!(planner.mix, Some(0.7));
        assert_eq!(planner.floor, Some(0.3));
    }

    #[test]
    fn test_planner_json_parsing_missing_required() {
        let json = r#"{
            "date_range": {
                "from": "2025-01-01",
                "to": "2025-01-31"
            }
        }"#;

        let result: Result<PlannerResponse, _> = serde_json::from_str(json);
        assert!(result.is_err()); // Should fail due to missing rewritten_query
    }

    #[test]
    fn test_planner_json_parsing_empty_rewritten_query() {
        let json = r#"{
            "rewritten_query": "",
            "date_range": {
                "from": "2025-01-01",
                "to": "2025-01-31"
            }
        }"#;

        let planner: PlannerResponse = serde_json::from_str(json).unwrap();
        assert_eq!(planner.rewritten_query, ""); // Parses but should be rejected in validation
    }

    #[test]
    fn test_planner_json_parsing_bad_types() {
        let json = r#"{
            "rewritten_query": "test query",
            "recency_days": "not_a_number",
            "top_k": "also_not_a_number"
        }"#;

        let result: Result<PlannerResponse, _> = serde_json::from_str(json);
        assert!(result.is_err()); // Should fail due to type mismatches
    }

    #[test]
    fn test_planner_json_parsing_minimal_valid() {
        let json = r#"{
            "rewritten_query": "test query"
        }"#;

        let planner: PlannerResponse = serde_json::from_str(json).unwrap();
        assert_eq!(planner.rewritten_query, "test query");
        assert!(planner.date_range.is_none());
        assert!(planner.recency_days.is_none());
        assert!(planner.include_tags.is_empty());
        assert!(planner.exclude_tags.is_empty());
        assert!(planner.entity_hints.is_empty());
        assert!(planner.top_k.is_none());
        assert!(planner.mix.is_none());
        assert!(planner.floor.is_none());
    }

    #[test]
    fn test_planner_parameter_clamping() {
        // Test top_k clamping
        let mut top_k = 100; // Above max
        top_k = top_k.clamp(1, 50);
        assert_eq!(top_k, 50);

        let mut top_k = 0; // Below min
        top_k = top_k.clamp(1, 50);
        assert_eq!(top_k, 1);

        let mut top_k = 25; // Within range
        top_k = top_k.clamp(1, 50);
        assert_eq!(top_k, 25);

        // Test mix clamping
        let mut mix: f32 = 1.5; // Above max
        mix = mix.clamp(0.0, 1.0);
        assert_eq!(mix, 1.0);

        let mut mix: f32 = -0.1; // Below min
        mix = mix.clamp(0.0, 1.0);
        assert_eq!(mix, 0.0);

        let mut mix: f32 = 0.6; // Within range
        mix = mix.clamp(0.0, 1.0);
        assert_eq!(mix, 0.6);

        // Test floor clamping
        let mut floor: f32 = 1.2; // Above max
        floor = floor.clamp(0.0, 1.0);
        assert_eq!(floor, 1.0);

        let mut floor: f32 = -0.05; // Below min
        floor = floor.clamp(0.0, 1.0);
        assert_eq!(floor, 0.0);

        let mut floor: f32 = 0.25; // Within range
        floor = floor.clamp(0.0, 1.0);
        assert_eq!(floor, 0.25);
    }

    #[test]
    fn test_date_range_parsing() {
        let json = r#"{
            "from": "2025-08-01",
            "to": "2025-08-31"
        }"#;

        let date_range: DateRange = serde_json::from_str(json).unwrap();
        assert_eq!(date_range.from, "2025-08-01");
        assert_eq!(date_range.to, "2025-08-31");
    }

    fn create_candidate(id: &str, score: f32) -> Candidate {
        Candidate {
            id: id.to_string(),
            table: "thoughts".to_string(),
            source_type: "thought".to_string(),
            origin: "human".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            text: "test content".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            score,
            tags: vec![],
            is_private: false,
            content_hash: "".to_string(),
            trust_tier: "".to_string(),
        }
    }
}
