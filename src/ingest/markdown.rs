//! Markdown parser for README files
//!
//! Implements deterministic extraction of sections, claims, and candidates from README.md
//! using CommonMark parsing without LLM dependencies.

use crate::error::Result;
use crate::ingest::{Candidate, Claim, DocumentParser, Provenance, Section, utils};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use regex::Regex;
use std::path::Path;

/// Parser for README.md files
pub struct ReadmeParser;

impl DocumentParser for ReadmeParser {
    /// Parse README content and extract sections using pulldown_cmark
    fn parse(content: &str, _path: &Path) -> Result<Vec<Section>> {
        let parser = Parser::new(content);
        let mut sections = Vec::new();
        let mut current_section: Option<SectionBuilder> = None;
        let mut heading_text = String::new();
        let mut in_heading = false;

        // Pre-calculate line numbers for events
        let mut line_number = 1;
        let mut current_line_pos = 0;

        for event in parser {
            // Update line number based on text consumed
            if let Event::Text(ref text) = event {
                let newline_count = text.chars().filter(|&c| c == '\n').count();
                line_number += newline_count;
                current_line_pos += text.len();
            }

            match event {
                Event::Start(tag) => {
                    if let Tag::Heading { level, .. } = tag {
                        // Finish previous section
                        if let Some(mut section) = current_section.take() {
                            if let Ok(s) = section.build() {
                                sections.push(s);
                            }
                        }

                        in_heading = true;
                        heading_text.clear();

                        let section = SectionBuilder {
                            title: String::new(),
                            level: level as u8,
                            content: String::new(),
                            slug: String::new(),
                            line_from: line_number,
                            line_to: line_number,
                        };
                        current_section = Some(section);
                    }
                }
                Event::Text(text) => {
                    if in_heading {
                        heading_text.push_str(&text);
                    } else if let Some(ref mut section) = current_section {
                        section.content.push_str(&text);
                        section.content.push(' ');
                    }
                }
                Event::End(tag_end) => {
                    if let TagEnd::Heading(_) = tag_end {
                        if let Some(ref mut section) = current_section {
                            section.title = heading_text.clone();
                            section.slug = slugify(&heading_text);
                        }
                        in_heading = false;
                    }
                }
                _ => {} // Ignore other events for now
            }
        }

        // Finish last section
        if let Some(mut section) = current_section.take() {
            if let Ok(s) = section.build() {
                sections.push(s);
            }
        }

        Ok(sections)
    }

    /// Extract deterministic claims from README sections
    fn extract_claims(sections: &[Section], _project_slug: &str) -> Result<Vec<Claim>> {
        let mut claims = Vec::new();

        for section in sections {
            // Pattern: headings containing keywords like "supports", "requires", "is"
            if section.level <= 3 {
                let re = Regex::new(
                    r"(?i)\b(supports?|requires?|is|provides?|uses?|allows?|enables?)\b.*?(?:\.|$)",
                )
                .unwrap();
                for cap in re.captures_iter(&section.content) {
                    let claim_text = cap.get(0).unwrap().as_str().trim();
                    if claim_text.len() > 10 {
                        // Filter short claims
                        let claim = Claim {
                            id: utils::generate_claim_id(&section.id, claim_text),
                            source_type: "readme".to_string(),
                            source_id: section.id.clone(),
                            release_id: None,
                            commit_sha: section.commit_sha.clone(),
                            claim_text: claim_text.to_string(),
                            normalized_text: normalize_claim_text(claim_text),
                            blake3_hash: utils::hash_claim(claim_text),
                            embedding: vec![], // Will be filled during persistence
                            embedding_model: "".to_string(),
                            embedding_dim: 0,
                            created_at: utils::current_timestamp(),
                        };
                        claims.push(claim);
                    }
                }
            }

            // Extract commands from code blocks
            if let Some(commands) = extract_commands_from_content(&section.content, &section.id) {
                for cmd in commands {
                    let claim = Claim {
                        id: utils::generate_claim_id(&section.id, &cmd),
                        source_type: "readme".to_string(),
                        source_id: section.id.clone(),
                        release_id: None,
                        commit_sha: section.commit_sha.clone(),
                        claim_text: format!("Command available: {}", cmd),
                        normalized_text: format!("command available: {}", normalize_command(&cmd)),
                        blake3_hash: utils::hash_claim(&cmd),
                        embedding: vec![],
                        embedding_model: "".to_string(),
                        embedding_dim: 0,
                        created_at: utils::current_timestamp(),
                    };
                    claims.push(claim);
                }
            }
        }

        Ok(claims)
    }

