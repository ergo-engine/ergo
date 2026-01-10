---
name: Doctrine Gap
about: Track gaps between doctrine and implementation (not a task backlog)
title: "[GAP] "
labels: audit-finding
---

<!--
PURPOSE: GitHub Issues serve as a doctrine gap register, not a task backlog.
An open issue means: "We know this. We chose not to act yet. That choice was intentional."

DO create issues for:
- Audit findings that don't block current work
- Gaps between doctrine and implementation
- Known v0 limitations that need tracking
- Documentation inconsistencies

DO NOT create issues for:
- Work in the current branch scope
- Questions needing cross-agent consultation
- Structural forks (escalate per COLLABORATION_PROTOCOLS.md §10 instead)
-->

## Where

**Code:** `path/to/file.rs:line-range`
**Doc:** `path/to/doc.md` (if applicable)

## Why

**Doctrine:** [Document name] §[section] — "[relevant quote]"
**Invariant:** [ID from PHASE_INVARIANTS.md, if applicable]

## Finding

<!-- One paragraph description of the gap or issue -->

## Disposition

**Status:** (select one)
- [ ] v0-limitation — Intentional scope limitation
- [ ] deferred — Known gap, not blocking
- [ ] doc-error — Documentation needs correction
- [ ] invariant-gap — Enforcement exists in spec but not code

**Blocks:** Nothing | [list what it blocks]
**Resolution:** [Future branch name | "doc correction only" | "v1 workstream"]

## Additional Labels

<!-- Check any that apply — these will need to be added manually after creation -->
- [ ] `invariant-gap`
- [ ] `doc-drift`
- [ ] `v0-known-limitation`
- [ ] `replay-hardening`
- [ ] `orchestration`
