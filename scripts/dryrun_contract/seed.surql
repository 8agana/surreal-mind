
-- Unextracted thoughts (for kg_populate)
CREATE thoughts:t1 SET content = 'The Federation uses SurrealDB for its knowledge graph.', created_at = time::now(), injected_memories = [], injection_scale = 0, significance = 0.5, access_count = 0, extracted_to_kg = false;
CREATE thoughts:t2 SET content = 'REMini orchestrates nightly maintenance tasks.', created_at = time::now(), injected_memories = [], injection_scale = 0, significance = 0.5, access_count = 0, extracted_to_kg = false;
CREATE thoughts:t3 SET content = 'DRY_RUN must never touch the database or call a real provider.', created_at = time::now(), injected_memories = [], injection_scale = 0, significance = 0.5, access_count = 0, extracted_to_kg = false;

-- A thought marked for gemini rethink (non-kg_entities correction path)
CREATE thoughts:t4 SET content = 'This thought was flagged for review.', created_at = time::now(), injected_memories = [], injection_scale = 0, significance = 0.5, access_count = 0, extracted_to_kg = true, marked_for = 'gemini', mark_type = 'correction', mark_note = 'General correction note, no merge target here', marked_at = time::now(), marked_by = 'seed';

-- kg_entities missing embeddings (for kg_embed)
CREATE kg_entities:embtest1 SET name = 'EmbedTestEntityOne', data = { entity_type: 'concept', description: 'seed entity missing an embedding' }, entity_type = 'concept', created_at = time::now();
CREATE kg_entities:embtest2 SET name = 'EmbedTestEntityTwo', data = { entity_type: 'concept', description: 'another seed entity missing an embedding' }, entity_type = 'concept', created_at = time::now();

-- kg_entities pair for consolidate: pre-seeded correction_events row already present
CREATE kg_entities:loserent1 SET name = 'LoserEntityOne', data = { entity_type: 'concept', description: 'duplicate to be merged away' }, entity_type = 'concept', created_at = time::now();
CREATE kg_entities:winnerent1 SET name = 'WinnerEntityOne', data = { entity_type: 'concept', description: 'canonical entity' }, entity_type = 'concept', created_at = time::now();
CREATE correction_events:ce1 SET target_id = 'loserent1', target_table = 'kg_entities', previous_state = {}, new_state = { status: 'pending', mode: 'merge_alias', loser_id: 'loserent1', winner_id: 'winnerent1' }, initiated_by = 'seed', reasoning = 'duplicate entity, should merge into canonical', sources = ['seed'], verification_status = 'auto_applied';

-- kg_entities pair marked for gemini rethink (exercises rethink's correction-event write path for a kg_entities target)
CREATE kg_entities:loserent2 SET name = 'LoserEntityTwo', data = { entity_type: 'concept', description: 'second duplicate' }, entity_type = 'concept', created_at = time::now(), marked_for = 'gemini', mark_type = 'correction', mark_note = 'Merge target: kg_entities:winnerent2', marked_at = time::now(), marked_by = 'seed';
CREATE kg_entities:winnerent2 SET name = 'WinnerEntityTwo', data = { entity_type: 'concept', description: 'second canonical entity' }, entity_type = 'concept', created_at = time::now();

-- kg_entities stale + high-volatility (for the sm_health.sh script path, not exercised by our task subset but kept for completeness)
CREATE kg_entities:staleent1 SET name = 'StaleEntityOne', data = { entity_type: 'concept', description: 'stale high-volatility entity' }, entity_type = 'concept', created_at = time::now(), volatility = 'high', last_refreshed = time::now() - 200d;
