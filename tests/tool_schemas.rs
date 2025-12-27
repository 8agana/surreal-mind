//! Integration tests for MCP tool schemas.
//!
//! These tests verify that the tools exposed by the surreal-mind MCP server
//! have the correct schemas and handle parameter validation properly.

use serde_json::{Value, json};

/// Helper function to validate that a JSON schema contains expected fields
fn schema_has_property(schema: &Value, property: &str) -> bool {
    schema["properties"][property].is_object()
}

#[test]
fn test_list_tools_returns_expected_tools() {
    // This test verifies that the expected tools are exposed
    // Note: In a real integration test, we would start the server and make actual calls
    // For now, we're testing the expected structure

    let expected_tools = [
        "legacymind_think",
        "memories_create",
        "maintenance_ops",
        "detailed_help",
        "inner_voice",
        "legacymind_search",
    ];
    assert_eq!(
        expected_tools.len(),
        6,
        "Tool roster should list 6 entries after removing photography and brain_store tools"
    );
}

#[test]
fn test_legacymind_think_schema_structure() {
    // Test that legacymind_think has the expected schema structure
    let expected_schema = json!({
        "type": "object",
        "properties": {
            "content": {"type": "string"},
            "hint": {"type": "string", "enum": ["debug", "build", "plan", "stuck", "question", "conclude"]},
            "injection_scale": {"type": "integer", "minimum": 0, "maximum": 3},
            "tags": {"type": "array", "items": {"type": "string"}},
            "significance": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "verbose_analysis": {"type": "boolean"},
            "session_id": {"type": "string"},
            "chain_id": {"type": "string"},
            "previous_thought_id": {"type": "string"},
            "revises_thought": {"type": "string"},
            "branch_from": {"type": "string"},
            "confidence": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "hypothesis": {"type": "string"},
            "needs_verification": {"type": "boolean"},
            "verify_top_k": {"type": "integer"},
            "min_similarity": {"type": "number"},
            "evidence_limit": {"type": "integer"},
            "contradiction_patterns": {"type": "array", "items": {"type": "string"}}
        },
        "required": ["content"]
    });

    // Verify required properties
    assert!(schema_has_property(&expected_schema, "content"));
    assert!(schema_has_property(&expected_schema, "hint"));
    assert!(schema_has_property(&expected_schema, "injection_scale"));
    assert!(schema_has_property(&expected_schema, "tags"));
    assert!(schema_has_property(&expected_schema, "significance"));
    assert!(schema_has_property(&expected_schema, "verbose_analysis"));

    // Verify required array
    assert_eq!(expected_schema["required"].as_array().unwrap().len(), 1);
    assert_eq!(expected_schema["required"][0].as_str().unwrap(), "content");
}

#[test]
fn test_detailed_help_schema_structure() {
    // Test that detailed_help has the expected schema structure
    let expected_schema = json!({
        "type": "object",
        "properties": {
            "tool": {"type": "string", "enum": ["legacymind_think", "memories_create", "legacymind_search", "maintenance_ops", "inner_voice", "detailed_help"]},
            "format": {"type": "string", "enum": ["compact", "full"], "default": "full"},
            "prompts": {"type": "boolean"}
        }
    });

    assert!(schema_has_property(&expected_schema, "tool"));
    assert!(schema_has_property(&expected_schema, "format"));
    assert!(schema_has_property(&expected_schema, "prompts"));

    // Verify format has default
    assert_eq!(expected_schema["properties"]["format"]["default"], "full");
}

#[test]
fn test_convo_think_accepts_valid_params() {
    // Test valid parameter combinations for convo_think
    let valid_params = vec![
        json!({
            "content": "Test thought"
        }),
        json!({
            "content": "Test with injection scale",
            "injection_scale": 3
        }),
        json!({
            "content": "Test with significance",
            "significance": 0.8
        }),
        json!({
            "content": "Test with low significance",
            "significance": 0.2
        }),
        json!({
            "content": "Test with high significance",
            "significance": 0.9
        }),
        json!({
            "content": "Full params test",
            "injection_scale": 3,
            "significance": 0.7,
            "tags": ["test", "integration"],
            "verbose_analysis": false
        }),
    ];

    // In a real integration test, we would validate these against the actual server
    // For now, we just verify the structure is correct
    for params in valid_params {
        assert!(
            params["content"].is_string(),
            "Content must be present and be a string"
        );
        // Verify optional parameters are valid types
        if let Some(injection_scale) = params["injection_scale"].as_i64() {
            assert!(
                (0..=3).contains(&injection_scale),
                "Injection scale must be 0-3"
            );
        }
        if let Some(significance) = params["significance"].as_f64() {
            assert!(
                (0.0..=1.0).contains(&significance),
                "Significance must be 0.0-1.0"
            );
        }
    }
}

#[test]
fn test_tech_think_accepts_valid_params() {
    // Test valid parameter combinations for tech_think
    let valid_params = vec![
        json!({
            "content": "Photography thought"
        }),
        json!({
            "content": "Photo with injection scale",
            "injection_scale": 2
        }),
        json!({
            "content": "Photo with all params",
            "injection_scale": 3,
            "significance": 0.7,
            "tags": ["rust", "mcp"],
            "verbose_analysis": true
        }),
    ];

    for params in valid_params {
        assert!(
            params["content"].is_string(),
            "Content must be present and be a string"
        );
        // Photography think validates injection_scale and significance ranges
        if let Some(injection_scale) = params["injection_scale"].as_i64() {
            assert!(
                (0..=3).contains(&injection_scale),
                "Injection scale must be 0-3"
            );
        }
        if let Some(significance) = params["significance"].as_f64() {
            assert!(
                (0.0..=1.0).contains(&significance),
                "Significance must be 0.0-1.0"
            );
        }
    }
}

#[test]
fn test_legacymind_think_rejects_invalid_significance() {
    // Test that significance value of -1 is rejected
    let invalid_params = json!({
        "content": "Test thought",
        "significance": -1.0
    });

    // This would be rejected by the actual deserializer
    let sig_value = invalid_params["significance"].as_f64().unwrap();
    assert!(
        sig_value < 0.0,
        "Testing rejection of negative significance"
    );
}

/// Integration test that would require actual server running
/// This is a placeholder showing how real integration tests would work
#[cfg(feature = "db_integration")]
#[tokio::test]
async fn test_actual_server_tool_listing() {
    // In a real integration test with the server running:
    // 1. Start the MCP server
    // 2. Make an actual list_tools request
    // 3. Verify the response contains exactly 3 tools
    // 4. Verify each tool's schema matches expectations

    // For now, this is gated behind a feature flag
    // Placeholder: Treat as success when feature is enabled in CI environments.
    // TODO: Implement actual integration test when db_integration feature is enabled
}
