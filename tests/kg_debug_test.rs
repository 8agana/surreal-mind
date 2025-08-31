//! Debug test for KG relationship creation issues
//! This test helps identify why relationships are extracted but not created in DB

use serde_json::json;
use std::env;
use surreal_mind::*;
use tokio;

/// Debug test for relationship creation flow
#[tokio::test]
async fn debug_kg_relationship_creation() {
    println!("🧪 DEBUG: Starting KG relationship creation debugging");

    // Create test extractor
    let extractor = surreal_mind::kg_extractor::HeuristicExtractor::new();

    // Use a simple test text that should produce predictable results
    let test_text = vec![
        "Sam fixed the database connection issue".to_string(),
        "The Rust compiler was updated successfully".to_string(),
        "Database queries now work correctly".to_string(),
    ];

    println!("📝 DEBUG: Test text: {:?}", test_text);

    // Extract knowledge
    let extraction_result = extractor.extract(&test_text).await.unwrap();
    println!("🔍 DEBUG: Extraction result:");
    println!(
        "  - Entities extracted: {}",
        extraction_result.entities.len()
    );
    println!(
        "  - Relationships extracted: {}",
        extraction_result.relationships.len()
    );

    // Show all entities
    println!("📋 DEBUG: Extracted entities:");
    for (i, entity) in extraction_result.entities.iter().enumerate() {
        println!(
            "  [{}] '{}' (type: {}, confidence: {:.2})",
            i, entity.name, entity.entity_type, entity.confidence
        );
    }

    // Show all relationships
    println!("🔗 DEBUG: Extracted relationships:");
    for (i, rel) in extraction_result.relationships.iter().enumerate() {
        println!(
            "  [{}] '{}' -> '{}' [{}] (confidence: {:.2})",
            i, rel.source_name, rel.target_name, rel.rel_type, rel.confidence
        );
    }

    // Test entity matching logic (the likely issue)
    println!("🎯 DEBUG: Testing entity matching logic:");
    for rel in &extraction_result.relationships {
        println!(
            "  Checking relationship: '{}' -> '{}' [{}]",
            rel.source_name, rel.target_name, rel.rel_type
        );

        // Test exact name matching
        let source_match_exact = extraction_result
            .entities
            .iter()
            .find(|e| e.name == rel.source_name);
        let target_match_exact = extraction_result
            .entities
            .iter()
            .find(|e| e.name == rel.target_name);

        println!("    Exact source match: {}", source_match_exact.is_some());
        println!("    Exact target match: {}", target_match_exact.is_some());

        // Test case-insensitive matching
        let source_match_case = extraction_result
            .entities
            .iter()
            .find(|e| e.name.to_lowercase() == rel.source_name.to_lowercase());
        let target_match_case = extraction_result
            .entities
            .iter()
            .find(|e| e.name.to_lowercase() == rel.target_name.to_lowercase());

        println!(
            "    Case-insensitive source match: {}",
            source_match_case.is_some()
        );
        if source_match_case.is_some() {
            println!("      Source entity: '{}'", source_match_case.unwrap().name);
        }
        println!(
            "    Case-insensitive target match: {}",
            target_match_case.is_some()
        );
        if target_match_case.is_some() {
            println!("      Target entity: '{}'", target_match_case.unwrap().name);
        }

        // Check if matching would succeed
        let would_succeed = source_match_case.is_some() && target_match_case.is_some();
        println!("    Would relationship creation succeed: {}", would_succeed);
        println!("");
    }

    // Test confidence filtering
    let confidence_min = 0.5;
    let filtered_entities: Vec<_> = extraction_result
        .entities
        .iter()
        .filter(|e| e.confidence >= confidence_min)
        .collect();

    let filtered_relationships: Vec<_> = extraction_result
        .relationships
        .iter()
        .filter(|r| r.confidence >= confidence_min)
        .collect();

    println!(
        "🎚️  DEBUG: After confidence filtering (min: {:.2}):",
        confidence_min
    );
    println!(
        "  - Entities: {} -> {}",
        extraction_result.entities.len(),
        filtered_entities.len()
    );
    println!(
        "  - Relationships: {} -> {}",
        extraction_result.relationships.len(),
        filtered_relationships.len()
    );

    // Test the full relationship creation simulation
    println!("🧪 DEBUG: Simulating relationship creation process:");
    let mut relationship_count = 0;

    for rel in &filtered_relationships {
        // Find entity IDs (simulated)
        let source_match = filtered_entities
            .iter()
            .enumerate()
            .find(|(_, e)| e.name.to_lowercase() == rel.source_name.to_lowercase());

        let target_match = filtered_entities
            .iter()
            .enumerate()
            .find(|(_, e)| e.name.to_lowercase() == rel.target_name.to_lowercase());

        if let (Some((source_idx, source_entity)), Some((target_idx, target_entity))) =
            (source_match, target_match)
        {
            println!(
                "  ✅ Would create relationship: '{}' ({}) -> '{}' ({}) [{}]",
                source_entity.name, source_idx, target_entity.name, target_idx, rel.rel_type
            );
            relationship_count += 1;
        } else {
            println!(
                "  ❌ Would skip relationship '{}' -> '{}' (entity not found)",
                rel.source_name, rel.target_name
            );
        }
    }

    println!("🎉 DEBUG: Simulation results:");
    println!(
        "  - Entities available for matching: {}",
        filtered_entities.len()
    );
    println!(
        "  - Relationships that would be created: {}",
        relationship_count
    );
    println!(
        "  - Success rate: {:.1}%",
        if !filtered_relationships.is_empty() {
            (relationship_count as f32 / filtered_relationships.len() as f32) * 100.0
        } else {
            0.0
        }
    );

    // Assertions
    assert!(
        !extraction_result.entities.is_empty(),
        "Should extract at least some entities"
    );
    assert!(
        !extraction_result.relationships.is_empty(),
        "Should extract at least some relationships"
    );

    // Key diagnostic: Check if the issue is in entity matching
    let total_relationships = extraction_result.relationships.len();
    let matching_relationships = filtered_relationships
        .iter()
        .filter(|rel| {
            filtered_entities
                .iter()
                .any(|e| e.name.to_lowercase() == rel.source_name.to_lowercase())
                && filtered_entities
                    .iter()
                    .any(|e| e.name.to_lowercase() == rel.target_name.to_lowercase())
        })
        .count();

    let match_rate = if total_relationships > 0 {
        (matching_relationships as f32 / total_relationships as f32) * 100.0
    } else {
        100.0
    };

    println!("📊 DEBUG: Entity matching diagnostics:");
    println!("  - Total relationships: {}", total_relationships);
    println!(
        "  - Relationships with matching entities: {}",
        matching_relationships
    );
    println!("  - Entity match rate: {:.1}%", match_rate);

    if match_rate < 80.0 {
        println!("⚠️  WARNING: Low entity match rate may indicate relationship creation issues");
        println!("   This suggests that extracted relationship source/target names");
        println!("   don't match the extracted entity names.");
    } else {
        println!("✅ Entity matching looks good - focus on database creation logic");
    }
}

