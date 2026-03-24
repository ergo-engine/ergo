---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-23
Owner: Documentation Index
Scope: Documentation map and authority taxonomy
Change Rule: Tracks implementation
---

# Documentation Index

> **Single-Source Rule:** Every fact has exactly one authoritative
> location. Authority level is declared in each document's
> frontmatter, not by directory placement. CI enforces change rules by
> reading frontmatter.

---

## Start Here

New to the system? Read these in order:

1. [kernel](system/kernel.md) — What "kernel" and "closed" mean
2. [current-architecture](system/current-architecture.md) — What the
   v1 system looks like today
3. [kernel-prod-separation](system/kernel-prod-separation.md) —
   Kernel/prod boundary and host/channel roles
4. [ontology](system/ontology.md) — The four primitives and their causal roles
5. [execution](system/execution.md) — How graphs evaluate
6. [freeze](system/freeze.md) — What is frozen vs patchable
7. [cluster-spec](authoring/cluster-spec.md) — Data structures for composition
8. [project-convention](authoring/project-convention.md) — SDK-first
   Ergo project shape, `Cargo.toml` vs `ergo.toml`, profiles,
   clusters, and crate layout
9. [getting-started-sdk](authoring/getting-started-sdk.md) — Scaffold,
   run, validate, replay, and edit a real SDK-first Ergo project
10. [testing-notes](authoring/testing-notes.md) — Practical notes from
   real project testing, including live ingress and current product
   edges
11. [ingress-channel-guide](authoring/ingress-channel-guide.md) —
   HostedEvent ingress and process-channel authoring
12. [egress-channel-guide](authoring/egress-channel-guide.md) — Intent
    dispatch and durable-accept egress authoring
13. [concepts](authoring/concepts.md) — Cluster composition concepts
14. [invariants](invariants/INDEX.md) — Enforcement loci for all invariants
15. [terminology](system/terminology.md) — Canonical terms and usage

---

## By Topic

Directory structure is the topic map. No separate navigation aids needed.

- [system/](system/)
  Core laws and identity: ontology, execution model, freeze
  declaration, kernel closure, current architecture,
  kernel/prod separation, and terminology.
- [orchestration/](orchestration/)
  Supervision and trust boundaries: supervisor spec and adapter
  contract.
- [authoring/](authoring/)
  Building projects, clusters, graphs, adapters, and boundary
  channels: project convention, getting-started guide, authoring
  concepts, cluster spec, YAML format, loader contract, testing notes,
  ingress channel guide, and egress channel guide.
- [primitives/](primitives/)
  Primitive implementation contracts: Source, Compute, Trigger,
  Action, and Adapter manifests.
- [contracts/](contracts/)
  External interface specifications: UI ↔ Runtime contract and
  extension roadmap.
- [invariants/](invariants/)
  Phase boundaries and enforcement: 194 tracked invariants across 16
  phase files plus the rule registry.
- [ledger/](ledger/)
  Operational planning and doctrine risk tracking: closure register,
  dev-work ledgers, gap-work ledgers, and the decision log.
- [plans/](plans/)
  Working design-loop documents: blast-radius maps, option analysis,
  scope shaping, and phased execution plans while the design loop remains
  active.

---

## Authority Levels

Every document declares its authority in frontmatter. The active levels
and their change rules:

- **FROZEN**
  Change rule: v1 required to change.
  Documents: [ontology](system/ontology.md),
  [execution](system/execution.md), [freeze](system/freeze.md),
  [supervisor](orchestration/supervisor.md), and
  [adapter](orchestration/adapter.md).
- **STABLE**
  Change rule: additive changes only.
  Documents: [concepts](authoring/concepts.md),
  [cluster-spec](authoring/cluster-spec.md),
  [yaml-format](authoring/yaml-format.md),
  [primitives/](primitives/) (all five manifests), and
  [rule-registry](invariants/rule-registry.md).
- **CANONICAL**
  Change rule: tracks implementation.
  Documents include: [kernel](system/kernel.md),
  [current-architecture](system/current-architecture.md),
  [kernel-prod-separation](system/kernel-prod-separation.md),
  [terminology](system/terminology.md),
  [project-convention](authoring/project-convention.md),
  [getting-started-sdk](authoring/getting-started-sdk.md),
  [loader](authoring/loader.md),
  [testing-notes](authoring/testing-notes.md),
  [ingress-channel-guide](authoring/ingress-channel-guide.md),
  [egress-channel-guide](authoring/egress-channel-guide.md), and
  [invariants/](invariants/) (all phase files plus INDEX), along with
  canonical ledger records when their frontmatter declares `Authority:
  CANONICAL`.
- **CONTRACTS**
  Change rule: external interfaces.
  Documents: [ui-runtime](contracts/ui-runtime.md) and
  [extension-roadmap](contracts/extension-roadmap.md).
- **PROJECT**
  Change rule: working project/implementation records that track active design
  or delivery state.
  Documents: [plans/](plans/) and operational ledger entries such as
  [ledger/dev-work/](ledger/dev-work/).
- **ESCALATION**
  Change rule: exceptional authority record for conflict, override, or
  unresolved semantic risk that cannot be handled cleanly inside the normal
  decision/gap flow.
  Documents: specific escalation records when they exist.

---

## How Documents Work Together

- **[invariants/](invariants/)** tracks which invariants are enforced
  where, one file per phase.
- **[ledger/closure-register](ledger/closure-register.md)** tracks
  semantic gaps and their resolutions.
- **[ledger/dev-work/](ledger/dev-work/)** tracks implementation
  delivery, **[ledger/gap-work/](ledger/gap-work/)** tracks
  doctrine/risk gaps, and
  **[ledger/decisions/](ledger/decisions/)** records authority
  outcomes.
- **[plans/](plans/)** is the sanctioned pre-ledger or cross-ledger
  working area for iterative architecture/design loops that are not ready
  to be split cleanly across gap, decision, and dev-work files.
- All three reference spec documents (ontology, execution, etc.) as
  the source of truth.
- Decision records explain **why** a ruling was made. Top-level system,
  primitive, and authoring docs explain **what the current system is**.

---

## Crate-Local Documentation

READMEs under `crates/` are informational for their respective packages.
They are not authoritative for system behavior.
