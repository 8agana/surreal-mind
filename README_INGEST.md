# Document Ingestion (`sm_ingest_docs`)

The `sm_ingest_docs` binary provides deterministic ingestion of README and CHANGELOG files into the Surreal Mind knowledge graph. This tool extracts structured information, generates claims, and creates candidates for verification.

## Quick Start

```bash
# Build the binary
cargo build --release --bin ingest_repo_docs

# Ingest README and CHANGELOG with defaults
./target/release/ingest_repo_docs --readme --changelog

# Dry run to see what would be processed
./target/release/ingest_repo_docs --readme --dry-run

# Process with verification
./target/release/ingest_repo_docs --readme --changelog --verify-claims --min-sim 0.5
```

## Command Line Flags

- `--root <PATH>`: Root directory to scan (default: current directory)
- `--project <SLUG>`: Project slug for IDs (default: "surreal-mind")
- `--readme`: Process README.md files
- `--changelog`: Process CHANGELOG.md files
- `--claims-only`: Only generate claims, skip candidate creation
- `--verify-claims`: Run hypothesis verification on extracted claims
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
```

## Environment Variables

Control confidence thresholds and behavior:

- `SURR_INGEST_CONFIDENCE_HEADING`: Confidence for headings/components (default: 0.65)
- `SURR_INGEST_CONFIDENCE_COMMAND`: Confidence for commands (default: 0.80)
- `SURR_INGEST_CONFIDENCE_CHANGELOG`: Confidence for changelog entries (default: 0.75)
- `SURR_INGEST_CONFIDENCE_VERIFICATION_BONUS`: Bonus for verified claims (default: 0.05)
- `SURR_INGEST_CONFIDENCE_CONTRADICTION_PENALTY`: Penalty for contradictory claims (default: 0.10)

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
  "sections_extracted": 15,
  "claims_generated": 23,
  "candidates_created": 12,
  "errors": [],
  "metrics": "...prometheus format..."
}
```

### Prometheus Metrics
```
# HELP ingest_sections_parsed_total Total sections parsed
# TYPE ingest_sections_parsed_total counter
ingest_sections_parsed_total 15
# HELP ingest_claims_extracted_total Total claims extracted
# TYPE ingest_claims_extracted_total counter
ingest_claims_extracted_total 23
# HELP ingest_candidates_created_total Total candidates created
# TYPE ingest_candidates_created_total counter
ingest_candidates_created_total 12
# HELP ingest_errors_count_total Total errors during ingestion
# TYPE ingest_errors_count_total counter
ingest_errors_count_total 0
```

## Verification Details

When using `--verify-claims`, the tool runs hypothesis verification against extracted claims:

- **Similarity**: Claims are embedded and compared against KG entities/observations
- **Evidence**: Up to `--evidence-limit` supporting/contradicting items per claim
- **Confidence**: Score = supporting / (supporting + contradicting)
- **Persistence**: Results stored in claim records with telemetry

The verification uses the same embedding model as the KG and respects dimension consistency.

## Error Handling

- Use `--continue-on-error` to skip problematic files
- Set `--max-retries` for transient DB failures
- Check JSON output for detailed error messages
- Failed batches are logged but don't stop processing

## Troubleshooting

- **No claims generated**: Check if files exist and contain expected content patterns
- **Verification fails**: Ensure KG has embeddings; run `reembed_kg` if needed
- **DB connection issues**: Verify environment variables and network connectivity
- **Dimension mismatch**: Use same embedder as KG; check `SURR_EMBED_MODEL`

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

The provided workflow (`.github/workflows/ingest-docs.yml`) handles this automatically.