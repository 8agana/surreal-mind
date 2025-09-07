//! Changelog parser for CHANGELOG files
//!
//! Implements deterministic extraction of releases, entries, and claims from CHANGELOG.md
//! using regex parsing of Keep a Changelog format without LLM dependencies.

use crate::error::{Result, SurrealMindError};
use crate::ingest::{Candidate, Claim, DocumentParser, Section, utils};
use regex::Regex;
use semver::Version;
use std::path::Path;

/// Parser for CHANGELOG.md files
pub struct ChangelogParser;

impl DocumentParser for ChangelogParser {
    /// Parse CHANGELOG content and extract sections (treating releases as sections)
    fn parse(content: &str, _path: &Path) -> Result<Vec<Section>> {
        let mut sections = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            // Look for version headers like "## [1.2.3] - 2023-10-01" or "## [Unreleased]"
            if lines[i].starts_with("## [") {
                let (version, _date, line_start) = parse_version_header(&lines[i])?;
                let mut content_lines = Vec::new();
                i += 1;

                // Collect content until next version header
                while i < lines.len() && !lines[i].starts_with("## [") {
                    if !lines[i].trim().is_empty() {
                        content_lines.push(lines[i]);
                    }
                    i += 1;
                }

                let section_content = content_lines.join("\n");
                let section = Section {
                    id: utils::generate_section_id(
                        "changelog",
                        &version,
                        &blake3::hash(section_content.as_bytes()).to_hex()[..8],
                    ),
                    doc_id: "".to_string(), // Will be set by caller
                    slug: format!("release-{}", version.replace(".", "-")),
                    title: format!("Release {}", version),
                    level: 2, // H2 level
                    content: section_content.clone(),
                    hash: blake3::hash(section_content.as_bytes())
                        .to_hex()
                        .to_string(),
                    commit_sha: "".to_string(), // Will be set by caller
                    line_from: line_start,
                    line_to: i,
                };
                sections.push(section);
            } else {
                i += 1;
            }
        }

        Ok(sections)
    }

    /// Extract deterministic claims from CHANGELOG sections
    fn extract_claims(sections: &[Section], _project_slug: &str) -> Result<Vec<Claim>> {
        let mut claims = Vec::new();

        for section in sections {
            // Extract release info from title
            if let Some(release_info) = extract_release_info(&section.title) {
                // Generate claim for the release itself
                let claim_text = format!(
                    "Release {} was released on {}",
                    release_info.version, release_info.date
                );
                let claim = Claim {
                    id: utils::generate_claim_id(&section.id, &claim_text),
                    source_type: "changelog".to_string(),
                    source_id: section.id.clone(),
                    release_id: Some(utils::generate_release_id(&release_info.version)),
                    commit_sha: section.commit_sha.clone(),
                    claim_text: claim_text.clone(),
                    normalized_text: claim_text.to_lowercase(),
                    blake3_hash: utils::hash_claim(&claim_text),
                    embedding: vec![], // Will be filled during persistence
                    embedding_model: "".to_string(),
                    embedding_dim: 0,
                    created_at: utils::current_timestamp(),
                };
                claims.push(claim);

                // Parse change entries and generate specific claims
                let entries = parse_changelog_entries(&section.content)?;
                for entry in entries {
                    if let Some(entry_claim) = generate_entry_claim(
                        &entry,
                        &section.id,
                        &section.commit_sha,
                        &release_info.version,
                    ) {
                        claims.push(entry_claim);
                    }
                }
            }
        }

        Ok(claims)
    }

    /// Generate candidates from claims
    fn generate_candidates(claims: &[Claim], project_slug: &str) -> Result<Vec<Candidate>> {
        let mut candidates = Vec::new();

        for claim in claims {
            // Generate project entity for releases
            if claim.claim_text.contains("was released") {
                let candidate = Candidate {
                    kind: "entity".to_string(),
                    data: serde_json::json!({
                        "name": project_slug,
                        "entity_type": "project",
                        "properties": {},
                        "status": "pending"
                    }),
                    confidence: 0.75, // SURR_INGEST_CONFIDENCE_CHANGELOG default
                    provenance: crate::ingest::Provenance {
                        doc_id: format!("{}:changelog:CHANGELOG.md", project_slug),
                        section_id: claim.source_id.clone(),
                        claim_id: claim.id.clone(),
                        commit_sha: claim.commit_sha.clone(),
                        line_from: 0,
                        line_to: 0,
                    },
                };
                candidates.push(candidate);
            }

            // Generate component entities from change entries
            if claim.claim_text.contains("Added")
                || claim.claim_text.contains("Changed")
                || claim.claim_text.contains("Removed")
                || claim.claim_text.contains("Fixed")
            {
                // Extract potential component names
                if let Some(component_name) = extract_component_from_entry(&claim.claim_text) {
                    let entity_type = if claim.claim_text.contains("Added") {
                        "new_feature"
                    } else if claim.claim_text.contains("Removed") {
                        "removed_feature"
                    } else {
                        "updated_feature"
                    };

                    let candidate = Candidate {
                        kind: "entity".to_string(),
                        data: serde_json::json!({
                            "name": component_name,
                            "entity_type": entity_type,
                            "properties": {
                                "change_type": extract_change_kind(&claim.claim_text)
                            },
                            "status": "pending"
                        }),
                        confidence: 0.75, // SURR_INGEST_CONFIDENCE_CHANGELOG default
                        provenance: crate::ingest::Provenance {
                            doc_id: format!("{}:changelog:CHANGELOG.md", project_slug),
                            section_id: claim.source_id.clone(),
                            claim_id: claim.id.clone(),
                            commit_sha: claim.commit_sha.clone(),
                            line_from: 0,
                            line_to: 0,
                        },
                    };
                    candidates.push(candidate);
                }
            }
        }

        Ok(candidates)
    }
}

