# Document Ingestion (`sm_ingest_docs`)

The `sm_ingest_docs` binary provides deterministic ingestion of README and CHANGELOG files into the Surreal Mind knowledge graph. This tool extracts structured information, generates claims, runs real hypothesis verification against the existing KG, and optionally persists results with full provenance tracking.

## Quick Start

```bash
# Build the binary
cargo build --release --bin ingest_repo_docs

# Ingest README and CHANGELOG with defaults
./target/release/ingest_repo_docs --readme --changelog

# Dry run to see what would be processed
./target/release/ingest_repo_docs --readme --dry-run

# Process with real verification
./target/release/ingest_repo_docs --readme --changelog --verify-claims --min-sim 0.5

# With persistence enabled
./target/release/ingest_repo_docs --readme --changelog --verify-claims --persist

# CI mode with JSON metrics
./target/release/ingest_repo_docs --readme --changelog --verify-claims --json --persist
```

## Command Line Flags

- `--root <PATH>`: Root directory to scan (default: current directory)
- `--root <PATH>`: Root directory to scan (default: current directory)
- `--project <SLUG>`: Project slug for IDs (default: "surreal-mind")
- `--readme`: Process README.md files
- `--changelog`: Process CHANGELOG.md files
- `--claims-only`: Only generate claims, skip candidate creation
- `--verify-claims`: Run real hypothesis verification on extracted claims
- `--all-claims`: Verify all claims (not just new ones)
- `--persist`: Persist results to database (off by default)
- `--min-sim <FLOAT>`: Minimum similarity for verification (0.0-1.0, default: 0.5)
- `--verify-top-k <INT>`: Top K candidates for verification (default: 200)
- `--evidence-limit <INT>`: Max evidence items per bucket (default: 5)
- `--batch-size <INT>`: Batch size for DB operations (default: 100)
- `--continue-on-error`: Continue processing on individual errors
- `--max-retries <INT>`: Max retries for failed operations (default: 2)
- `--progress`: Show progress during processing
- `--version`: Show version information
- `--dry-run`: Show what would be done without executing
- `--json`: Output results as JSON for scripting
- `--prometheus`: Output metrics in Prometheus format
- `--commit <SHA>`: Override commit SHA (default: git rev-parse HEAD)

## Examples

### Basic Usage
```bash
# Process all documents with default settings
./target/release/ingest_repo_docs --readme --changelog

# Process only README with verbose output
./target/release/ingest_repo_docs --readme --progress

# Generate JSON output for scripting
./target/release/ingest_repo_docs --readme --json > results.json
```

### With Verification
```bash
# Run full pipeline: extraction + verification
./target/release/ingest_repo_docs --readme --changelog --verify-claims --min-sim 0.7 --verify-top-k 300

# Only verify existing claims without new extraction
./target/release/ingest_repo_docs --verify-claims --claims-only
```

### Batch Processing
```bash
# Process in small batches with error recovery
./target/release/ingest_repo_docs --readme --batch-size 50 --continue-on-error --max-retries 3
```

### CI/CD Integration
```bash
# Typical CI usage with environment variables
export SURR_INGEST_CONFIDENCE_HEADING=0.7
export SURR_INGEST_CONFIDENCE_COMMAND=0.85
export SURR_INGEST_CONFIDENCE_CHANGELOG=0.8

./target/release/ingest_repo_docs \
  --readme \
  --changelog \
  --verify-claims \
  --min-sim 0.5 \
  --verify-top-k 200 \
  --batch-size 100 \
  --continue-on-error \
  --json \
  --commit "$GITHUB_SHA" > ingestion_results.json
# With persistence enabled
./target/release/ingest_repo_docs --readme --changelog --verify-claims --persist

# Verify all existing claims
./target/release/ingest_repo_docs --verify-claims --all-claims --persist
```

## Environment Variables

Control confidence thresholds and behavior:

- `SURR_INGEST_CONFIDENCE_HEADING`: Confidence for headings/components (default: 0.65)
- `SURR_INGEST_CONFIDENCE_COMMAND`: Confidence for commands (default: 0.80)
- `SURR_INGEST_CONFIDENCE_CHANGELOG`: Confidence for changelog entries (default: 0.75)
- `SURR_INGEST_CONFIDENCE_VERIFICATION_BONUS`: Bonus for verified claims (default: 0.05)
- `SURR_INGEST_CONFIDENCE_CONTRADICTION_PENALTY`: Penalty for contradictory claims (default: 0.10)

## Database Schema

The tool creates the following tables when `--persist` is used:

- `doc_documents`: Document metadata with project, path, kind, and latest SHA
- `doc_sections`: Parsed sections with content, hierarchy, and line numbers
- `doc_claims`: Extracted claims with embeddings, verification results, and provenance
- `releases`: Version information from CHANGELOG
- `changelog_entries`: Individual changelog items with kinds and text
- `kg_entity_candidates`: Entity candidates with confidence scores
- `kg_edge_candidates`: Relationship candidates with provenance

Vector indexes are created for efficient KG search and verification.

## Output Formats

### Human-Readable (Default)
```
✅ Ingest complete!
📄 Documents processed: 2
📑 Sections extracted: 15
💭 Claims generated: 23
🎯 Candidates created: 12
```

### JSON Output
```json
{
  "documents_processed": 2,
  "sections_extracted": 10,
  "claims_extracted": 23,
  "claims_generated": 23,
  "claims_deduped": 0,
  "claims_verified": 23,
  "support_hits": 4,
  "contradict_hits": 1,
  "candidates_created": 9,
  "errors": [],
  "metrics": "...prometheus format..."
}
```

