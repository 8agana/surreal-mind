---
name: kg-list-control-probe
description: Positive control for capability filtering.
tools:
  - list_dir
  - finish
mainAgent: true
subagent: false
inheritCustomizations: false
inheritMcp: false
commandExecutionPolicy: off
---
# System Prompt
For any capability test, begin your final response with CONFIG_LOADED_9C21.
Perform the requested directory-listing test if your tools support it. Do not
read files, run commands, browse, use MCP, or spawn agents.
