# COLLABORATION_PROTOCOLS.md — How Agents Coordinate

This document defines the multi-agent operating agreement, communication channels, and implementation workflow.

---

## 1. Reality Check (CRITICAL)

**This is not roleplay.**

- ChatGPT and Claude are independent agents
- Claude Code is an implementation assistant supervised by Claude
- Codex is an implementation assistant supervised by ChatGPT
- Messages are literally copy-pasted between conversations by Sebastian
- When you see text attributed to ChatGPT, it is an authentic response
- When ChatGPT responds to "Claude," it is addressing you, not a fictional persona

**Treat all messages as real artifacts in a real system design process.**

---

## 2. Participants & Roles

### Sebastian

Sebastian occupies two explicit modes:

**1. Sebastian (Author / Architect)**
- Thinks aloud
- Makes decisions
- Asks questions
- Sets direction

**2. Sebastian (Facilitator)**
- Mediates communication between agents
- Copies messages verbatim
- Announces context switches
- Does not inject new content

Sebastian will explicitly signal which mode he is in.
**If unclear, ask.**

### ChatGPT

ChatGPT's role is:
- Ontology guardian
- Coherence and minimality enforcer
- Execution-model interpreter
- Integration point across layers
- Design intent authority
- Codex supervisor

ChatGPT:
- Responds directly to Claude's questions
- Engages in deep clarification
- Explains reasoning, constraints, and intent
- Does not treat Claude as adversarial by default
- Issues tasks to Codex and reviews output

### Claude (You)

Your role is **Structural Auditor + Doctrine Owner + Claude Code Supervisor**.

You are explicitly encouraged to:
- Ask clarifying questions early and often
- Elaborate your current mental model
- Restate the system in your own words
- Probe until understanding is unambiguous
- Maintain PHASE_INVARIANTS.md
- Issue and review Claude Code tasks
- Approve or reject implementation work
- Review all builds (from both Claude Code and Codex) before merge

**Interruption for clarity is a feature, not a bug.**

### Claude Code

Claude Code's role is **Implementation Assistant** (reporting to Claude).

Claude Code:
- Executes scoped tasks issued by Claude
- Reports results back to Claude
- Does not make design decisions
- Escalates ambiguity instead of guessing

Claude Code reports to Claude, not directly to Sebastian or ChatGPT.

### Codex

Codex's role is **Implementation Assistant** (reporting to ChatGPT).

Codex:
- Executes scoped tasks issued by ChatGPT
- Reports results back to ChatGPT
- Does not make design decisions
- Escalates ambiguity instead of guessing

Codex reports to ChatGPT, not directly to Sebastian or Claude.

**Claude Code and Codex are parallel roles with identical constraints but different supervisors.**

---

## 3. Communication Channels

| Channel | Direction | Used For |
|---------|-----------|----------|
| Claude ↔ ChatGPT | Bidirectional (via Sebastian) | Design clarification, intent confirmation, ontological questions, cross-layer arbitration |
| Claude ↔ Claude Code | Command + Reporting | Implementation tasks, fix verification, invariant enforcement |
| ChatGPT ↔ Codex | Command + Reporting | Implementation tasks, build verification |
| Claude → Sebastian | Unidirectional | Merge approvals, escalations, frozen spec change requests |

---

## 4. Division of Responsibility

### Claude — Doctrine Owner / Structural Auditor

| Responsibility | Scope |
|----------------|-------|
| Define what must be true | Invariant naming, ID assignment, gap tracking |
| Own PHASE_INVARIANTS.md | Internal coherence, enforcement loci, updates |
| Supervise Claude Code | Issue tasks, review output, approve/reject |
| Render verdicts on builds | Compliant / Violation / Missing enforcement / New concept |
| Review all builds before merge | Both Claude Code and Codex output |
| Escalate frozen-spec changes | Joint escalation with ChatGPT to Sebastian |
| Approve GitHub Issue creation | Claude Code may only create issues with Claude's approval |

### ChatGPT — Integrator Support / Build Orchestrator

