# Open Questions & Decisions Needed

1. **Namespace Naming**
   - Do we use lowercase (`codex`) everywhere or include prefixes (`agent_codex`) for clarity?
2. **Federation Update Policy**
   - Who can edit shared brain entries?
   - Do we require a review/approval step before changes go live?
3. **Access Control**
   - Should we enforce namespace-level credentials in SurrealDB (if available), or rely on MCP-level controls for now?
4. **Search Scope Defaults**
   - Should some tools (e.g., handoff search) default to `all` instead of `own`?
5. **Photography Data Migration**
   - Existing photography entries currently in shared spaces—do we migrate them entirely to Codex namespace or leave references in federation?
6. **Cross-Agent Visibility**
   - When one agent requests `scope=all`, do we need to redact private sections or is full visibility acceptable within the Federation?
7. **Schema Evolution**
   - How do we handle future schema changes? (e.g., version field on records, migration scripts)
8. **Automation Windows**
   - During migration, what automation needs to be paused (culling scripts, PCP agents)? We should coordinate scheduling.
9. **Audit Trails**
   - Do we want to log every write (agent, namespace, section) in a centralized audit DB for compliance?
10. **Testing Environment**
   - Do we spin up a separate SurrealDB instance for rehearsal, or run the migration dry-run in the existing cluster with a different namespace prefix?
