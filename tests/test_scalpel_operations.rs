// Integration tests for Scalpel file operations
// These tests verify the core functionality without requiring full server setup

use surreal_mind::tools::scalpel::{parse_tool_call_json, resolve_path};
use serde_json::json;

#[test]
fn test_tool_call_parsing_standard_format() {
    let json_str = r#"{"name":"read_file","params":{"path":"/test"}}"#;
    let tool_call = parse_tool_call_json(json_str).unwrap();
    assert_eq!(tool_call.name, "read_file");
    assert_eq!(tool_call.params["path"], "/test");
}

#[test]
fn test_tool_call_parsing_legacy_format() {
    let json_str = r#"{"tool":"write_file","parameters":{"path":"/test","content":"hello"}}"#;
    let tool_call = parse_tool_call_json(json_str).unwrap();
    assert_eq!(tool_call.name, "write_file");
    assert_eq!(tool_call.params["path"], "/test");
    assert_eq!(tool_call.params["content"], "hello");
}

#[test]
fn test_tool_call_parsing_alternative_format() {
    let json_str = r#"{"tool_name":"append_file","arguments":{"path":"/test","content":"data"}}"#;
    let tool_call = parse_tool_call_json(json_str).unwrap();
    assert_eq!(tool_call.name, "append_file");
    assert_eq!(tool_call.params["path"], "/test");
    assert_eq!(tool_call.params["content"], "data");
}

#[test]
fn test_tool_call_parsing_missing_fields() {
    // Should handle missing params field gracefully
    let json_str = r#"{"name":"read_file"}"#;
    let tool_call = parse_tool_call_json(json_str);
    // With our improved parsing, this should now work and return empty params
    assert!(tool_call.is_some());
    let tool_call = tool_call.unwrap();
    assert_eq!(tool_call.name, "read_file");
    assert!(tool_call.params.as_object().unwrap().is_empty());
}

#[test]
fn test_path_resolution_absolute() {
    let abs_path = resolve_path("/absolute/path/test.txt");
    assert!(abs_path.to_string_lossy().contains("/absolute/path/test.txt"));
}

#[test]
fn test_path_resolution_relative() {
    let rel_path = resolve_path("relative/test.txt");
    let current_dir = std::env::current_dir().unwrap();
    let expected = current_dir.join("relative/test.txt");
    assert_eq!(rel_path, expected);
}

#[test]
fn test_path_resolution_with_dots() {
    let path_with_dots = resolve_path("./test.txt");
    let current_dir = std::env::current_dir().unwrap();
    let expected = current_dir.join("test.txt");
    assert_eq!(path_with_dots, expected);
}