/// Test with specific problematic case from user report
#[tokio::test]
async fn debug_specific_user_case() {
    println!("👤 DEBUG: Testing specific user case 'Sam fixed surreal'");

    let extractor = surreal_mind::kg_extractor::HeuristicExtractor::new();
    let test_text = vec![
        "Sam fixed the surreal database connection".to_string(),
        "RAM usage has been optimized".to_string(),
    ];

    let extraction_result = extractor.extract(&test_text).await.unwrap();

    println!("🔍 User case results:");
    println!("  - Entities: {}", extraction_result.entities.len());
    for entity in &extraction_result.entities {
        println!("    '{}' (type: {})", entity.name, entity.entity_type);
    }

    println!(
        "  - Relationships: {}",
        extraction_result.relationships.len()
    );
    for rel in &extraction_result.relationships {
        println!(
            "    '{}' -> '{}' [{}]",
            rel.source_name, rel.target_name, rel.rel_type
        );

        // Check if entities match
        let source_exists = extraction_result
            .entities
            .iter()
            .any(|e| e.name.to_lowercase() == rel.source_name.to_lowercase());
        let target_exists = extraction_result
            .entities
            .iter()
            .any(|e| e.name.to_lowercase() == rel.target_name.to_lowercase());

        println!(
            "      Source '{}' exists: {}",
            rel.source_name, source_exists
        );
        println!(
            "      Target '{}' exists: {}",
            rel.target_name, target_exists
        );
        println!(
            "      Relationship would create: {}",
            source_exists && target_exists
        );
    }

    // Check for the specific "Sam fixed surreal" relationship
    let sam_surreal_rel = extraction_result.relationships.iter().find(|r| {
        r.source_name.to_lowercase().contains("sam")
            && r.target_name.to_lowercase().contains("surreal")
    });

    if let Some(rel) = sam_surreal_rel {
        println!("🎯 Found 'Sam fixed surreal' pattern:");
        println!(
            "  Source: '{}', Target: '{}', Type: '{}'",
            rel.source_name, rel.target_name, rel.rel_type
        );
    } else {
        println!("❌ 'Sam fixed surreal' pattern not found");
        println!("   Available relationships:");
        for rel in &extraction_result.relationships {
            println!(
                "   - '{}' -> '{}' [{}]",
                rel.source_name, rel.target_name, rel.rel_type
            );
        }
    }
}

/// Test the database relationship creation workflow
#[tokio::test]
async fn debug_database_relationship_workflow() {
    println!("🗄️ DEBUG: Testing database relationship creation workflow");

    // This test would need actual database access to be meaningful
    // For now, we'll simulate the workflow

    let mock_entities = vec![
        ("sam", "person"),
        ("surreal", "database"),
        ("rust", "language"),
    ];

    let mock_relationships = vec![
        ("sam", "surreal", "fixed"),
        ("rust", "surreal", "integrates_with"),
    ];

    println!("🎭 Simulating database workflow:");
    println!("  Mock entities: {:?}", mock_entities);
    println!("  Mock relationships: {:?}", mock_relationships);

    let mut simulated_creations = 0;
    let mut simulated_skips = 0;

    for (source, target, rel_type) in &mock_relationships {
        // Simulate entity ID lookup
        let source_exists = mock_entities.iter().any(|(name, _)| name == source);
        let target_exists = mock_entities.iter().any(|(name, _)| name == target);

        if source_exists && target_exists {
            println!("  ✅ Would create: {} -> {} [{}]", source, target, rel_type);
            simulated_creations += 1;
        } else {
            println!(
                "  ❌ Would skip: {} -> {} [{}] (missing entities)",
                source, target, rel_type
            );
            simulated_skips += 1;
        }
    }

    println!("📊 Workflow simulation results:");
    println!("  - Relationships attempted: {}", mock_relationships.len());
    println!("  - Would succeed: {}", simulated_creations);
    println!("  - Would fail: {}", simulated_skips);

    if simulated_creations > 0 {
        println!("✅ Database workflow simulation successful");
        println!("   If real relationships aren't being created, check:");
        println!("   - Database connection issues");
        println!("   - Schema definition problems");
        println!("   - SurrealDB query syntax");
    } else {
        println!("❌ Database workflow simulation failed");
        println!("   Relationships would be skipped - check entity extraction logic");
    }
}
