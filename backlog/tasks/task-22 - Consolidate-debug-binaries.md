---
id: task-22
title: Consolidate debug binaries
status: To Do
assignee: []
created_date: '2026-01-06 03:57'
labels:
  - refactoring
  - developer-experience
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Currently have 16 binaries in src/bin/. Keep production tools separate: smtop, kg_wander, kg_populate, kg_embed. Consolidate debug utilities into `surreal-mind admin <subcommand>` using clap. Debug utilities to consolidate: kg_inspect, sanity_cosine, db_check, check_db_contents, simple_db_test, fix_dimensions. This reduces build clutter and improves developer experience with a unified admin interface.
<!-- SECTION:DESCRIPTION:END -->
