//! DB-free exact-ten-name roster test.
//!
//! Distinct from `tests/tool_schemas.rs`'s `test_list_tools_returns_expected_tools`,
//! which is a hand-maintained array asserted against itself (synthetic, not a
//! witness of the server's actual registration path). This test instead
//! calls `surreal_mind::server::router::build_tool_list()` directly — the
//! exact same pure function the real `tools/list` wire handler
//! (`SurrealMindServer::list_tools` in `src/server/router.rs`) delegates to.
//! It requires no `SurrealMindServer` instance, no live database, and no
//! `db_integration` feature: `build_tool_list()` touches only
//! `crate::schemas::*` and `rmcp::model::Tool`, nothing stateful.

use surreal_mind::server::router::build_tool_list;

const EXPECTED_TOOL_NAMES: [&str; 10] = [
    "think",
    "wander",
    "maintain",
    "journal",
    "rethink",
    "corrections",
    "test_notification",
    "remember",
    "howto",
    "search",
];

#[test]
fn tool_roster_is_exactly_ten_names_in_registration_order() {
    let tools = build_tool_list();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();

    assert_eq!(
        names.len(),
        10,
        "build_tool_list() must return exactly 10 tools, got: {names:?}"
    );
    assert_eq!(
        names, EXPECTED_TOOL_NAMES,
        "tool roster must match the exact expected set, in order"
    );

    // Negative control: none of the six removed delegation/job-management
    // tool names may reappear here (fed-734b8f steps 2-5).
    for removed in [
        "call_gem",
        "call_cc",
        "call_vibe",
        "call_status",
        "call_jobs",
        "call_cancel",
    ] {
        assert!(
            !names.contains(&removed),
            "removed tool {removed:?} must not be present in the roster"
        );
    }
}
