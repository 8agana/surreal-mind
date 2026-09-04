//! DB-free test that the `howto` tool's `tool` parameter enum agrees with
//! the server's actual live tool roster (fed-734b8f, Codex re-review #172
//! item 1: `howto_schema()`'s enum had omitted `test_notification` even
//! though the handler's match arm for it already existed at
//! `src/tools/howto.rs:248`).
//!
//! Why compare against the roster rather than invoking
//! `SurrealMindServer::handle_howto` directly for each enum value: that
//! handler requires a live `SurrealMindServer`, and
//! `SurrealMindServer::new()` requires a live database connection even for
//! DB-free-looking operations (see `tests/stdio_smoke.rs`'s comment:
//! "`SurrealMindServer::new` needs a live database even for stdio"). There
//! is no DB-free constructor for `SurrealMindServer`, so real dispatch is
//! not invokable here.
//!
//! Instead this test reads BOTH sides from real production sources and
//! compares them — not a second hardcoded list asserted against itself:
//!   - `surreal_mind::schemas::howto_schema()` — the exact function used to
//!     build the wire schema for the `howto` tool.
//!   - `surreal_mind::server::router::build_tool_list()` — the exact pure
//!     function the real `tools/list` wire handler
//!     (`SurrealMindServer::list_tools`) delegates to, and the same DB-free
//!     witness `tests/tool_roster_db_free.rs` uses for "what tools this
//!     server actually exposes".
//!
//! The `howto` tool's `tool` enum exists so a caller can ask for help about
//! any tool the server exposes, so the two sets must be exactly equal.

use std::collections::BTreeSet;

use surreal_mind::schemas::howto_schema;
use surreal_mind::server::router::build_tool_list;

#[test]
fn howto_tool_enum_matches_live_tool_roster() {
    let schema = howto_schema();

    let enum_values: BTreeSet<String> = schema
        .get("properties")
        .and_then(|p| p.get("tool"))
        .and_then(|t| t.get("enum"))
        .and_then(|e| e.as_array())
        .expect("howto_schema() must have properties.tool.enum as an array")
        .iter()
        .map(|v| {
            v.as_str()
                .expect("howto tool enum values must be strings")
                .to_string()
        })
        .collect();

    let roster_names: BTreeSet<String> = build_tool_list()
        .iter()
        .map(|t| t.name.to_string())
        .collect();

    assert_eq!(
        enum_values, roster_names,
        "howto schema's `tool` enum must list exactly the live tool roster \
         from server::router::build_tool_list(); enum={enum_values:?} roster={roster_names:?}"
    );

    // Directly pins the #172 item 1 fix: the enum must include
    // test_notification now that it's part of the live roster.
    assert!(
        enum_values.contains("test_notification"),
        "howto schema's `tool` enum must include test_notification (fed-734b8f #172 item 1)"
    );
}
