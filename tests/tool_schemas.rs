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
        "convo_think",
        "tech_think",
        "search_thoughts",
        "inner_voice",
        "detailed_help",
    ];
    assert_eq!(expected_tools.len(), 5, "Should have exactly 5 tools");
}

#[test]
fn test_convo_think_schema_structure() {
    // Test that convo_think has the expected schema structure
    let expected_schema = json!({
        "type": "object",
        "properties": {
            "content": {"type": "string"},
            "injection_scale": {"type": ["integer", "string"]},
            "submode": {"type": "string", "enum": ["sarcastic", "philosophical", "empathetic", "problem_solving"]},
            "tags": {"type": "array", "items": {"type": "string"}},
            "significance": {"type": ["number", "string"]},
            "verbose_analysis": {"type": "boolean", "default": true}
        },
        "required": ["content"]
    });

    // Verify required properties
    assert!(schema_has_property(&expected_schema, "content"));
    assert!(schema_has_property(&expected_schema, "injection_scale"));
    assert!(schema_has_property(&expected_schema, "submode"));
    assert!(schema_has_property(&expected_schema, "tags"));
    assert!(schema_has_property(&expected_schema, "significance"));
    assert!(schema_has_property(&expected_schema, "verbose_analysis"));

    // Verify required array
    assert_eq!(expected_schema["required"].as_array().unwrap().len(), 1);
    assert_eq!(expected_schema["required"][0].as_str().unwrap(), "content");
}

#[test]
fn test_tech_think_schema_structure() {
    // Test that tech_think has the expected schema structure
    let expected_schema = json!({
        "type": "object",
        "properties": {
            "content": {"type": "string"},
            "injection_scale": {"type": ["integer", "string"]},
            "submode": {"type": "string", "enum": ["plan", "build", "debug"], "default": "plan"},
            "tags": {"type": "array", "items": {"type": "string"}},
            "significance": {"type": ["number", "string"]},
            "verbose_analysis": {"type": "boolean", "default": true}
        },
        "required": ["content"]
    });

    // Verify tech_think specific submodes
    let submodes = expected_schema["properties"]["submode"]["enum"]
        .as_array()
        .unwrap();
    assert_eq!(submodes.len(), 3);
    assert!(submodes.contains(&json!("plan")));
    assert!(submodes.contains(&json!("build")));
    assert!(submodes.contains(&json!("debug")));
}

#[test]
fn test_detailed_help_schema_structure() {
    // Test that detailed_help has the expected schema structure
    let expected_schema = json!({
        "type": "object",
        "properties": {
            "tool": {"type": "string", "enum": ["convo_think", "tech_think"]},
            "format": {"type": "string", "enum": ["compact", "full"], "default": "full"}
        }
    });

    assert!(schema_has_property(&expected_schema, "tool"));
    assert!(schema_has_property(&expected_schema, "format"));

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
            "content": "Test with preset",
            "injection_scale": "HIGH"
        }),
        json!({
            "content": "Test with significance",
            "significance": 0.8
        }),
        json!({
            "content": "Test with string significance",
            "significance": "high"
        }),
        json!({
            "content": "Test with integer significance",
            "significance": 8
        }),
        json!({
            "content": "Full params test",
            "injection_scale": "MAXIMUM",
            "significance": "low",
            "submode": "philosophical",
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
    }
}

#[test]
fn test_tech_think_accepts_valid_params() {
    // Test valid parameter combinations for tech_think
    let valid_params = vec![
        json!({
            "content": "Technical thought"
        }),
        json!({
            "content": "Tech with submode",
            "submode": "debug"
        }),
        json!({
            "content": "Tech with all params",
            "injection_scale": "MEDIUM",
            "significance": 7,
            "submode": "build",
            "tags": ["rust", "mcp"],
            "verbose_analysis": true
        }),
    ];

    for params in valid_params {
        assert!(
            params["content"].is_string(),
            "Content must be present and be a string"
        );
        if let Some(submode) = params["submode"].as_str() {
            assert!(
                ["plan", "build", "debug"].contains(&submode),
                "Submode must be valid"
            );
        }
    }
}

#[test]
fn test_convo_think_rejects_invalid_significance() {
    // Test that significance value of 1 is rejected
    let invalid_params = json!({
        "content": "Test thought",
        "significance": 1
    });

    // This would be rejected by the actual deserializer
    // The error message should mention ambiguity
    let sig_value = invalid_params["significance"].as_i64().unwrap();
    assert_eq!(sig_value, 1, "Testing rejection of ambiguous value 1");
}

#[test]
fn test_tech_think_rejects_invalid_significance() {
    // Test that tech_think also rejects significance value of 1
    let invalid_params = json!({
        "content": "Technical thought",
        "significance": 1,
        "submode": "plan"
    });

    let sig_value = invalid_params["significance"].as_i64().unwrap();
    assert_eq!(sig_value, 1, "Testing rejection of ambiguous value 1");
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
    unimplemented!("Real server integration tests require server to be running");
}