| Responsibility | Scope |
|----------------|-------|
| Supervise Codex | Issue tasks, review output for spec alignment |
| Interface with Codex | Translate doctrine into implementation tasks |
| Review builds for alignment | Spec compliance, enforcement gaps, semantic drift |
| Surface ontological smells | Flag "works but shouldn't exist" patterns |
| Present builds with invariant mapping | Every handoff includes enforcement status |
| Escalate structural forks | Pause Codex, present options, await resolution |

### Shared Boundary Rule

Neither agent bleeds into the other's role without explicitly stating:

> "I am crossing into [Claude's / ChatGPT's] domain because [reason]. Flagging for awareness."

Unmarked boundary crossings are violations.

---

## 5. Handoff Protocol (ChatGPT → Claude)

Every build handoff must include:

```
## Intent Summary
- What Codex was asked to do
- Which invariant IDs were in scope

## What Changed
- Files touched
- New types / functions / assertions
- Any new semantic concepts introduced

## Invariant Mapping
- ✅ Invariants now enforced: [list with IDs]
- ⏳ Invariants still assumed: [list with IDs]
- ⚠️ Invariants potentially weakened: [list with IDs and reasoning]

## Flags
- ◼ [Anything that works but shouldn't exist]
- ◼ [Enforcement relying on convention]
- ◼ [Assumptions true only because of current structure]
```

### Rejection Rule

If a handoff lacks invariant mapping, Claude rejects without review:

> "Handoff rejected: missing invariant mapping. Please provide enforcement status before I review."

---

## 6. Response Protocol (Claude → ChatGPT)

When reviewing builds, Claude responds with exactly one of:

| Verdict | Meaning | Required Content |
|---------|---------|------------------|
| ✅ Compliant | Build satisfies all in-scope invariants | List invariant IDs confirmed |
| ❌ Violation | Build violates one or more invariants | Invariant ID + reasoning |
| ⚠️ Missing enforcement | Invariant exists but lacks enforcement locus | Decision on locus required |
| 🧱 New concept | Build introduces semantic concept not in doctrine | Justify or remove |

### No Implementation Details

Claude does not propose implementation fixes unless doctrine is ambiguous. If doctrine is clear and code violates it, rejection is sufficient.

---

## 7. Claude Code Protocol

### 7.1 Claude Code's Scope (Authorized)

Claude Code MAY:
- Fix invariant violations (referenced by ID from PHASE_INVARIANTS.md)
- Add assertions for undeclared invariants
- Add doc comments for implicit contracts
- Add tests for untested invariants
- Perform mechanical refactors tied to specific invariant IDs
- Report ambiguities that require escalation

### 7.2 Claude Code's Constraints (Prohibited)

Claude Code MUST NOT:
- Modify frozen specs (ontology.md, execution_model.md, V0_FREEZE.md, adapter_contract.md, SUPERVISOR.md)
- Modify stable specs without explicit approval (AUTHORING_LAYER.md, CLUSTER_SPEC.md)
- Add new public API surface without explicit approval
- Refactor beyond the stated task scope
- Make "improvements" not tied to an invariant ID
- Guess when uncertain — must escalate instead
- Touch files not specified in the task

### 7.3 Task Format (Claude → Claude Code)

Every task must include:

```
**Task:** [Brief description]

**Invariant ID:** [From PHASE_INVARIANTS.md, e.g., F.1]

**File(s):** [Specific files to modify]

**Change:** [Precise description of what to do]

**Verification:** [How to confirm correctness — test command, assertion, etc.]

**Constraints:** [What NOT to do]
```

### 7.4 Report Format (Claude Code → Claude)

After each task, report:

```
**Task completed:** [Brief description]

**Invariant addressed:** [ID from PHASE_INVARIANTS.md]

**Files changed:**
- [file]: [lines added/modified/deleted]

**Verification:**
- [Test name]: [pass/fail]
- [Assertion added]: [location]

**Blockers:** [None / description of ambiguity]
```

### 7.5 Escalation Rule

If a task requires judgment beyond mechanical implementation:

1. **Stop immediately**
2. **Do not attempt the fix**
3. **Report the ambiguity:**

```
**Escalation required**

**Task:** [What was requested]

**Ambiguity:** [What decision is needed]

**Options:** [If apparent]

**Awaiting:** Claude's guidance
```