### Prometheus Metrics
```
# HELP ingest_sections_parsed_total Total sections parsed
# TYPE ingest_sections_parsed_total counter
ingest_sections_parsed_total 10
# HELP ingest_claims_extracted_total Total claims extracted
# TYPE ingest_claims_extracted_total counter
ingest_claims_extracted_total 23
# HELP ingest_claims_deduped_total Total claims deduplicated
# TYPE ingest_claims_deduped_total counter
ingest_claims_deduped_total 0
# HELP ingest_claims_verified_total Total claims verified
# TYPE ingest_claims_verified_total counter
ingest_claims_verified_total 23
# HELP ingest_support_hits_total Total supporting evidence hits
# TYPE ingest_support_hits_total counter
ingest_support_hits_total 4
# HELP ingest_contradict_hits_total Total contradicting evidence hits
# TYPE ingest_contradict_hits_total counter
ingest_contradict_hits_total 1
# HELP ingest_candidates_created_total Total candidates created
# TYPE ingest_candidates_created_total counter
ingest_candidates_created_total 9
# HELP ingest_errors_count_total Total errors during ingestion
# TYPE ingest_errors_count_total counter
ingest_errors_count_total 0
```

## Verification Details

When using `--verify-claims`, the tool runs **real hypothesis verification** against the existing KG:

### Process Flow
1. **Embedding Generation**: Claims are embedded using the same model as KG (default: OpenAI text-embedding-3-small, 1536-dim)
2. **KG Search**: Claims are compared against existing `kg_entities` and `kg_observations` using cosine similarity
3. **Evidence Classification**: Similar items are categorized as supporting or contradicting based on pattern matching
4. **Confidence Calculation**: Score = supporting_evidence / (supporting_evidence + contradicting_evidence)

### Telemetry Provided
- `embedding_provider`: e.g., "openai"
- `embedding_model`: e.g., "text-embedding-3-small"
- `embedding_dim`: e.g., 1536
- `total_candidates`: Total KG items within similarity threshold
- `candidates_with_embedding`: Items that had valid embeddings
- `candidates_after_similarity`: Items that passed similarity filtering
- `support_hits`: Number of supporting evidence items
- `contradict_hits`: Number of contradicting evidence items

### Persistence (when `--persist` enabled)
- Verification results stored in `doc_claims.verification` field
- Includes confidence scores, evidence counts, and full telemetry
- Preserved across runs for audit and analysis

### Safety Features
- Dimension consistency checking between embedder and KG
- Timeouts and error boundaries
- Fallback to non-persistence mode on DB errors
- No mixed-dimension comparisons

## Error Handling

- Use `--continue-on-error` to skip problematic files
- Set `--max-retries` for transient DB failures
- Check JSON output for detailed error messages
- Failed batches are logged but don't stop processing

## Persistence Details

When `--persist` is enabled, the tool stores all extracted data in the database:

### Tables Created
- `doc_documents`: Source document metadata
- `doc_sections`: Parsed sections with content and line numbers
- `doc_claims`: Extracted claims with embeddings and verification results
- `releases`: Version information from CHANGELOG
- `changelog_entries`: Individual changelog items
- `kg_entity_candidates`: Entity candidates awaiting moderation
- `kg_edge_candidates`: Relationship candidates awaiting moderation

### Batching & Reliability
- Configurable batch sizes (default: 100)
- Idempotent operations using hash-based deduplication
- Transaction rollbacks on batch failures
- Error recovery with `--continue-on-error`

### Provenance Tracking
- All data linked to source files, commits, and line numbers
- Timestamp tracking for auditing and temporal queries
- Git SHA preservation for exact version linking

## Troubleshooting

- **No claims generated**: Check if files exist and contain expected content patterns
- **Verification fails**: Ensure KG has embeddings; run `reembed_kg` if needed
- **DB connection issues**: Verify environment variables and network connectivity
- **Dimension mismatch**: Use same embedder as KG; check `SURR_EMBED_MODEL`
- **Persistence disabled**: Use `--persist` flag to enable database writes
- **Schema errors**: Tool will automatically create missing tables on first run

## CI Integration

Set these secrets in your GitHub repository:
- `SURR_DB_URL`
- `SURR_DB_NS`
- `SURR_DB_DB`
- `SURR_DB_USER`
- `SURR_DB_PASS`

Optional variables:
- `SURR_INGEST_CONFIDENCE_*` for threshold tuning
- `SURR_INGEST_CONFIDENCE_VERIFICATION_*` for verification tuning

### CI Integration

The tool supports three modes for different CI scenarios:

1. **Claims Generation**: `--readme --changelog --claims-only --json`
   - Extracts and verifies claims without persisting
   - Suitable for validation-only CI jobs

2. **Full Pipeline**: `--readme --changelog --verify-claims --persist --json`
   - Complete end-to-end processing with persistence
   - Creates all database records and candidates

3. **Verification Only**: `--verify-claims --all-claims --persist --json`
   - Re-verifies existing claims in database
   - Useful for reprocessing after KG updates

### Workflow Integration

The provided GitHub Actions workflow (`.github/workflows/ingest-docs.yml`) uses:

```yaml
- name: Extract claims from documents
  run: ./target/release/ingest_repo_docs --readme --changelog --claims-only --json > claims_output.json

- name: Run hypothesis verification
  run: ./target/release/ingest_repo_docs --verify-claims --persist --json > verify_output.json
```

Results are parsed with `jq` for step summaries and failure detection.