-- Document Ingestion Schema for Surreal Mind
-- This schema supports the automatic ingestion of README and CHANGELOG files
-- into the knowledge graph with proper provenance tracking and moderation

-- Documents metadata table
DEFINE TABLE doc_documents SCHEMALESS;
DEFINE FIELD project ON doc_documents TYPE string;
DEFINE FIELD path ON doc_documents TYPE string;
DEFINE FIELD kind ON doc_documents TYPE string ASSERT $value IN ['readme', 'changelog'];
DEFINE FIELD latest_sha ON doc_documents TYPE string;
DEFINE FIELD created_at ON doc_documents TYPE datetime DEFAULT time::now();
DEFINE FIELD modified_at ON doc_documents TYPE datetime DEFAULT time::now();

-- Document sections (from README)
DEFINE TABLE doc_sections SCHEMALESS;
DEFINE FIELD doc_id ON doc_sections TYPE string;
DEFINE FIELD slug ON doc_sections TYPE string;
DEFINE FIELD title ON doc_sections TYPE string;
DEFINE FIELD level ON doc_sections TYPE int MINIMUM 1 MAXIMUM 6;
DEFINE FIELD content ON doc_sections TYPE string;
DEFINE FIELD hash ON doc_sections TYPE string;
DEFINE FIELD commit_sha ON doc_sections TYPE string;
DEFINE FIELD line_from ON doc_sections TYPE int;
DEFINE FIELD line_to ON doc_sections TYPE int;
DEFINE FIELD created_at ON doc_sections TYPE datetime DEFAULT time::now();
DEFINE FIELD modified_at ON doc_sections TYPE datetime DEFAULT time::now();

-- Release information (from CHANGELOG)
DEFINE TABLE releases SCHEMALESS;
DEFINE FIELD id ON releases TYPE string; -- Format: v<major>.<minor>.<patch>
DEFINE FIELD semver ON releases TYPE string;
DEFINE FIELD date ON releases TYPE string;
DEFINE FIELD commit_sha ON releases TYPE string;
DEFINE FIELD created_at ON releases TYPE datetime DEFAULT time::now();
DEFINE FIELD modified_at ON releases TYPE datetime DEFAULT time::now();

-- CHANGELOG entries
DEFINE TABLE changelog_entries SCHEMALESS;
DEFINE FIELD release_id ON changelog_entries TYPE string;
DEFINE FIELD kind ON changelog_entries TYPE string ASSERT $value IN ['Added', 'Changed', 'Deprecated', 'Removed', 'Fixed', 'Security', 'Other'];
DEFINE FIELD text ON changelog_entries TYPE string;
DEFINE FIELD hash ON changelog_entries TYPE string;
DEFINE FIELD created_at ON changelog_entries TYPE datetime DEFAULT time::now();
DEFINE FIELD modified_at ON changelog_entries TYPE datetime DEFAULT time::now();

-- Document claims with verification results
DEFINE TABLE doc_claims SCHEMALESS;
DEFINE FIELD source_type ON doc_claims TYPE string ASSERT $value IN ['readme', 'changelog'];
DEFINE FIELD source_id ON doc_claims TYPE string;
DEFINE FIELD release_id ON doc_claims TYPE option<string>;
DEFINE FIELD commit_sha ON doc_claims TYPE string;
DEFINE FIELD claim_text ON doc_claims TYPE string;
DEFINE FIELD normalized_text ON doc_claims TYPE string;
DEFINE FIELD blake3_hash ON doc_claims TYPE string;
DEFINE FIELD embedding ON doc_claims TYPE array<float>;
DEFINE FIELD embedding_model ON doc_claims TYPE string;
DEFINE FIELD embedding_dim ON doc_claims TYPE int;
DEFINE FIELD verification ON doc_claims TYPE option<object>;
DEFINE FIELD created_at ON doc_claims TYPE datetime DEFAULT time::now();
DEFINE FIELD modified_at ON doc_claims TYPE datetime DEFAULT time::now();

