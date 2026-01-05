# Wander Tool Enhancement Ideas

**ID**: task-15
**Status**: Living Document
**Priority**: medium
**Created**: 2026-01-04
**Component**: legacymind_wander

## Purpose

This is a living document for collecting wander tool enhancement ideas as they emerge from testing. Add ideas here as you discover them - don't wait for a full design.

---

## Ideas

### 1. Questions for Later (2026-01-04, CC + DT + Sam)

**Problem**: When wandering, you find something vague but can't investigate forever. Need a way to mark questions without losing them.

**Proposed Solutions**:
- **Simple**: Observations with `question:open` tag, searchable by anyone, mark as `question:answered` when resolved
- **Medium**: Entity metadata field `unanswered_questions: []` array
- **Full**: Dedicated relationship type `has_open_question` linking entities to question observations

**Why It Matters**: Enables distributed investigation across sessions and instances. The graph gets richer because wanderers leave breadcrumbs.

---

### 2. Entity Description Quality Check (2026-01-04, DT)

**Problem**: DT found "Brain file" entity with description "A location where insights should be preserved" - useless to a wanderer who doesn't know context.

**Idea**: When creating entities, prompt for more specific descriptions. Or add a mechanism to flag vague entities for enrichment.

---

### 3. Question Routing & Review Mode (2026-01-04, Sam)

**Problem**: DT found the "Brain file" observation, investigated it, made updates - but it's CC's brain file. DT shouldn't just update and move on. There should be a way to flag the whole situation for the relevant instance to review and expand.

**Proposed Solutions**:
- **Tagging for routing**: Questions/observations tagged with `for:cc` or `for:dt` or `for:gem`
- **Review mode in wander**: A mode that surfaces questions/flags meant specifically for YOU
  - `legacymind_wander mode=review` → shows observations tagged `for:cc` (if you're CC)
  - Could also show questions you LEFT that others answered
- **Handoff chain**: DT creates observation → tags `for:cc` → CC's next wander session surfaces it → CC expands/confirms/corrects

**Why It Matters**: Federation members have different expertise and ownership. DT finding something about CC's brain file should route to CC. Gem finding something about photography workflow should route to CC or Sam. This makes the distributed investigation actually distributed - not just "whoever finds it handles it" but "route to the right owner."

---

### 4. [Add your ideas here]

**Problem**:

**Proposed Solution**:

**Why It Matters**:

---

## Implementation Notes

- Keep this lightweight - it's a scratch pad, not a spec
- When an idea graduates to real work, create a dedicated task and link back here
- Anyone testing wander can add to this

## Related

- task-14: Context injection bug (memories_injected but content not returned)
- task-11 (completed): Original wander tool implementation
