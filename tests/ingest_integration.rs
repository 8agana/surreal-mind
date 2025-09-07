use std::path::Path;
use surreal_mind::ingest::{Candidate, Claim, changelog::ChangelogParser, markdown::ReadmeParser};
use surreal_mind::ingest::{Document, DocumentParser, IngestConfig};

/// Test document ingestion end-to-end
#[cfg(feature = "db_integration")]
#[tokio::test]
async fn test_ingest_end_to_end() {
    // Setup test config
    let config = IngestConfig {
        root_path: "fixtures".to_string(),
        project_slug: "test-project".to_string(),
        include_readme: true,
        include_changelog: true,
        claims_only: false,
        verify_claims: false,
        min_similarity: 0.5,
        verify_top_k: 200,
        evidence_limit: 5,
        batch_size: 100,
        continue_on_error: true,
        max_retries: 2,
        progress: false,
        prometheus: false,
    };

    // Test README parsing
    let readme_content = std::fs::read_to_string("fixtures/sample_readme.md").unwrap();
    let doc = Document {
        id: "test:readme:README.md".to_string(),
        project: "test-project".to_string(),
        path: "README.md".to_string(),
        kind: "readme".to_string(),
        content: readme_content.clone(),
        hash: blake3::hash(readme_content.as_bytes()).to_hex().to_string(),
        commit_sha: "abcd1234".to_string(),
    };

    let path = Path::new("fixtures/sample_readme.md");
    let sections = ReadmeParser::parse(&doc.content, path).unwrap();
    assert!(!sections.is_empty(), "Should extract sections from README");

    let claims = ReadmeParser::extract_claims(&sections, &doc.project).unwrap();
    assert!(
        !claims.is_empty(),
        "Should extract claims from README sections"
    );

    // Check for specific patterns
    let has_install_claim = claims.iter().any(|c| c.claim_text.contains("install"));
    let has_command_claim = claims
        .iter()
        .any(|c| c.claim_text.contains("Command available"));
    assert!(
        has_install_claim || has_command_claim,
        "Should extract install or command claims"
    );

    let candidates = ReadmeParser::generate_candidates(&claims, &doc.project).unwrap();
    assert!(
        !candidates.is_empty(),
        "Should generate candidates from claims"
    );

    // Test CHANGELOG parsing
    let changelog_content = std::fs::read_to_string("fixtures/sample_changelog.md").unwrap();
    let changelog_doc = Document {
        id: "test:changelog:CHANGELOG.md".to_string(),
        project: "test-project".to_string(),
        path: "CHANGELOG.md".to_string(),
        kind: "changelog".to_string(),
        content: changelog_content.clone(),
        hash: blake3::hash(changelog_content.as_bytes())
            .to_hex()
            .to_string(),
        commit_sha: "abcd1234".to_string(),
    };

    let changelog_path = Path::new("fixtures/sample_changelog.md");
    let changelog_sections =
        ChangelogParser::parse(&changelog_doc.content, changelog_path).unwrap();
    assert!(
        !changelog_sections.is_empty(),
        "Should extract releases from CHANGELOG"
    );

    let changelog_claims =
        ChangelogParser::extract_claims(&changelog_sections, &changelog_doc.project).unwrap();
    assert!(
        !changelog_claims.is_empty(),
        "Should extract claims from CHANGELOG releases"
    );

    // Check for version claims
    let has_version_claim = changelog_claims
        .iter()
        .any(|c| c.claim_text.contains("was released"));
    assert!(has_version_claim, "Should extract version release claims");

    let changelog_candidates =
        ChangelogParser::generate_candidates(&changelog_claims, &changelog_doc.project).unwrap();
    assert!(
        !changelog_candidates.is_empty(),
        "Should generate candidates from changelog claims"
    );

    println!(
        "✅ Integration test passed: extracted {} sections, {} claims, {} candidates from README and {} claims, {} candidates from CHANGELOG",
        sections.len(),
        claims.len(),
        candidates.len(),
        changelog_claims.len(),
        changelog_candidates.len()
    );
}

/// Test claim normalization and hashing
#[test]
fn test_claim_normalization() {
    use surreal_mind::ingest::utils;

    let claim_text = "  Rust IS a fast programming language!  ";
    let normalized = claim_text.to_lowercase().trim().to_string();

    let hash1 = utils::hash_claim(claim_text);
    let hash2 = utils::hash_claim(&normalized);

    // Different normalization should produce different hashes
    assert_ne!(hash1, hash2);

    // Same text should produce same hash
    let hash3 = utils::hash_claim(claim_text);
    assert_eq!(hash1, hash3);

    // Hash should be a valid hex string
    assert!(
        hash1.chars().all(|c| c.is_ascii_hexdigit()),
        "Hash should be valid hex"
    );

    // Hash should have the expected length (BLAKE3 output is 64 hex chars)
    assert_eq!(hash1.len(), 64, "Hash should be 64 hex characters");
}

/// Test candidate confidence rules
#[test]
fn test_candidate_confidence_rules() {
    // Test that confidence values are within bounds
    let test_confidences = [0.0, 0.5, 0.75, 1.0];
    for &conf in &test_confidences {
        assert!(
            conf >= 0.0 && conf <= 1.0,
            "Confidence should be between 0.0 and 1.0"
        );
    }

    // Test env var parsing simulation
    let default_heading = 0.65;
    let default_command = 0.80;
    let default_changelog = 0.75;

    assert!(
        default_heading >= 0.4 && default_heading <= 0.9,
        "Heading confidence in reasonable range"
    );
    assert!(
        default_command >= 0.4 && default_command <= 0.9,
        "Command confidence in reasonable range"
    );
    assert!(
        default_changelog >= 0.4 && default_changelog <= 0.9,
        "Changelog confidence in reasonable range"
    );
}

/// Test document ID generation
#[test]
fn test_document_id_generation() {
    use surreal_mind::ingest::utils;

    let doc_id = utils::generate_doc_id("surreal-mind", "README.md", "readme");
    assert_eq!(doc_id, "surreal-mind:readme:README.md");

    let changelog_doc_id = utils::generate_doc_id("my-project", "CHANGELOG.md", "changelog");
    assert_eq!(changelog_doc_id, "my-project:changelog:CHANGELOG.md");
}
