---
Authority: CONTRACTS
Version: v0
Last Updated: 2026-01-06
Scope: Index of all external interface contracts
Verified Against Tag: v0.28-kernel-closed
Change Rule: Review required
---

# Contract Index

This directory contains external interface specifications.

---

## Current Contracts

### UI Runtime Contract

- **File:** [UI_RUNTIME_CONTRACT.md](UI_RUNTIME_CONTRACT.md)
- **Purpose:** Defines the data structure a UI must emit to drive the runtime
- **Key Types:** ExpandedGraph, ExpandedNode, ExpandedEdge, ParameterValue
- **Trust Boundary:** Runtime validates; UI-side validation is advisory only

### Adapter Contract

- **File:** [../FROZEN/adapter_contract.md](../FROZEN/adapter_contract.md)
- **Purpose:** Defines adapter compliance requirements for replay determinism
- **Authority:** FROZEN (lives in FROZEN/, linked here for completeness)
- **Key Requirements:** Determinism under replay, capture support, declared semantic shaping

---

## Contract Authority Rules

| Contract | Authority Level | Change Process |
|----------|-----------------|----------------|
| UI_RUNTIME_CONTRACT | CONTRACTS | Review required |
| adapter_contract | FROZEN | v1 required |

---

## Reference Client Notice

`crates/ui-authoring` is a **reference client**, not a canonical contract implementation.

Production clients must:

1. Follow the Rust types in `crates/runtime/src/cluster.rs`
2. Follow this documentation
3. Not rely on reference client conveniences

---

## See Also

- [TOPICS/contracts.md](../TOPICS/contracts.md) — Contracts topic summary
- [PHASE_INVARIANTS.md](../CANONICAL/PHASE_INVARIANTS.md) — Enforcement loci
