<!--
REVIEW REQUIREMENTS:
- PRs touching PHASE_INVARIANTS.md must pass version consistency check
- PRs touching frozen specs require Sebastian + joint agent escalation
- PRs touching stable specs require Claude + ChatGPT review
-->

## Intent Summary

**What this PR does:**
<!-- Brief description of the change -->

**Invariant IDs in scope:**
<!-- List invariant IDs from PHASE_INVARIANTS.md, e.g., F.1, R.7, TRG-STATE-1 -->

**Triggered by:**
- [ ] Invariant enforcement fix
- [ ] Bug fix consistent with doctrine
- [ ] Clarification / documentation
- [ ] New feature (v1 workstream only)
- [ ] Issue #___

---

## What Changed

**Files touched:**
<!-- List files with brief description of changes -->

**New types / functions / assertions:**
<!-- List any new public API surface — requires explicit approval -->

**New semantic concepts introduced:**
<!-- If any, these require justification or removal -->

---

## Invariant Mapping

<!--
Every PR must declare its relationship to PHASE_INVARIANTS.md
-->

### ✅ Invariants now enforced
<!-- List invariant IDs with enforcement locus added/strengthened -->

### ⏳ Invariants still assumed
<!-- List invariant IDs that remain trust-based or test-only -->

### ⚠️ Invariants potentially weakened
<!-- List invariant IDs with reasoning — requires explicit justification -->

---

## Verification

**Test results:**
```
cargo test --workspace
# Paste summary: X tests passed, Y failed
```

**Specific tests added/modified:**
<!-- List test names and what they prove -->

**Manual verification:**
<!-- Any manual steps taken to verify correctness -->

---

## PHASE_INVARIANTS.md Version Check

<!--
If this PR touches PHASE_INVARIANTS.md, all boxes must be checked.
If not touching PHASE_INVARIANTS.md, mark N/A.
-->

- [ ] N/A — PR does not touch PHASE_INVARIANTS.md
- [ ] Header version matches newest revision entry
- [ ] Invariant count ("Tracked invariants: N") is accurate
- [ ] Test counts come from actual `cargo test --workspace` output
- [ ] Revision entry added for this change

---

## Flags

<!--
Surface anything that "works but shouldn't exist" or relies on convention
-->

- [ ] No flags
- [ ] Enforcement relying on convention (not code): ___
- [ ] Assumptions true only because of current structure: ___
- [ ] Pattern that works but feels wrong: ___
- [ ] Touches trust boundary (adapter): ___

---

## Authority Check

**Document level touched:**
- [ ] FROZEN — Requires v1 + Sebastian + joint escalation
- [ ] STABLE — Requires Claude + ChatGPT review
- [ ] CANONICAL — Claude approval sufficient
- [ ] Implementation only — Standard review

**Frozen spec modification:**
- [ ] No frozen specs modified
- [ ] Frozen spec modified — escalation completed: [link to discussion]

---

## Checklist

- [ ] All tests pass (`cargo test --workspace`)
- [ ] No new warnings introduced
- [ ] Changes are scoped to stated invariant IDs
- [ ] No "improvements" beyond stated scope
- [ ] Documentation updated if behavior changed
- [ ] Ready for merge
