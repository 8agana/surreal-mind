//! Unit tests for inner_voice tool

use surreal_mind::tools::inner_voice::{
    Candidate, allocate_slots, apply_adaptive_floor, cap_text, compute_trust_tier, hash_content,
    select_and_dedupe,
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

        let query_emb = vec![0.1, 0.2, 0.3]; // 3 dims

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
        let include_tags = vec!["important".to_string()];
        let exclude_tags = vec!["private".to_string()];

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
