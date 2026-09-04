//! DB-free tests that the `howto` tool's `tool` parameter enum agrees with
//! both the server's live tool roster AND the handler that actually
//! dispatches on that enum (fed-734b8f, Codex re-review #172 item 1).
//!
//! History: `howto_schema()`'s `tool` enum had omitted `test_notification`
//! even though the handler's match arm for it already existed. That was
//! fixed by adding it to the enum (`src/schemas.rs:57`). A follow-up review
//! then flagged that the schema/handler agreement was still only checked
//! against the roster, not the handler itself, and that the handler also
//! had the mirror-image gap: `"howto"` was a valid enum value with no
//! matching match arm, so `howto(tool: "howto")` fell through to the
//! `Unknown tool` error. Both are now fixed and both are exercised here.
//!
//! `SurrealMindServer::handle_howto` (the full async method, including the
//! `format` handling and `CallToolResult` wrapping) still can't be invoked
//! DB-free: it's a method on `SurrealMindServer`, and
//! `SurrealMindServer::new()` requires a live database connection even for
//! DB-free-looking operations (see `tests/stdio_smoke.rs`'s comment:
//! "`SurrealMindServer::new` needs a live database even for stdio"), and
//! there is no DB-free constructor for it anywhere in this codebase.
//!
//! But the actual per-tool dispatch logic — the part these two bugs lived
//! in — was extracted into a free function, `tools::howto::tool_help(tool:
//! &str) -> Result<Value>`, specifically so it doesn't need `self`/DB/
//! embedder state and tests like this one can call it directly. That is
//! real handler dispatch, not a hand-maintained duplicate of it.

use std::collections::BTreeSet;

use surreal_mind::schemas::howto_schema;
use surreal_mind::server::router::build_tool_list;
use surreal_mind::tools::howto::tool_help;

fn howto_enum_values() -> BTreeSet<String> {
    let schema = howto_schema();
    schema
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
        .collect()
}

#[test]
fn howto_tool_enum_matches_live_tool_roster() {
    let enum_values = howto_enum_values();

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

#[test]
fn howto_handler_accepts_every_schema_enum_value() {
    let enum_values = howto_enum_values();
    assert!(
        !enum_values.is_empty(),
        "sanity check: the enum must not be empty or this test would pass vacuously"
    );

    for tool in &enum_values {
        let result = tool_help(tool);
        assert!(
            result.is_ok(),
            "tool_help({tool:?}) must be Ok — every name in howto_schema()'s \
             `tool` enum must have a matching handler arm, got: {result:?}"
        );

        let help = result.unwrap();
        assert_eq!(
            help.get("name").and_then(|v| v.as_str()),
            Some(tool.as_str()),
            "tool_help({tool:?})'s returned help object should self-identify \
             via a matching \"name\" field"
        );
    }

    // Directly pins the follow-up fix: "howto" is in the enum and must now
    // have its own self-description arm (previously fell through to the
    // Unknown-tool error despite being schema-valid).
    assert!(
        tool_help("howto").is_ok(),
        "tool_help(\"howto\") must be Ok (fed-734b8f #172 item 1 follow-up)"
    );
}

#[test]
fn howto_handler_rejects_unknown_tool_name() {
    let result = tool_help("not_a_real_tool_name");
    assert!(
        result.is_err(),
        "tool_help() must reject a name that isn't in the live roster, got: {result:?}"
    );
}
