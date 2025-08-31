//! KG extraction module for converting natural language text into structured knowledge graph components
//! Supports both heuristic and LLM-based extraction methods

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Data model for a single entity extracted from text
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl HeuristicExtractor {
    /// Create a new heuristic extractor
    pub fn new() -> Self {
        HeuristicExtractor
    }

    /// Extract entities from text using various heuristics
    fn extract_entities(&self, text: &str) -> Vec<ExtractedEntity> {
        let mut entities = Vec::new();

        // 1. Extract proper nouns (basic capitalization heuristic)
        for word in text.split_whitespace() {
            let word = word.trim_matches(|c: char| !c.is_alphanumeric());
            if word.len() >= 3 && word.chars().next().unwrap().is_uppercase() {
                // Skip common words that shouldn't be entities
                let common_words = ["The", "This", "That", "When", "What", "How", "Why", "Who"];
                if !common_words.contains(&word) {
                    entities.push(ExtractedEntity {
                        name: word.to_string(),
                        entity_type: self.classify_entity_type(word),
                        properties: serde_json::json!({}),
                        confidence: 0.7,
                    });
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

        // 3. Extract known vendor/brand keywords
        let vendor_keywords = [
            ("rust", "language"),
            ("cargo", "tool"),
            ("serde", "library"),
            ("tokio", "library"),
            ("surrealdb", "database"),
            ("surreal", "database"),
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

        // Deduplicate entities by name
        let mut seen = HashMap::new();
        entities.retain(|e| {
            if seen.contains_key(&e.name) {
                false
            } else {
                seen.insert(e.name.clone(), true);
                true
            }
        });

        entities
    }

    /// Classify entity type based on patterns
    fn classify_entity_type(&self, word: &str) -> String {
        let word_lower = word.to_lowercase();

        if word_lower.contains("ci") || word_lower.contains("cd") || word_lower.contains("pipeline")
        {
            "process".to_string()
        } else if word_lower.contains("error") || word_lower.contains("warning") {
            "issue".to_string()
        } else if word_lower.contains("function") || word_lower.contains("method") {
            "code_entity".to_string()
        } else {
            "person".to_string() // Default fallback
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

    /// Extract relationships using pattern matching
    fn extract_relationships(
        &self,
        text: &str,
        entities: &[ExtractedEntity],
    ) -> Vec<ExtractedRelationship> {
        let mut relationships = Vec::new();
        let text_lower = text.to_lowercase();

        // Relationship patterns
        let relation_patterns = [
            ("fixed", "fixed"),
            ("broke", "broke"),
            ("caused", "caused"),
            ("requires", "requires"),
            ("depends", "depends_on"),
            ("blocked", "blocked_by"),
            ("warned", "warned_about"),
            ("solved", "solved"),
            ("added", "added_to"),
            ("removed", "removed_from"),
            ("changed", "modified"),
        ];

        for (pattern, rel_type) in relation_patterns.iter() {
            if text_lower.contains(pattern) {
                // Simple heuristic: first entity before pattern, first after
                let parts: Vec<&str> = text.split(pattern).collect();
                if parts.len() >= 2 {
                    let source_part = parts[0].split_whitespace().last().unwrap_or("");
                    let target_part = parts[1].split_whitespace().next().unwrap_or("");

                    // Find matching entities
                    let source_entity = entities.iter().find(|e| e.name.contains(source_part));
                    let target_entity = entities.iter().find(|e| e.name.contains(target_part));

                    if let (Some(source), Some(target)) = (source_entity, target_entity) {
                        relationships.push(ExtractedRelationship {
                            source_name: source.name.clone(),
                            target_name: target.name.clone(),
                            rel_type: rel_type.to_string(),
                            properties: serde_json::json!({}),
                            confidence: 0.6,
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

        // Simple past tense verb detection (very basic)
        let past_verbs = [
            "fixed", "broke", "added", "removed", "changed", "created", "deleted", "updated",
        ];

        for verb in past_verbs.iter() {
            if text.to_lowercase().contains(verb) {
                // Find the object (words after the verb)
                let parts: Vec<&str> = text.split(verb).collect();
                if parts.len() >= 2 {
                    let object = parts[1]
                        .split_whitespace()
                        .take(3) // Take first few words as description
                        .collect::<Vec<&str>>()
                        .join(" ");

                    if !object.is_empty() {
                        events.push(ExtractedEvent {
                            description: format!("{} {}", verb, object.trim()),
                            entities_involved: vec![], // Would be populated by linking to extracted entities
                            timestamp: None,
                            confidence: 0.5,
                        });
                    }
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

        // Deduplicate entities across all texts
        let mut seen = HashMap::new();
        all_entities.retain(|e| {
            if seen.contains_key(&e.name) {
                false
            } else {
                seen.insert(e.name.clone(), true);
                true
            }
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
