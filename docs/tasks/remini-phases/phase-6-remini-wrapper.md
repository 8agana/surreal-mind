# Phase 6: REMini Wrapper

**Status:** In Progress (remini binary live; wander defaulted; health script hook; rethink type filter env)
**Parent:** [remini-correction-system.md](../remini-correction-system.md)
**Depends On:** Phase 5 (gem_rethink)
**Assignee:** TBD

---

## Goal

Unified maintenance daemon that orchestrates all background knowledge graph operations.

**REMini = REM + Gemini** - Synthetic REM sleep for AI consciousness.

---

## Deliverables

- [ ] `remini` binary
- [ ] Task orchestration logic
- [ ] Configurable task selection
- [ ] launchd plist for scheduling
- [ ] Sleep report logging
- [ ] Dry-run mode

---

## Components

| Task | Binary/Function | Purpose |
|------|----------------|---------|
| `populate` | kg_populate | Extract entities/observations/relationships from thoughts |
| `embed` | kg_embed | Generate embeddings for new entries |
| `rethink` | gem_rethink | Process correction queue |
| `wander` | wander (optional) | Explore for new connections |
| `health` | health_check | Orphans, duplicates, consistency |

---

## Interface

```bash
remini --all                        # run full maintenance suite
remini --tasks populate,embed       # run specific tasks
remini --tasks rethink              # just process corrections
remini --dry-run                    # preview without changes
remini --report                     # show last sleep report
```

---

## Orchestration Flow

```
1. Start logging
2. For each selected task:
   a. Log task start
   b. Execute task
   c. Capture results/errors
   d. Log task completion
3. Aggregate results into sleep report
4. Save report to vault
5. Exit
```

---

## launchd Plist

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>dev.legacymind.remini</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Users/samuelatagana/Projects/LegacyMind/surreal-mind/target/release/remini</string>
        <string>--all</string>
    </array>
    <key>StartCalendarInterval</key>
    <dict>
        <key>Hour</key>
        <integer>3</integer>
        <key>Minute</key>
        <integer>0</integer>
    </dict>
    <key>StandardOutPath</key>
    <string>/Users/samuelatagana/LegacyMind_Vault/logs/remini.log</string>
    <key>StandardErrorPath</key>
    <string>/Users/samuelatagana/LegacyMind_Vault/logs/remini-error.log</string>
</dict>
</plist>
```

---

## Sleep Report Format

```json
{
  "run_timestamp": "2026-01-10T03:00:00Z",
  "tasks_run": ["populate", "embed", "rethink", "health"],
  "summary": {
    "thoughts_processed": 12,
    "entities_created": 8,
    "embeddings_generated": 25,
    "corrections_made": 3,
    "health_issues_found": 1
  },
  "task_details": {
    "populate": { ... },
    "embed": { ... },
    "rethink": { ... },
    "health": { ... }
  },
  "duration_seconds": 300,
  "next_scheduled": "2026-01-11T03:00:00Z"
}
```

---

## Implementation Notes

- Added `remini` binary (standalone orchestrator):
  - CLI: `--all` or `--tasks populate,embed,rethink,wander,health`, `--dry-run`, `--report`.
  - Executes child binaries: `kg_populate`, `kg_embed`, `gem_rethink`, `kg_wander` (now included in default/all), and `scripts/sm_health.sh` when present; collects stdout/stderr + timings into JSON report (`logs/remini_report.json`).
  - Dry-run propagates via `DRY_RUN=1` to children.
- Rethink type filter: pass env `RETHINK_TYPES` (comma-separated) via `--rethink-types` flag; forwarded to gem_rethink.
- Health task: runs `scripts/sm_health.sh` if present; otherwise skipped with note.
- Wander task: runs `kg_wander` binary (if built); included in default/all task lists.
- Report: prints and persists JSON summary (tasks_run, per-task status, duration).
- Remaining TODOs: richer task selection flags (e.g., --for), better health implementation, retries, and launchd plist wiring.

---

## Testing

*To be defined*