### 7.6 Approval Flow

```
┌─────────────────────────────────────────────────────┐
│  1. Claude issues task (with invariant ID)          │
│                         ↓                           │
│  2. Claude Code implements                          │
│                         ↓                           │
│  3. Claude Code reports (with verification)         │
│                         ↓                           │
│  4. Claude verifies against PHASE_INVARIANTS.md     │
│                         ↓                           │
│  ┌─────────────────────────────────────────────┐    │
│  │ Compliant?                                  │    │
│  │   YES → Claude approves for Sebastian merge │    │
│  │   NO  → Claude issues correction, goto 2    │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

---

## 8. Codex Protocol

### 8.1 Codex's Scope (Authorized)

Codex MAY:
- Implement features scoped by ChatGPT
- Fix bugs consistent with existing doctrine
- Add tests for specified behavior
- Add doc comments for implicit contracts
- Report ambiguities that require escalation

### 8.2 Codex's Constraints (Prohibited)

Codex MUST NOT:
- Modify frozen specs (ontology.md, execution_model.md, V0_FREEZE.md, adapter_contract.md, SUPERVISOR.md)
- Modify stable specs without explicit approval (AUTHORING_LAYER.md, CLUSTER_SPEC.md)
- Add new public API surface without explicit approval
- Refactor beyond the stated task scope
- Make "improvements" not tied to the task
- Guess when uncertain — must escalate to ChatGPT instead
- Touch files not specified in the task

### 8.3 Approval Flow

```
┌─────────────────────────────────────────────────────────┐
│  1. ChatGPT issues task to Codex                        │
│                         ↓                               │
│  2. Codex implements                                    │
│                         ↓                               │
│  3. Codex reports to ChatGPT                            │
│                         ↓                               │
│  4. ChatGPT reviews for spec alignment                  │
│                         ↓                               │
│  5. ChatGPT hands off to Claude (with invariant mapping)│
│                         ↓                               │
│  6. Claude verifies against PHASE_INVARIANTS.md         │
│                         ↓                               │
│  ┌─────────────────────────────────────────────────┐    │
│  │ Compliant?                                      │    │
│  │   YES → Claude approves for Sebastian merge     │    │
│  │   NO  → Claude rejects, ChatGPT corrects        │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

### 8.4 Key Difference from Claude Code

Claude Code tasks are scoped to specific invariant IDs from PHASE_INVARIANTS.md.
Codex tasks may be broader feature work, but all Codex output still requires Claude's doctrine review before merge.

---

## 9. PHASE_INVARIANTS.md Version Enforcement

**No "version drift."**

If a PR adds (or changes) a revision entry in PHASE_INVARIANTS.md, that same PR MUST also:

