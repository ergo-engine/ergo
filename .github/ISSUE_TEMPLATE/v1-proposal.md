---
name: v1 Proposal
about: Propose new semantics beyond the frozen v0 kernel
title: "[v1] "
labels: v1-proposal
---

<!--
PURPOSE: Proposals for new semantics beyond the frozen v0 kernel.

The v0 kernel is CLOSED. New semantic obligations require explicit v1 decision records.
This template enforces the requirements from KERNEL_CLOSURE.md.

USE THIS TEMPLATE WHEN:
- Proposing new behavior not covered by v0
- Requesting meaning changes to existing primitives
- Suggesting new coercions, defaults, or domain-loaded naming
- Any change that would "improve behavior by quietly changing meanings"

DO NOT USE FOR:
- Bug fixes consistent with existing doctrine (use Doctrine Gap template)
- Invariant enforcement gaps (use Doctrine Gap template)
- Documentation clarifications (use Doctrine Gap template)
-->

## Proposal Summary

**What new semantic obligation does this introduce?**
<!-- One paragraph. Be specific about what behavior would change. -->

**Why can't this be achieved within v0?**
<!-- Reference specific frozen constraints that block the v0 approach -->

---

## Specification

<!--
REQUIRED: v1 work must be "explicitly specified"
-->

**Proposed behavior:**
<!-- Precise description of new semantics -->

**Affected primitives/contracts:**
<!-- Which frozen specs would this touch if it were v0? -->

**Interaction with existing invariants:**
<!-- How does this relate to PHASE_INVARIANTS.md? New invariants needed? -->

---

## Phase Boundary

<!--
REQUIRED: v1 work must be "phase-bounded"
-->

**Where is this enforced?**
- [ ] Type system (compile-time)
- [ ] Validation (runtime, recoverable)
- [ ] Assertion (runtime, panic)
- [ ] Test (regression detection only)
- [ ] Trust-based (adapter boundary)

**What happens if violated?**
<!-- Specific error or behavior -->

---

## Regression Testing

<!--
REQUIRED: v1 work must be "regression-tested"
-->

**Test plan:**
<!-- What tests will prove this works? -->

**Backward compatibility:**
<!-- Does this affect persisted formats? If yes, compatibility plan required. -->

- [ ] No persisted format changes
- [ ] Persisted format affected — compatibility plan: ___

---

## Decision Record

<!--
REQUIRED: v1 work must be "tagged/recorded as new obligations"
-->

**If accepted, this becomes a new obligation in:**
- [ ] New frozen spec (requires v2 to change)
- [ ] New stable spec (requires review to change)
- [ ] New canonical entry (Claude-owned)

**Proposed invariant ID (if applicable):**
<!-- e.g., V1-FEATURE-1 -->

---

## Justification

**What vertical/use-case proves necessity?**
<!-- Per PHASE_INVARIANTS.md: "No new core implementations without a vertical proof demonstrating necessity" -->

**Alternatives considered:**
<!-- Why can't clusters/composition solve this? -->

---

## Approval Path

Per KERNEL_CLOSURE.md, v1 proposals require:
- [ ] Sebastian review
- [ ] Claude (Doctrine Owner) review
- [ ] ChatGPT (Ontology Guardian) review

**Status:** Draft | Under Review | Approved | Rejected