    /// Generate candidates from claims
    fn generate_candidates(claims: &[Claim], project_slug: &str) -> Result<Vec<Candidate>> {
        let mut candidates = Vec::new();

        for claim in claims {
            // Generate entity candidates from component mentions (more inclusive)
            if claim.claim_text.contains("module")
                || claim.claim_text.contains("component")
                || claim.claim_text.contains("server")
                || claim.claim_text.contains("tool")
                || claim.claim_text.contains("config")
                || claim.claim_text.contains("system")
                || claim.claim_text.contains("feature")
                || claim.claim_text.contains("provider")
                || claim.claim_text.contains("database")
            {
                let candidate = Candidate {
                    kind: "entity".to_string(),
                    data: serde_json::json!({
                        "name": extract_entity_name(&claim.claim_text),
                        "entity_type": "component",
                        "properties": {},
                        "status": "pending"
                    }),
                    confidence: 0.65, // SURR_INGEST_CONFIDENCE_HEADING default
                    provenance: Provenance {
                        doc_id: format!("{}:readme:README.md", project_slug),
                        section_id: claim.source_id.clone(),
                        claim_id: claim.id.clone(),
                        commit_sha: claim.commit_sha.clone(),
                        line_from: 0,
                        line_to: 0,
                    },
                };
                candidates.push(candidate);
            }

            // Generate command entity candidates (more inclusive patterns)
            if claim.claim_text.starts_with("Command available:")
                || claim.claim_text.contains("cargo")
                || claim.claim_text.contains("install")
            {
                let cmd = if claim.claim_text.starts_with("Command available: ") {
                    claim
                        .claim_text
                        .trim_start_matches("Command available: ")
                        .trim()
                } else {
                    // Extract command from claim text
                    if claim.claim_text.contains("cargo install") {
                        claim
                            .claim_text
                            .split("cargo install")
                            .nth(1)
                            .unwrap_or(&claim.claim_text)
                            .trim()
                            .split_whitespace()
                            .next()
                            .unwrap_or(&claim.claim_text)
                    } else {
                        &claim.claim_text
                    }
                };
                let candidate = Candidate {
                    kind: "entity".to_string(),
                    data: serde_json::json!({
                        "name": cmd,
                        "entity_type": "command",
                        "properties": {},
                        "status": "pending"
                    }),
                    confidence: 0.80, // SURR_INGEST_CONFIDENCE_COMMAND default
                    provenance: Provenance {
                        doc_id: format!("{}:readme:README.md", project_slug),
                        section_id: claim.source_id.clone(),
                        claim_id: claim.id.clone(),
                        commit_sha: claim.commit_sha.clone(),
                        line_from: 0,
                        line_to: 0,
                    },
                };
                candidates.push(candidate);
            }

            // Also create candidates for claims that look like features
            if claim.claim_text.contains("supports")
                || claim.claim_text.contains("provides")
                || claim.claim_text.contains("uses")
                || claim.claim_text.contains("requires")
            {
                let candidate = Candidate {
                    kind: "entity".to_string(),
                    data: serde_json::json!({
                        "name": extract_entity_name(&claim.claim_text),
                        "entity_type": "feature",
                        "properties": {
                            "description": claim.claim_text.clone()
                        },
                        "status": "pending"
                    }),
                    confidence: 0.70,
                    provenance: Provenance {
                        doc_id: format!("{}:readme:README.md", project_slug),
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

        Ok(candidates)
    }
}

/// Helper struct for building sections
struct SectionBuilder {
    title: String,
    level: u8,
    content: String,
    slug: String,
    line_from: usize,
    line_to: usize,
}

impl SectionBuilder {
    fn build(self) -> Result<Section> {
        Ok(Section {
            id: format!(
                "section:{}:{}",
                self.slug,
                &blake3::hash(self.content.as_bytes()).to_hex()[..8]
            ),
            doc_id: "".to_string(), // Will be set by caller
            slug: self.slug,
            title: self.title,
            level: self.level,
            content: self.content.clone(),
            hash: blake3::hash(self.content.as_bytes()).to_hex().to_string(),
            commit_sha: "".to_string(), // Will be set by caller
            line_from: self.line_from,
            line_to: self.line_to,
        })
    }
}

/// Find heading text and line number

/// Slugify text for URL-safe identifiers
fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Normalize claim text for consistent hashing
fn normalize_claim_text(text: &str) -> String {
    text.to_lowercase().trim().to_string()
}

/// Extract commands from code blocks
fn extract_commands_from_content(content: &str, _section_id: &str) -> Option<Vec<String>> {
    let re = Regex::new(r"```\s*(bash|sh|shell|console)\n(.*?)\n```").unwrap();
    let mut commands = Vec::new();

    for cap in re.captures_iter(content) {
        let block_content = cap.get(2)?.as_str();
        for line in block_content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                // Extract command name from lines like "cargo install surreal-mind"
                if line.contains("cargo install") {
                    if let Some(cmd_part) = line.split("cargo install").nth(1) {
                        let cmd = cmd_part
                            .trim()
                            .split_whitespace()
                            .next()
                            .unwrap_or(cmd_part.trim());
                        commands.push(format!("cargo install {}", cmd));
                    }
                } else {
                    commands.push(line.to_string());
                }
            }
        }
    }

    if commands.is_empty() {
        None
    } else {
        Some(commands)
    }
}

/// Extract entity name from claim text
fn extract_entity_name(claim_text: &str) -> String {
    // Simple heuristic: take first noun-like phrase after keywords
    let keywords = [
        "module",
        "component",
        "server",
        "tool",
        "config",
        "system",
        "feature",
        "provider",
        "database",
    ];

    for &keyword in &keywords {
        if let Some(start) = claim_text.find(keyword) {
            let before = &claim_text[..start];
            let words: Vec<&str> = before.split_whitespace().collect();
            if let Some(last_word) = words.last() {
                if !last_word.is_empty() {
                    return last_word.to_string();
                }
            }
        }
    }

    // Fallback: take first meaningful word
    let words: Vec<&str> = claim_text.split_whitespace().collect();
    for word in words {
        if word.len() > 3 && word.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return word.to_string();
        }
    }

    "unknown".to_string()
}

/// Normalize command text
fn normalize_command(cmd: &str) -> String {
    // Remove common prefixes and normalize
    cmd.replace("$ ", "").replace("> ", "").trim().to_string()
}
