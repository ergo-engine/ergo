# Documentation Index

> **Single-Source Rule:** Every fact has exactly one authoritative location. Authority level is declared in each document's frontmatter, not by directory placement. CI enforces change rules by reading frontmatter.

---

## Start Here

New to the system? Read these in order:

1. [kernel](system/kernel.md) — What "kernel" and "closed" mean
2. [current-architecture](system/current-architecture.md) — What the v1 system looks like today
3. [kernel-prod-separation](system/kernel-prod-separation.md) — Kernel/prod boundary and host/channel roles
4. [ontology](system/ontology.md) — The four primitives and their causal roles
5. [execution](system/execution.md) — How graphs evaluate
6. [freeze](system/freeze.md) — What is frozen vs patchable
7. [cluster-spec](authoring/cluster-spec.md) — Data structures for composition
8. [ingress-channel-guide](authoring/ingress-channel-guide.md) — HostedEvent ingress and process-channel authoring
9. [egress-channel-guide](authoring/egress-channel-guide.md) — Intent dispatch and durable-accept egress authoring
10. [concepts](authoring/concepts.md) — Cluster composition concepts
11. [invariants](invariants/INDEX.md) — Enforcement loci for all invariants
12. [terminology](system/terminology.md) — Canonical terms and usage

---

## By Topic

Directory structure is the topic map. No separate navigation aids needed.

| Directory | Concern | Contents |
|-----------|---------|----------|
| [system/](system/) | Core laws and identity | Ontology, execution model, freeze declaration, kernel closure, current architecture, kernel/prod separation, terminology |
| [orchestration/](orchestration/) | Supervision and trust boundaries | Supervisor spec, adapter contract |
| [authoring/](authoring/) | Building clusters, graphs, adapters, and boundary channels | Authoring concepts, cluster spec, YAML format, loader contract, ingress channel guide, egress channel guide |
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
| **CANONICAL** | Tracks implementation | [kernel](system/kernel.md), [current-architecture](system/current-architecture.md), [kernel-prod-separation](system/kernel-prod-separation.md), [terminology](system/terminology.md), [loader](authoring/loader.md), [ingress-channel-guide](authoring/ingress-channel-guide.md), [egress-channel-guide](authoring/egress-channel-guide.md), [invariants/](invariants/) (all phase files + INDEX) |
| **CONTRACTS** | External interfaces | [ui-runtime](contracts/ui-runtime.md), [extension-roadmap](contracts/extension-roadmap.md) |

---

## How Documents Work Together

- **[invariants/](invariants/)** tracks which invariants are enforced where — one file per phase
- **[ledger/closure-register](ledger/closure-register.md)** tracks semantic gaps and their resolutions
- **[ledger/dev-work/](ledger/dev-work/)** tracks implementation delivery, **[ledger/gap-work/](ledger/gap-work/)** tracks doctrine/risk gaps, and **[ledger/decisions/](ledger/decisions/)** records authority outcomes
- All three reference spec documents (ontology, execution, etc.) as the source of truth
- Decision records explain **why** a ruling was made. Top-level system,
  primitive, and authoring docs explain **what the current system is**.

---

## Crate-Local Documentation

READMEs under `crates/` are informational for their respective packages.
They are not authoritative for system behavior.
