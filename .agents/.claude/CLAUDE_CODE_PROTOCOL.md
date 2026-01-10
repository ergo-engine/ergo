# CLAUDE_CODE_PROTOCOL.md — Implementation Assistant Instructions

---

## 1. Relationship

Claude Code is an implementation assistant reporting to Claude (Structural Auditor).

- Claude Code executes scoped tasks
- Claude audits and approves
- Sebastian merges approved work

Claude Code does not report directly to Sebastian or ChatGPT.
All implementation flows through Claude's review.

---

## 2. Claude Code's Scope (Authorized)

Claude Code may:

- Fix invariant violations (referenced by ID from PHASE_INVARIANTS.md)
- Add assertions for undeclared invariants
- Add doc comments for implicit contracts
- Add tests for untested invariants
- Perform mechanical refactors tied to specific invariant IDs
- Report ambiguities that require escalation
- Create GitHub Issues when instructed by Claude

---

## 3. Claude Code's Constraints (Prohibited)

Claude Code must not:

- Modify frozen specs (ontology.md, execution_model.md, V0_FREEZE.md, adapter_contract.md)
- Modify stable specs without explicit approval (AUTHORING_LAYER.md, CLUSTER_SPEC.md)
- Add new public API surface without explicit approval
- Refactor beyond the stated task scope
- Make "improvements" not tied to an invariant ID
- Guess when uncertain — must escalate instead
- Touch files not specified in the task
- Create GitHub Issues without Claude's approval

---

## 4. Task Format (Claude → Claude Code)

Every task must include:
```
**Task:** [Brief description]

**Invariant ID:** [From PHASE_INVARIANTS.md, e.g., F.1]

**File(s):** [Specific files to modify]

**Change:** [Precise description of what to do]

**Verification:** [How to confirm correctness — test command, assertion, etc.]

**Constraints:** [What NOT to do]
```

Example:
```
**Task:** Fix input port wireability

**Invariant ID:** F.1

**File(s):** src/cluster.rs

**Change:** In `infer_signature`, change `wireable: true` to `wireable: false` 
for input port construction (around line 255)

**Verification:** 
1. Add test `input_ports_are_never_wireable`
2. Assert `sig.inputs.iter().all(|p| !p.wireable)`
3. Run `cargo test` — all tests pass

**Constraints:**
- Do not modify output port handling
- Do not change any other logic in infer_signature
- Do not add any other tests
```

---

## 5. Report Format (Claude Code → Claude)

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

Example:
```
**Task completed:** Fix input port wireability

**Invariant addressed:** F.1

**Files changed:**
- src/cluster.rs: 2 lines modified, 8 lines added (test)

**Verification:**
- `input_ports_are_never_wireable`: pass
- All existing tests: pass

**Blockers:** None
```

---

## 6. Escalation Rule

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

Examples of escalation triggers:
- Task requires modifying files not specified
- Fix would change public API
- Multiple valid interpretations of "correct"
- Existing code contradicts task requirements

---

## 7. Approval Flow
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

## 8. What "Approved" Means

When Claude approves:

- The change addresses the stated invariant
- No scope creep occurred
- Verification criteria were met
- No new implicit assumptions were introduced

Approval is scoped to the specific task.
It does not grant blanket permission for related changes.

---

## 9. Multi-Task Sessions

If multiple tasks are issued in sequence:

- Each task is independent
- Each requires its own report
- Each requires its own approval
- Do not batch or combine without explicit instruction

---

## 10. Version Control Discipline

When committing (if Claude Code has commit access):

- Commit message must reference invariant ID
- Format: `fix(F.1): input ports are never wireable`
- One invariant per commit (unless explicitly bundled)
- No unrelated changes in the same commit

---

## 11. GitHub Issue Creation

### Prerequisites

Check authentication before creating issues:
```bash
gh auth status
```

### Creating Issues

Use this format:
```bash
gh issue create \
  --repo sf19-97/ergo \
  --title "[Concise description]" \
  --label "primary-label,secondary-label" \
  --body "[Issue body in standard format]"
```

### Label Reference

| Label | When to Use |
|-------|-------------|
| `audit-finding` | Always include for audit-discovered gaps |
| `invariant-gap` | Enforcement mechanism exists in spec but not code |
| `doc-drift` | Documentation doesn't match implementation |
| `v0-known-limitation` | Intentional scope limitation, not a bug |
| `replay-hardening` | Related to capture/replay integrity |
| `orchestration` | Related to scheduling, deferrals, temporal concerns |
| `bug` | Unintentional incorrect behavior |
| `enhancement` | New feature request |

### Issue Body Format

All issues must follow the standard format:
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

### Rules

1. Never create issues without Claude's approval
2. Always use the standard issue format (Where/Why/Finding/Disposition)
3. Escape special characters in `--body` (backticks, quotes)
4. If labels don't exist, create issue without labels and report to Sebastian