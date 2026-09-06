# fed-6d00e5: subscription capability experiment

## Result

Agy 1.1.26 can select a workspace custom agent through `--new-project
--agent <name>` using the existing subscription login. A system-only marker
confirmed definition loading. A finish-only agent returned UNAVAILABLE when
asked to list the scratch directory; a control with list_dir enabled invoked
that tool successfully. These are bounded behavioral controls, not a formal
proof covering all hidden execution paths.

The stream's `init.tools` continued listing the global tool inventory even
for the custom agent. Earlier claims that this inventory alone proved the
custom capability restriction ineffective are therefore withdrawn. The first
probe without --new-project did not establish definition loading.

## Structured output

The gardener probe returned SUCCESS and a valid `result.structured_output`
with action, parameters, and rationale. Its separate response text contained
additional toolAction/toolSummary keys despite additionalProperties:false.
An integration must consume and independently validate structured_output,
not trust or parse the prose response as the schema-constrained result.

## Scope and next step

All provider runs used Agy subscription authentication, scratch workspaces,
and harmless supplied content. No API key or broad permission grant was used.
The positive control listed only the scratch directory. The initial unloaded
challenge attempted the protected Agy app directory and was denied; no file
read completed. Production SurrealMind was not modified.

Next: integrate a dedicated decision runner in the existing isolated worktree,
validate action-specific parameters before KG execution, reject unexpected
tool events, and test malformed, truncated, and denied responses offline.
Live scheduler activation remains pending.

Official references consulted:
- https://antigravity.google/docs/subagents
- https://antigravity.google/docs/cli/headless/
- https://antigravity.google/docs/sdk/overview/

The SDK documentation advertises API-key and Vertex paths; subscription OAuth
support there was not established. This experiment uses the CLI instead.