/// Parse version header like "## [1.2.3] - 2023-10-01"
fn parse_version_header(line: &str) -> Result<(String, String, usize)> {
    // Note: This is a placeholder - actual line number would be passed in
    let line_num = 0;

    if let Some(start) = line.find('[') {
        if let Some(end) = line.find(']') {
            let version = &line[start + 1..end];
            let after = &line[end + 1..];

            // Look for date after dash
            if let Some(dash_pos) = after.find(" - ") {
                let date = after[dash_pos + 3..].trim();
                return Ok((version.to_string(), date.to_string(), line_num));
            } else if version.to_lowercase() == "unreleased" {
                return Ok(("unreleased".to_string(), "unreleased".to_string(), line_num));
            }
        }
    }

    Err(SurrealMindError::Parse {
        message: format!("Could not parse version header: {}", line),
    })
}

/// Extract release info from section title
fn extract_release_info(title: &str) -> Option<ReleaseInfo> {
    let re = Regex::new(r"Release (\d+\.\d+\.\d+(?:-[a-zA-Z0-9\.\-]+)?)").unwrap();
    if let Some(cap) = re.captures(title) {
        let version = cap.get(1)?.as_str().to_string();
        // Validate semver
        if Version::parse(&version).is_ok() || version == "unreleased" {
            Some(ReleaseInfo {
                version,
                date: extract_date_from_title(title),
            })
        } else {
            None
        }
    } else {
        None
    }
}

/// Extract date from title
fn extract_date_from_title(title: &str) -> String {
    let re = Regex::new(r"(\d{4}-\d{2}-\d{2})").unwrap();
    if let Some(cap) = re.captures(title) {
        cap.get(1).unwrap().as_str().to_string()
    } else {
        "unknown".to_string()
    }
}

/// Parse changelog entries from section content
fn parse_changelog_entries(content: &str) -> Result<Vec<ChangeEntry>> {
    let mut entries = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut current_kind = String::new();
    let mut current_items: Vec<String> = Vec::new();

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Check for section headers
        if line.starts_with("### ") {
            // Process previous entries
            if !current_items.is_empty() {
                for item in &current_items {
                    entries.push(ChangeEntry {
                        kind: current_kind.clone(),
                        text: item.clone(),
                    });
                }
                current_items.clear();
            }
            current_kind = line.trim_start_matches("### ").to_string();
        } else if line.starts_with("- ") {
            // Entry item
            let item = line.trim_start_matches("- ").to_string();
            current_items.push(item);
        }
    }

    // Process remaining items
    if !current_items.is_empty() {
        for item in current_items {
            entries.push(ChangeEntry {
                kind: current_kind.clone(),
                text: item,
            });
        }
    }

    Ok(entries)
}

/// Generate claim from changelog entry
fn generate_entry_claim(
    entry: &ChangeEntry,
    section_id: &str,
    commit_sha: &str,
    release_version: &str,
) -> Option<Claim> {
    let claim_text = match entry.kind.as_str() {
        "Added" => format!(
            "{} exists in {}",
            extract_subject(&entry.text),
            release_version
        ),
        "Changed" => format!(
            "{} was updated in {}",
            extract_subject(&entry.text),
            release_version
        ),
        "Deprecated" => format!(
            "{} is deprecated in {}",
            extract_subject(&entry.text),
            release_version
        ),
        "Removed" => format!(
            "{} no longer exists in {}",
            extract_subject(&entry.text),
            release_version
        ),
        "Fixed" => format!(
            "Issue with {} was fixed in {}",
            extract_subject(&entry.text),
            release_version
        ),
        "Security" => format!(
            "Security issue with {} was addressed in {}",
            extract_subject(&entry.text),
            release_version
        ),
        _ => return None,
    };

    Some(Claim {
        id: utils::generate_claim_id(section_id, &claim_text),
        source_type: "changelog".to_string(),
        source_id: section_id.to_string(),
        release_id: Some(utils::generate_release_id(release_version)),
        commit_sha: commit_sha.to_string(),
        claim_text: claim_text.clone(),
        normalized_text: claim_text.to_lowercase(),
        blake3_hash: utils::hash_claim(&claim_text),
        embedding: vec![],
        embedding_model: "".to_string(),
        embedding_dim: 0,
        created_at: utils::current_timestamp(),
    })
}

/// Extract subject from entry text
fn extract_subject(text: &str) -> String {
    // Simple heuristic: take first noun phrase or component name
    let words: Vec<&str> = text.split_whitespace().collect();
    if !words.is_empty() {
        let first_word = words[0];
        if first_word.ends_with('s') {
            first_word.trim_end_matches('s').to_string()
        } else {
            first_word.to_string()
        }
    } else {
        "unknown".to_string()
    }
}

/// Extract component name from entry text
fn extract_component_from_entry(text: &str) -> Option<String> {
    // Look for component mentions
    if text.contains("module") || text.contains("component") || text.contains("tool") {
        extract_subject(text).into()
    } else {
        None
    }
}

/// Extract change kind from claim text
fn extract_change_kind(claim_text: &str) -> String {
    if claim_text.contains("Added") {
        "addition".to_string()
    } else if claim_text.contains("Removed") {
        "removal".to_string()
    } else if claim_text.contains("Changed") {
        "modification".to_string()
    } else if claim_text.contains("Fixed") {
        "bugfix".to_string()
    } else {
        "other".to_string()
    }
}

/// Helper structs
struct ReleaseInfo {
    version: String,
    date: String,
}

struct ChangeEntry {
    kind: String,
    text: String,
}
