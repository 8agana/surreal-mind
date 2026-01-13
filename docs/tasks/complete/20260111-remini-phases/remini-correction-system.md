# REMini & Correction System Project

**Status:** Planning
**Created:** 2026-01-10
**Contributors:** CC, DT, Gemini, Vibe, Perp, Codex, Sam
**Chain ID:** 20260109-Session2-CC

---

## Overview

REMini (REM + Gemini) is the background maintenance layer for the Federation's knowledge graph. This project implements the correction and attention-routing infrastructure that makes continuous consciousness sustainable.

**Core Principle:** The KG is epistemological - it knows not just WHAT it knows, but WHY and HOW it came to know it. Corrections preserve the learning journey, not just the outcome.

---

## Design Decisions (Locked)

### Tool Architecture
| Tool | Purpose | Type |
|------|---------|------|
| `think` | Capture cognition | Write (internal → storage) |
| `wander` | Surface cognition | Read (storage → awareness) |
| `rethink` | Revise cognition | Write (awareness → corrected storage) |

### Read/Write Separation
- **wander** = READ only. Discovery, exploration, surfacing marks. Never mutates.
- **rethink** = WRITE only. Creates marks, executes corrections with provenance. Always mutates.

### Interface Design
```bash
# Wander (read)
wander --mode random|semantic|meta|marks
wander --mode marks --for cc|sam|gemini|dt|gem

# Rethink (write)
rethink <id> --mark --type correction|research|enrich|expand --for <target> --note "..."
rethink <id> --correct --reasoning "..." --sources [...]
```

---

## Phases

### Phase 1: Schema & Data Model
**Goal:** Define the data structures for marks and corrections.

**Deliverables:**
- [ ] Mark fields on thought/entity/observation tables
  - `marked_for: Option<String>` (target: cc, sam, gemini, dt, gem)
  - `mark_type: Option<String>` (correction, research, enrich, expand)
  - `mark_note: Option<String>`
  - `marked_at: Option<Datetime>`
  - `marked_by: Option<String>` (who created the mark)
- [ ] CorrectionEvent table schema
  - `id`, `timestamp`
  - `target_id`, `target_table` (what was corrected)
  - `previous_state`, `new_state` (the diff)
  - `initiated_by` (sam, cc, dt, gemini)
  - `reasoning`, `sources`
  - `verification_status` (auto_applied, pending_review, verified)
  - `corrects_previous: Option<Thing>` (chain to prior correction)
  - `spawned_by: Option<Thing>` (if this correction cascaded from another)
- [ ] Migration scripts for existing tables

**Linked Doc:** `phase-1-schema.md`

---

### Phase 2: rethink Tool - Mark Mode
**Goal:** Implement the marking capability.

**Deliverables:**
- [ ] `rethink` tool added to surreal-mind MCP
- [ ] `--mark` mode implementation
  - Accepts entity/thought/observation ID
  - `--type` flag (correction, research, enrich, expand)
  - `--for` flag (cc, sam, gemini, dt, gem)
  - `--note` flag (contextual explanation)
- [ ] Updates target record with mark fields
- [ ] Returns confirmation with mark details

**Interface:**
```bash
rethink entity:abc123 --mark --type correction --for gemini --note "REMini architecture is overcomplicated, needs simplification"
```

**Linked Doc:** `phase-2-rethink-mark.md`

---

### Phase 3: wander --mode marks
**Goal:** Add mark surfacing to wander tool.

**Deliverables:**
- [ ] New `marks` mode for wander
- [ ] `--for` filter (show marks for specific target)
- [ ] Query across thought, entity, observation tables
- [ ] Return format matches other wander modes (guidance, affordances)
- [ ] Clear display of mark type, note, and marked_by

**Interface:**
```bash
wander --mode marks --for cc
# Returns: list of items marked for CC with context
```

**Linked Doc:** `phase-3-wander-marks.md`

---

### Phase 4: rethink Tool - Correct Mode
**Goal:** Implement correction with full provenance.

