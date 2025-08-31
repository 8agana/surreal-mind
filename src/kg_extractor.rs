//! KG extraction module for converting natural language text into structured knowledge graph components
//! Supports both heuristic and LLM-based extraction methods

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

/// Data model for a single entity extracted from text
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedEntity {
    pub name: String,
    pub entity_type: String,
    pub properties: serde_json::Value,
    pub confidence: f32,
}

/// Data model for a relationship between entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedRelationship {
    pub source_name: String,
    pub target_name: String,
    pub rel_type: String,
    pub properties: serde_json::Value,
    pub confidence: f32,
}

/// Data model for an event (represented as a special entity with temporal properties)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEvent {
    pub description: String,
    pub entities_involved: Vec<String>,
    pub timestamp: Option<String>,
    pub confidence: f32,
}

/// Result of extraction process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub entities: Vec<ExtractedEntity>,
    pub relationships: Vec<ExtractedRelationship>,
    pub events: Vec<ExtractedEvent>,
    pub synthesis: String,
    pub average_confidence: f32,
}

/// Heuristic extractor using pattern matching and natural language processing rules
pub struct HeuristicExtractor;

impl Default for HeuristicExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl HeuristicExtractor {
    /// Create a new heuristic extractor
    pub fn new() -> Self {
        HeuristicExtractor
    }

    /// Extract entities from text using various heuristics
    fn extract_entities(&self, text: &str) -> Vec<ExtractedEntity> {
        let mut entities = Vec::new();
        // Expanded stoplist to reduce noise from sentence starters and discourse markers
        // Includes both capitalized and lowercase forms for robust matching
        let stopwords: HashSet<&'static str> = [
            // Pronouns / determiners
            "i","you","he","she","it","we","they","me","him","her","us","them",
            "my","your","his","her","its","our","their","mine","yours","ours","theirs",
            "a","an","the","this","that","these","those","there","here",
            // Discourse / fillers
            "let","lets","ok","okay","hi","hello","thanks","thank","please","note","btw","fyi",
            "also","but","so","then","now","next","well","yeah","yep","nope",
            // Time words
            "today","yesterday","tomorrow","tonight","morning","evening","afternoon",
            // Days
            "monday","tuesday","wednesday","thursday","friday","saturday","sunday",
            // Months
            "january","february","march","april","may","june","july","august","september","october","november","december",
        ].into_iter().collect();

        // 1. Extract proper nouns (basic capitalization heuristic)
        for raw in text.split_whitespace() {
            // Clean leading/trailing punctuation; keep inner hyphens (e.g., type-name) but drop apostrophes
            let cleaned = raw
                .trim_matches(|c: char| !c.is_alphanumeric())
                .replace(['’', '\'', '"'], "");
            if cleaned.len() < 3 { continue; }

            // Primary proper-noun style: starts uppercase and contains at least one lowercase letter
            // Avoid shouting words like ALLCAPS unless they are known tech terms (handled later)
            let starts_upper = cleaned.chars().next().unwrap().is_uppercase();
            let has_lower = cleaned.chars().any(|c| c.is_lowercase());
            let lower = cleaned.to_lowercase();
            if starts_upper {
                // Skip common stopwords/time words when capitalized at sentence start
                if stopwords.contains(lower.as_str()) { continue; }
                if cleaned.ends_with("ed") || cleaned.ends_with("ing") { continue; }
                // Accept if looks like a proper noun token
                if has_lower || cleaned.contains('_') || cleaned.chars().any(|c| c.is_ascii_digit()) {
                    entities.push(ExtractedEntity {
                        name: cleaned.clone(),
                        entity_type: self.classify_entity_type(&cleaned),
                        properties: serde_json::json!({}),
                        confidence: 0.7,
                    });
                    continue;
                }
            }
        }