-- Knowledge Graph candidate entities (enhanced)
DEFINE TABLE kg_entity_candidates SCHEMALESS;
DEFINE FIELD entity_type ON kg_entity_candidates TYPE string;
DEFINE FIELD name ON kg_entity_candidates TYPE string;
DEFINE FIELD properties ON kg_entity_candidates TYPE object;
DEFINE FIELD staged ON kg_entity_candidates TYPE bool DEFAULT true;
DEFINE FIELD review_state ON kg_entity_candidates TYPE string ASSERT $value IN ['pending', 'approved', 'rejected'] DEFAULT 'pending';
DEFINE FIELD confidence ON kg_entity_candidates TYPE float MINIMUM 0.0 MAXIMUM 1.0 DEFAULT 0.5;
DEFINE FIELD origin ON kg_entity_candidates TYPE string;
DEFINE FIELD provenance ON kg_entity_candidates TYPE object;
DEFINE FIELD dedupe_key ON kg_entity_candidates TYPE string;
DEFINE FIELD created_at ON kg_entity_candidates TYPE datetime DEFAULT time::now();
DEFINE FIELD modified_at ON kg_entity_candidates TYPE datetime DEFAULT time::now();

-- Knowledge Graph candidate relationships (enhanced)
DEFINE TABLE kg_edge_candidates SCHEMALESS;
DEFINE FIELD rel_type ON kg_edge_candidates TYPE string;
DEFINE FIELD source ON kg_edge_candidates TYPE string;
DEFINE FIELD target ON kg_edge_candidates TYPE string;
DEFINE FIELD staged ON kg_edge_candidates TYPE bool DEFAULT true;
DEFINE FIELD review_state ON kg_edge_candidates TYPE string ASSERT $value IN ['pending', 'approved', 'rejected'] DEFAULT 'pending';
DEFINE FIELD confidence ON kg_edge_candidates TYPE float MINIMUM 0.0 MAXIMUM 1.0 DEFAULT 0.5;
DEFINE FIELD origin ON kg_edge_candidates TYPE string;
DEFINE FIELD provenance ON kg_edge_candidates TYPE object;
DEFINE FIELD dedupe_key ON kg_edge_candidates TYPE string;
DEFINE FIELD created_at ON kg_edge_candidates TYPE datetime DEFAULT time::now();
DEFINE FIELD modified_at ON kg_edge_candidates TYPE datetime DEFAULT time::now();

-- Indexes for performance
-- Document lookups
DEFINE INDEX doc_documents_by_path ON doc_documents FIELDS project, path, kind;
DEFINE INDEX doc_sections_lookup ON doc_sections FIELDS doc_id, slug, commit_sha;
DEFINE INDEX releases_by_semver ON releases FIELDS id, semver;
DEFINE INDEX changelog_lookup ON changelog_entries FIELDS release_id, kind;

-- Claims indexes
DEFINE INDEX doc_claims_dedupe ON doc_claims FIELDS blake3_hash, commit_sha UNIQUE;
DEFINE INDEX doc_claims_by_source ON doc_claims FIELDS source_id;
DEFINE INDEX doc_claims_by_release ON doc_claims FIELDS release_id;

-- Vector search index for claims (use appropriate dimension for your embedder)
-- Note: Adjust dimension to match your embedding model (1536 for OpenAI text-embedding-3-small)
DEFINE INDEX doc_claims_embed_vector ON doc_claims FIELDS embedding TYPE hnsw DIM 1536;

-- Knowledge Graph candidate indexes
DEFINE INDEX kg_entity_candidates_status ON kg_entity_candidates FIELDS review_state, created_at;
DEFINE INDEX kg_entity_candidates_confidence ON kg_entity_candidates FIELDS confidence;
DEFINE INDEX kg_entity_candidates_dedupe ON kg_entity_candidates FIELDS dedupe_key UNIQUE;
DEFINE INDEX kg_entity_candidates_type ON kg_entity_candidates FIELDS entity_type, name, review_state;

DEFINE INDEX kg_edge_candidates_status ON kg_edge_candidates FIELDS review_state, created_at;
DEFINE INDEX kg_edge_candidates_confidence ON kg_edge_candidates FIELDS confidence;
DEFINE INDEX kg_edge_candidates_dedupe ON kg_edge_candidates FIELDS dedupe_key UNIQUE;
DEFINE INDEX kg_edge_candidates_type ON kg_edge_candidates FIELDS rel_type, source, target, review_state;

-- Update helper functions
DEFINE FUNCTION fn::update_doc_timestamp($id: string) {
    UPDATE $id SET modified_at = time::now();
};
