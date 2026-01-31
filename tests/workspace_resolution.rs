use surreal_mind::workspace::{WorkspaceMap, resolve_workspace};

#[test]
fn test_call_cc_with_workspace_alias() {
    // Set up environment
    unsafe {
        std::env::set_var("WORKSPACE_TEST", "/tmp/test");
    }

    let map = WorkspaceMap::from_env();
    let resolved = resolve_workspace("test", &map).unwrap();

    assert_eq!(resolved, "/tmp/test");

    unsafe {
        std::env::remove_var("WORKSPACE_TEST");
    }
}

#[test]
fn test_backward_compatibility_absolute_paths() {
    let map = WorkspaceMap::from_env();

    // Absolute paths should still work
    let resolved = resolve_workspace("/var/local/foo", &map).unwrap();
    assert_eq!(resolved, "/var/local/foo");
}

#[test]
fn test_error_suggests_valid_aliases() {
    unsafe {
        std::env::set_var("WORKSPACE_SURREAL_MIND", "/path/to/surreal-mind");
    }

    let map = WorkspaceMap::from_env();
    let result = resolve_workspace("surreal", &map);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("surreal-mind"));
    assert!(err_msg.contains("Did you mean"));

    unsafe {
        std::env::remove_var("WORKSPACE_SURREAL_MIND");
    }
}