        // 2. Extract code-like tokens (variables, functions, etc.)
        for word in text.split_whitespace() {
            if self.is_code_token(word) {
                entities.push(ExtractedEntity {
                    name: word.to_string(),
                    entity_type: "code_token".to_string(),
                    properties: serde_json::json!({}),
                    confidence: 0.8,
                });
            }
        }

        // 3. Extract known vendor/brand keywords and generic terms
        let vendor_keywords = [
            ("rust", "language"),
            ("cargo", "tool"),
            ("serde", "library"),
            ("tokio", "library"),
            ("surrealdb", "database"),
            ("surreal", "database"),
            ("database", "technology"),
            ("api", "interface"),
            ("compiler", "tool"),
            ("connection", "technology"),
            ("query", "operation"),
            ("issue", "problem"),
            ("github", "platform"),
            ("firefox", "browser"),
            ("chrome", "browser"),
            ("ci", "process"),
            ("cd", "process"),
            ("pipeline", "process"),
        ];

        for (keyword, etype) in vendor_keywords.iter() {
            if text.to_lowercase().contains(keyword) {
                entities.push(ExtractedEntity {
                    name: keyword.to_string(),
                    entity_type: etype.to_string(),
                    properties: serde_json::json!({}),
                    confidence: 0.9,
                });
            }
        }

