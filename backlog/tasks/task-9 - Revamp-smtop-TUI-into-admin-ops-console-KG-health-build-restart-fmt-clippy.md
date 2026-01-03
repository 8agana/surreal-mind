---
id: task-9
title: >-
  Revamp smtop TUI into admin-ops console (KG, health, build/restart,
  fmt/clippy)
status: To Do
assignee: []
created_date: '2026-01-03 02:00'
labels:
  - smtop
  - tui
  - admin
  - ops
  - surreal-mind
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
User request: turn `src/bin/smtop.rs` into an admin-ops console. Add ops panel with hotkeys for `kg_populate`, `kg_embed`, `reembed_kg`, health checks, build release + restart launchd, fmt, clippy. Add command runner pane (last command, status, duration, tail output). Build+restart should `cargo build --release --bin surreal-mind` then `launchctl kickstart -k gui/$(id -u)/dev.legacymind.surreal-mind` on success; include optional toggle for auto-restart after build. Keep existing status/logs; reflow layout as needed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Ops panel present in smtop with hotkeys to trigger kg_populate, kg_embed, reembed_kg, health check, build+restart, fmt, clippy.
- [ ] #2 Command runner pane shows last command, exit status, duration, and tail output; updates when commands run.
- [ ] #3 Build+restart runs release build for surreal-mind then restarts launchd service only on success.
- [ ] #4 Existing monitoring (health, sessions, DB, logs) preserved and layout remains readable.
- [ ] #5 Keybindings/help text updated to include new ops actions.
<!-- AC:END -->
