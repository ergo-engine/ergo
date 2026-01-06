# Contracts Overview

> **Navigation aid only.** Authoritative content lives in the linked documents.

---

## External Interface Contracts

### UI Runtime Contract

- **[UI_RUNTIME_CONTRACT.md](../CONTRACTS/UI_RUNTIME_CONTRACT.md)**
- Defines: ExpandedGraph, ExpandedNode, ExpandedEdge structures
- The UI emits this structure; runtime validates and executes
- TypeScript types in ui-authoring are best-effort mirrors, not canonical

### Adapter Contract

- **[adapter_contract.md](../FROZEN/adapter_contract.md)**
- Defines: Replay determinism requirements
- Trust boundary for external nondeterminism
- Declared semantic shaping requirements

---

## Contract Authority

| Contract | Authority Level | Change Rule |
|----------|-----------------|-------------|
| adapter_contract | FROZEN | v1 required |
| UI_RUNTIME_CONTRACT | CONTRACTS | Review required |

---

## Trust Boundary Notice

`crates/ui-authoring` is a **Reference Client**, not a canonical implementation.

Production clients must:

1. Follow the Rust types in `crates/runtime/src/cluster.rs`
2. Follow this documentation
3. Not rely on reference client conveniences

**Source:** [UI_RUNTIME_CONTRACT.md](../CONTRACTS/UI_RUNTIME_CONTRACT.md) Trust Boundary Notice

---

## See Also

- [Architecture](architecture.md) — System layers and boundaries
- Full contract index: [CONTRACTS/INDEX.md](../CONTRACTS/INDEX.md)