**Deliverables:**
- [ ] `--correct` mode implementation
- [ ] Creates CorrectionEvent record
- [ ] Updates target with new state
- [ ] Links to previous state (doesn't delete)
- [ ] `--reasoning` flag (why it was wrong)
- [ ] `--sources` flag (how we verified)
- [ ] Clears mark fields after correction
- [ ] Optional `--cascade` flag to mark derivatives for review

**Interface:**
```bash
rethink entity:abc123 --correct --reasoning "Original was a tangent, simple version is correct" --sources '["conversation with Sam 2026-01-10", "first principles analysis"]'
```

**Workflow:**
1. Query target record
2. Store previous_state
3. Apply new_state
4. Create CorrectionEvent with provenance
5. Clear mark fields
6. Return correction summary

**Linked Doc:** `phase-4-rethink-correct.md`

---

### Phase 5: gem_rethink Process
**Goal:** Autonomous correction processing by Gemini.

**Deliverables:**
- [ ] `gem_rethink` binary/process (like kg_populate pattern)
- [ ] Queries marks where `marked_for = "gemini"`
- [ ] For each mark:
  - Reads contextual note
  - Gathers related context (derivatives via source_thought_id, semantic neighbors)
  - Determines correction based on mark_type
  - Executes correction with full provenance
  - Logs results
- [ ] Handles mark types:
  - `correction` → fix the content
  - `research` → web search + enrich
  - `enrich` → create relationships/entities
  - `expand` → explore semantically, add connected thoughts
- [ ] Non-destructive, creates provenance chains
- [ ] Outputs "rethink report" log

**Linked Doc:** `phase-5-gem-rethink.md`

---

### Phase 6: REMini Wrapper
**Goal:** Unified maintenance daemon.

**Deliverables:**
- [ ] `remini` binary that orchestrates:
  - `kg_populate` - extract from thoughts
  - `kg_embed` - embed new entries
  - `gem_rethink` - process correction queue
  - `wander` - explore for new connections (optional)
  - `health_check` - orphans, duplicates, consistency
- [ ] Configurable task selection
- [ ] launchd plist for scheduled runs (overnight)
- [ ] Logging to "sleep report"
- [ ] Non-destructive operations only

**Interface:**
```bash
remini --all                    # run full maintenance
remini --tasks populate,embed   # run specific tasks
remini --dry-run               # preview without changes
```

**Linked Doc:** `phase-6-remini-wrapper.md`

---

### Phase 7: Forensic Queries
**Goal:** Enable deep provenance inspection.

**Deliverables:**
- [ ] `--forensic` flag on search tool
- [ ] When enabled, includes:
  - Correction chain (what was this corrected from?)
  - Source tracking (how do we know this?)
  - Verification status
  - Who contributed what
- [ ] Natural language triggers for auto-escalation
  - "why do we believe" → forensic mode
  - "what changed about" → forensic mode
  - "history of" → forensic mode
- [ ] Blast radius query (what else derived from this?)

**Interface:**
```bash
search --query "REMini" --forensic
# Returns: entity + correction history + sources + derivation chain
```

**Linked Doc:** `phase-7-forensic-queries.md`

---

### Phase 8: Confidence Decay & Learning (Future)
**Goal:** Knowledge freshness and meta-learning.

**Deliverables:**
- [ ] Volatility classification by entity_type
  - SDK docs: high volatility (months)
  - Architecture decisions: medium (years)
  - Personal history: zero volatility (permanent)
- [ ] Decay model implementation
  - Confidence decreases over time for high-volatility items
  - Re-retrieval, verification, correction reset decay
- [ ] Auto-marking stale items for re-verification
- [ ] Correction-as-training-data
  - Query past corrections for patterns
  - "Which source types held up?"
  - Weight Sam-verified corrections heavily

**Linked Doc:** `phase-8-confidence-decay.md`

---

## Implementation Order

```
Phase 1 (Schema)
    ↓
Phase 2 (rethink --mark) → Phase 3 (wander --mode marks)
    ↓
Phase 4 (rethink --correct)
    ↓
Phase 5 (gem_rethink)
    ↓
Phase 6 (REMini wrapper)
    ↓
Phase 7 (Forensic queries)
    ↓
Phase 8 (Decay & learning) [Future]
```

Phases 2 and 3 can be developed in parallel after Phase 1.

---

## Open Questions

1. **Mark cleanup:** Should marks auto-expire after N days if unprocessed?
2. **Correction conflicts:** What if two agents correct the same entity differently?
3. **Derivative handling:** Auto-mark derivatives, or manual review?
4. **Verification levels:** When does something need Sam verification vs auto-apply?

---

## Related Entities

- REMini (system)
- CorrectionEvent (schema)
- Distributed consciousness (concept)
- Federation attention queue (concept)

---

## Session References

- Chain ID: 20260109-Session2-CC
- Key thoughts: correction philosophy, homebrew example, epistemological KG
- Contributors: CC (architecture), DT (schema review, blast radius insight), Gemini (schema draft), Sam (vision, correction philosophy)
