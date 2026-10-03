---
id: TASK-29
title: >-
  Add the agent rule: consult the local tdd and refactor skills when
  implementing changes
status: Done
assignee: []
created_date: '2026-10-03 10:43'
updated_date: '2026-10-03 10:47'
labels:
  - dx
  - tooling
  - docs
dependencies: []
references:
  - backlog/docs/test-refactor-plan.md
  - .agents/skills/tdd/SKILL.md
  - .agents/skills/refactor/SKILL.md
priority: high
type: chore
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
AGENTS.md tells an agent what the repo is and which gates to run, but nothing
in it points at the skills the repo already vendors under .agents/skills/ —
so an agent implementing a change has no instruction to work test-first or to
keep refactoring behaviour-preserving, even though the tdd and refactor
skills codifying exactly that are checked in and pinned by skills-lock.json.

This task adds the rule. It is deliberately small and self-contained so the
larger test-suite refactor (task-28) can cite it rather than carry it.

The rule to add, in substance:

- When implementing a change (feature or fix), consult .agents/skills/tdd
  first and follow its red -> green loop: failing test first, minimal code to
  pass, one seam/one test/one implementation per cycle, tests at public
  seams only.
- When restructuring existing code without changing behaviour, consult
  .agents/skills/refactor and follow its process: clear purpose, tests exist
  first, small steps, gates green between steps, never mix refactoring with
  feature changes in one step.
- The skills are the reference for *how*; AGENTS.md remains the reference
  for *what* (gates, layout, conventions). The rule must point at the local
  skill paths, not external copies, because skills-lock.json pins what was
  reviewed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 AGENTS.md has a section (near Code Style/Testing) directing agents to .agents/skills/tdd for implementing changes test-first and .agents/skills/refactor for behaviour-preserving restructuring, stating when each applies
- [x] #2 The rule names the local vendored paths under .agents/skills/ (pinned by skills-lock.json), not external URLs
- [x] #3 The rule states that refactor steps and red->green cycles are separate commits, consistent with the Commits section
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added the 'Implementing changes: consult the vendored skills' section to AGENTS.md, between Tooling and Code Style. It points at the local .agents/skills/tdd and .agents/skills/refactor SKILL.md paths (pinned by skills-lock.json), states when each applies, and that red->green cycles and refactor steps are separate commits. Committed as docs(agents).
<!-- SECTION:NOTES:END -->
