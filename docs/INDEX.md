# Documentation Index

> **Single-Source Rule:** Every fact has exactly one authoritative location. Authority level is declared in each document's frontmatter, not by directory placement. CI enforces change rules by reading frontmatter.

---

## Start Here

New to the system? Read these in order:

1. [kernel](system/kernel.md) — What "kernel" and "closed" mean
2. [kernel-prod-separation](system/kernel-prod-separation.md) — Kernel/prod boundary and host intent
3. [ontology](system/ontology.md) — The four primitives and their causal roles
4. [execution](system/execution.md) — How graphs evaluate
5. [freeze](system/freeze.md) — What is frozen vs patchable
6. [cluster-spec](authoring/cluster-spec.md) — Data structures for composition
7. [ui-runtime](contracts/ui-runtime.md) — What a UI must emit
8. [concepts](authoring/concepts.md) — Cluster composition concepts
9. [invariants](invariants/INDEX.md) — Enforcement loci for all invariants
10. [terminology](system/terminology.md) — Canonical terms and usage

---

## By Topic

Directory structure is the topic map. No separate navigation aids needed.

| Directory | Concern | Contents |
|-----------|---------|----------|
| [system/](system/) | Core laws and identity | Ontology, execution model, freeze declaration, kernel closure, kernel/prod separation, terminology |
| [orchestration/](orchestration/) | Supervision and trust boundaries | Supervisor spec, adapter contract |
| [authoring/](authoring/) | Building clusters and graphs | Authoring concepts, cluster spec, YAML format, loader contract |
| [primitives/](primitives/) | Primitive implementation contracts | Source, Compute, Trigger, Action, Adapter manifests |
| [contracts/](contracts/) | External interface specifications | UI ↔ Runtime contract, extension roadmap |
| [invariants/](invariants/) | Phase boundaries and enforcement | 194 tracked invariants across 16 phase files + rule registry |
| [ledger/](ledger/) | Operational planning and doctrine risk tracking | Closure register, dev-work ledgers, gap-work ledgers, decision log |

---

## Authority Levels

Every document declares its authority in frontmatter. The four levels and their change rules:

| Level | Change Rule | Documents |
|-------|-------------|-----------|
| **FROZEN** | v1 required to change | [ontology](system/ontology.md), [execution](system/execution.md), [freeze](system/freeze.md), [supervisor](orchestration/supervisor.md), [adapter](orchestration/adapter.md) |
| **STABLE** | Additive changes only | [concepts](authoring/concepts.md), [cluster-spec](authoring/cluster-spec.md), [yaml-format](authoring/yaml-format.md), [primitives/](primitives/) (all five manifests), [rule-registry](invariants/rule-registry.md) |
| **CANONICAL** | Tracks implementation | [kernel](system/kernel.md), [kernel-prod-separation](system/kernel-prod-separation.md), [terminology](system/terminology.md), [loader](authoring/loader.md), [invariants/](invariants/) (all phase files + INDEX) |
| **CONTRACTS** | External interfaces | [ui-runtime](contracts/ui-runtime.md), [extension-roadmap](contracts/extension-roadmap.md) |

---

## How Documents Work Together

- **[invariants/](invariants/)** tracks which invariants are enforced where — one file per phase
- **[ledger/closure-register](ledger/closure-register.md)** tracks semantic gaps and their resolutions
- **[ledger/dev-work/](ledger/dev-work/)** tracks implementation delivery, **[ledger/gap-work/](ledger/gap-work/)** tracks doctrine/risk gaps, and **[ledger/decisions/](ledger/decisions/)** records authority outcomes
- All three reference spec documents (ontology, execution, etc.) as the source of truth

---

## Crate-Local Documentation

READMEs under `crates/` are informational for their respective packages.
They are not authoritative for system behavior.