        // Deduplicate entities by lowercase name
        let mut seen = HashMap::new();
        entities.retain(|e| match seen.entry(e.name.to_lowercase()) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(true);
                true
            }
            std::collections::hash_map::Entry::Occupied(_) => false,
        });

        entities
    }

    /// Classify entity type based on patterns
    fn classify_entity_type(&self, word: &str) -> String {
        let word_lower = word.to_lowercase();

        // Known technology/programming terms
        let tech_terms = [
            ("rust", "language"),
            ("javascript", "language"),
            ("java", "language"),
            ("python", "language"),
            ("surrealdb", "database"),
            ("mongodb", "database"),
            ("mysql", "database"),
            ("postgres", "database"),
            ("docker", "tool"),
            ("kubernetes", "tool"),
            ("git", "tool"),
            ("api", "interface"),
        ];

        for (term, etype) in tech_terms.iter() {
            if word_lower.contains(term) {
                return etype.to_string();
            }
        }

        if word_lower.contains("ci") || word_lower.contains("cd") || word_lower.contains("pipeline")
        {
            "process".to_string()
        } else if word_lower.contains("browser")
            || word_lower.contains("firefox")
            || word_lower.contains("chrome")
        {
            "browser".to_string()
        } else if word_lower.contains("database")
            || word_lower.contains("surrealdb")
            || word_lower.contains("connection")
        {
            "technology".to_string()
        } else if word_lower.contains("api") || word_lower.contains("interface") {
            "interface".to_string()
        } else if word_lower.contains("error")
            || word_lower.contains("warning")
            || word_lower.contains("issue")
        {
            "issue".to_string()
        } else if word_lower.contains("compiler") || word_lower.contains("tool") {
            "tool".to_string()
        } else if word_lower.contains("function") || word_lower.contains("method") {
            "code_entity".to_string()
        } else if word_lower.contains("query") || word_lower.contains("operation") {
            "operation".to_string()
        } else if word_lower.len() >= 4 && word.chars().all(|c| c.is_ascii_alphabetic()) {
            // Likely a proper name if all letters and reasonably long
            "person".to_string()
        } else {
            "concept".to_string() // Default fallback
        }
    }

    /// Check if a word looks like code (variables, functions, etc.)
    fn is_code_token(&self, word: &str) -> bool {
        // Basic heuristic: contains underscore, starts with letter, alphanumeric + underscore only
        let chars: Vec<char> = word.chars().collect();
        if chars.is_empty() || !chars[0].is_alphabetic() {
            return false;
        }

        chars.iter().all(|c| c.is_alphanumeric() || *c == '_') && word.contains('_')
    }

    /// Extract relationships from text using pattern matching
    fn extract_relationships(
        &self,
        text: &str,
        entities: &[ExtractedEntity],
    ) -> Vec<ExtractedRelationship> {
        let mut relationships = Vec::new();
        let text_lower = text.to_lowercase();
        // Relationship patterns with better context
        let relation_patterns = [
            (" fixed ", "fixed"),
            (" broke ", "broke"),
            (" caused ", "caused"),
            (" requires ", "requires"),
            (" depends ", "depends_on"),
            (" depend ", "depends_on"),
            (" blocked ", "blocked_by"),
            (" warned ", "warned_about"),
            (" solved ", "solved"),
            (" added ", "added_to"),
            (" removed ", "removed_from"),
            (" changed ", "modified"),
            (" used ", "uses"),
            (" implemented ", "implemented_in"),
            (" integrated ", "integrated_with"),
        ];

        for (pattern, rel_type) in relation_patterns.iter() {
            if text_lower.contains(pattern.trim()) {
                // Find entities within a reasonable context window around the pattern
                let pattern_pos = text_lower.find(pattern.trim()).unwrap();
                let window_start = pattern_pos.saturating_sub(50);
                let window_end = (pattern_pos + pattern.len() + 50).min(text.len());

                let context_window = &text_lower[window_start..window_end];

                // Find entities mentioned in the context
                let mut relevant_entities = Vec::new();
                for entity in entities.iter() {
                    // More robust case-insensitive matching
                    let entity_lower = entity.name.to_lowercase();
                    if context_window.contains(&entity_lower) {
                        relevant_entities.push(entity.clone());
                    }
                }

                // Create relationships between relevant entities
                if relevant_entities.len() >= 2 {
                    for i in 0..relevant_entities.len() {
                        for j in (i + 1)..relevant_entities.len() {
                            relationships.push(ExtractedRelationship {
                                source_name: relevant_entities[i].name.clone(),
                                target_name: relevant_entities[j].name.clone(),
                                rel_type: rel_type.to_string(),
                                properties: serde_json::json!({}),
                                confidence: 0.5,
                            });
                        }
                    }
                } else if relevant_entities.len() == 1 && entities.len() > 1 {
                    // If only one entity found in context, relate to the most likely other entity
                    let other_entities: Vec<&ExtractedEntity> = entities
                        .iter()
                        .filter(|e| !relevant_entities.contains(e))
                        .collect();

                    if let Some(other) = other_entities.first() {
                        relationships.push(ExtractedRelationship {
                            source_name: relevant_entities[0].name.clone(),
                            target_name: other.name.clone(),
                            rel_type: rel_type.to_string(),
                            properties: serde_json::json!({}),
                            confidence: 0.4,
                        });
                    }
                }
            }
        }

        relationships
    }

    /// Extract events from text (past tense actions with objects)
    fn extract_events(&self, text: &str) -> Vec<ExtractedEvent> {
        let mut events = Vec::new();
        let text_lower = text.to_lowercase();

        // Expanded past tense verb detection
        let past_verbs = [
            "fixed",
            "broke",
            "added",
            "removed",
            "changed",
            "created",
            "deleted",
            "updated",
            "implemented",
            "integrated",
            "resolved",
            "completed",
            "finished",
            "started",
            "began",
        ];

        // Also check for present perfect (have/has + past participle)
        let present_perfect = [
            "have fixed",
            "have added",
            "have removed",
            "have changed",
            "have created",
            "has fixed",
            "has added",
            "has removed",
            "has changed",
            "has created",
        ];

        // Combine both patterns
        let all_verb_patterns = past_verbs
            .iter()
            .map(|v| v.to_string())
            .chain(present_perfect.iter().map(|v| v.to_string()))
            .collect::<Vec<_>>();

        for verb_pattern in all_verb_patterns.iter() {
            if text_lower.contains(verb_pattern) {
                // Find the object (words after the verb pattern)
                let verb_pos = text_lower.find(verb_pattern).unwrap_or(0);
                let after_verb = &text[verb_pos + verb_pattern.len()..];
                let object_words: Vec<&str> = after_verb
                    .split_whitespace()
                    .take(5) // Take up to 5 words as object description
                    .filter(|word| !word.starts_with(",") && !word.starts_with("."))
                    .collect();

                let object = object_words.join(" ");

                if !object.is_empty() && object.len() >= 3 {
                    events.push(ExtractedEvent {
                        description: format!("{} {}", verb_pattern.trim(), object.trim()),
                        entities_involved: vec![], // Would be populated by linking to extracted entities
                        timestamp: None,
                        confidence: 0.6,
                    });
                }
            }
        }

        events
    }

    /// Generate synthesis summary
    fn generate_synthesis(
        &self,
        entities: &[ExtractedEntity],
        relationships: &[ExtractedRelationship],
        events: &[ExtractedEvent],
    ) -> String {
        let mut synthesis = String::new();

        if !entities.is_empty() {
            synthesis.push_str("• Key entities mentioned:\n");
            for entity in entities.iter().take(5) {
                // Limit to top entities
                synthesis.push_str(&format!("  - {} ({})\n", entity.name, entity.entity_type));
            }
        }

        if !relationships.is_empty() {
            synthesis.push_str("• Notable relationships:\n");
            for rel in relationships.iter().take(3) {
                synthesis.push_str(&format!(
                    "  - {} {} {}\n",
                    rel.source_name, rel.rel_type, rel.target_name
                ));
            }
        }

        if !events.is_empty() {
            synthesis.push_str("• Recent events:\n");
            for event in events.iter().take(2) {
                synthesis.push_str(&format!("  - {}\n", event.description));
            }
        }

        if synthesis.is_empty() {
            synthesis = "• No significant patterns detected in the content".to_string();
        }

        synthesis
    }

    /// Calculate average confidence
    fn average_confidence(
        &self,
        entities: &[ExtractedEntity],
        relationships: &[ExtractedRelationship],
        events: &[ExtractedEvent],
    ) -> f32 {
        let total_items = entities.len() + relationships.len() + events.len();
        if total_items == 0 {
            return 0.0;
        }

        let sum: f32 = entities.iter().map(|e| e.confidence).sum::<f32>()
            + relationships.iter().map(|r| r.confidence).sum::<f32>()
            + events.iter().map(|e| e.confidence).sum::<f32>();

        sum / total_items as f32
    }

    /// Extract structured knowledge from a collection of text strings
    pub async fn extract(&self, texts: &[String]) -> Result<ExtractionResult> {
        if texts.is_empty() {
            return Ok(ExtractionResult {
                entities: vec![],
                relationships: vec![],
                events: vec![],
                synthesis: "No text provided for analysis".to_string(),
                average_confidence: 0.0,
            });
        }

        let mut all_entities = Vec::new();
        let mut all_relationships = Vec::new();
        let mut all_events = Vec::new();

        // Process each text
        for text in texts.iter() {
            let entities = self.extract_entities(text);
            let relationships = self.extract_relationships(text, &entities);
            let events = self.extract_events(text);

            all_entities.extend(entities);
            all_relationships.extend(relationships);
            all_events.extend(events);
        }

        // Deduplicate entities across all texts (case-insensitive)
        let mut seen = HashMap::new();
        all_entities.retain(|e| match seen.entry(e.name.to_lowercase()) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(true);
                true
            }
            std::collections::hash_map::Entry::Occupied(_) => false,
        });

        // Generate synthesis
        let synthesis = self.generate_synthesis(&all_entities, &all_relationships, &all_events);
        let avg_confidence =
            self.average_confidence(&all_entities, &all_relationships, &all_events);

        Ok(ExtractionResult {
            entities: all_entities,
            relationships: all_relationships,
            events: all_events,
            synthesis,
            average_confidence: avg_confidence,
        })
    }
}