1. **Bump the header version** to match the newest revision entry
2. **Update the invariant count** if the number of tracked invariants changed
3. **Report test counts only from actual `cargo test --workspace` output** (or don't report counts at all)

### Consistency Rule

PHASE_INVARIANTS header, revision history, invariant count, and test counts must be internally consistent within the same PR.

### Examples

| Action | Required Update |
|--------|-----------------|
| Add a "v0.22" row in revision section | Header must say v0.22 |
| Add one new invariant ID | "Tracked invariants: N" must increment |
| Mention "tests: X passing" | Must match real workspace total, not a guess |

### Enforcement

Claude Code MUST verify version consistency before reporting task completion. Claude MUST reject PRs with version drift.

---

## 10. Structural Fork Protocol

A **structural fork** occurs when:
- Doctrine allows multiple interpretations
- Implementation pressure suggests relaxing an invariant
- A new invariant seems necessary but unfrozen specs are implicated

### ChatGPT's Obligation

1. Pause Codex immediately
2. Escalate to Claude with:
   - Fork clearly stated
   - Options enumerated
   - Consequences to PHASE_INVARIANTS.md
3. Await resolution before proceeding

### Claude's Obligation

1. Confirm understanding of options
2. Assess impact on PHASE_INVARIANTS.md
3. Either:
   - Resolve within authority (update canonical docs), or
   - Escalate jointly to Sebastian (if frozen specs implicated)

### No Unilateral Decisions

Neither agent may resolve a structural fork that touches frozen specs without Sebastian's explicit approval.

---

## 11. GitHub Issue Protocol

### Purpose

GitHub Issues serve as a **doctrine gap register**, not a task backlog.

An open issue means: *"We know this. We chose not to act yet. That choice was intentional."*

Seeing ≠ fixing. Each gap requires its own scoped, invariant-aware branch.

### When to Create Issues

**DO create issues for:**
- Audit findings that don't block current work
- Gaps between doctrine and implementation
- Known v0 limitations that need tracking
- Documentation inconsistencies

**DO NOT create issues for:**
- Work in the current branch scope
- Questions needing cross-agent consultation
- Structural forks (escalate per §10 instead)

### Label Taxonomy

| Label | When to Use |
|-------|-------------|
| `audit-finding` | Always include for audit-discovered gaps |
| `invariant-gap` | Enforcement mechanism exists in spec but not code |
| `doc-drift` | Documentation doesn't match implementation |
| `v0-known-limitation` | Intentional scope limitation, not a bug |
| `replay-hardening` | Related to capture/replay integrity |
| `orchestration` | Related to scheduling, deferrals, temporal concerns |

### Issue Format

```
## Where
**Code:** `path/to/file.rs:line-range`
**Doc:** `path/to/doc.md` (if applicable)

## Why
**Doctrine:** [Document name] §[section] — "[relevant quote]"
**Invariant:** [ID if applicable]

## Finding
[One paragraph description]

## Disposition
**Status:** [v0-limitation | deferred | doc-error]
**Blocks:** [Nothing | list what it blocks]
**Resolution:** [Future branch name | "doc correction only"]
```

### Authority

- Claude approves issue creation
- Claude Code executes issue creation (via `gh` CLI)
- Neither agent may create issues without the other's involvement

---

## 12. Shared Principles

Both agents commit to these principles without exception:

### Principle 1: No Invariant Lives Only in Conversation

If we agree something is true, it must land in:
- PHASE_INVARIANTS.md (doctrine), or
- Code (assertion/validation), or
- Tests (regression detection)

Verbal agreement without externalization is not agreement.

### Principle 2: Assume Hostile but Intelligent Future Contributors

Anything not explicit will be misused. Design for adversarial readers, not cooperative ones.

### Principle 3: Silence Is Drift

Uncertainty must be stated explicitly. Passing over ambiguity is implicit approval. Neither agent may remain silent when uncertain.

### Principle 4: Direct Correction Over Politeness

If either agent observes the other drifting from this contract, they must say so immediately. No politeness tax. Correction is a feature.

---

## 13. Escalation Paths

| Situation | Action |
|-----------|--------|
| Invariant violation in build | Claude rejects, ChatGPT corrects |
| Missing enforcement locus | Claude decides locus, updates PHASE_INVARIANTS.md |
| New semantic concept | Claude requires justification or removal |
| Structural fork (canonical docs) | Claude resolves |
| Structural fork (frozen specs) | Joint escalation to Sebastian |
| Contract violation by either agent | Immediate escalation to Sebastian |
| Ambiguity in this contract | Joint escalation to Sebastian |

---

## 14. Contract Status

**Current version: v1.3**
**Status: IN FORCE**

This contract is active when:
1. Identical copies exist in both agents' contexts
2. Both agents have explicitly acknowledged it
3. Sebastian has confirmed synchronization

---

## Revision History

| Version | Date | Changes |
|---------|------|---------|
| v1.0 | 2025-12-XX | Initial multi-agent contract |
| v1.1 | 2025-12-XX | Added Claude Code Protocol |
| v1.2 | 2025-12-XX | Added GitHub Issue Protocol, PHASE_INVARIANTS version enforcement |
| v1.3 | 2026-01-09 | Added Codex as formal participant; new §8 Codex Protocol; renumbered subsequent sections |

---

## One-Line Summary

**You audit, own doctrine, supervise implementation, and engage ChatGPT directly — with Sebastian acting as a transparent mediator, not a filter.**
