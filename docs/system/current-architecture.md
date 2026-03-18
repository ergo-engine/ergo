---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-16
Owner: Sebastian (Architect)
Scope: Current v1 system architecture, authored artifacts, and runtime roles
Change Rule: Operational log
---

# Current Architecture

This document explains the current v1 system design directly.

Read this file when you want to understand what Ergo is today without
reconstructing the model from decision records.

---

## 1. What Users Author

Users author five semantic artifact kinds inside a project that also
contains `Cargo.toml`, `ergo.toml`, fixtures, and captures:

- **Implementations**
  Concrete Source, Compute, Trigger, and Action implementations.
- **Graphs**
  Cluster/graph compositions that wire implementations together.
- **Adapters**
  Declarative contracts for context, events, and accepted effect
  kinds.
- **Ingress channels**
  Prod boundary code that delivers external events into host
  execution.
- **Egress channels**
  Prod boundary code that receives effect intent from host and performs
  true external I/O.

Projects may also contain fixtures and capture artifacts, but those are
operational inputs/outputs rather than authored semantic components.

---

## 1A. Product Surface

The intended primary product surface is the Rust SDK layer over host +
loader.

In practice that means:

- a production Ergo application is a Rust crate
- user code registers custom primitives in-process
- named profiles live in `ergo.toml`
- CLI remains supporting tooling for validation, replay, and explicit
  path-based runs

The SDK crate itself is now delivered by `feat/sdk-rust`, and the
engine architecture supports it cleanly through the host + loader
library boundaries. `ergo init` now provides Rust-crate scaffolding
and generated sample-project UX on top of that surface.

---

## 2. The Runtime Roles

Ergo's current execution model distinguishes five runtime-facing roles:

- **Action**
  Emits effect intent from graph execution.
- **Adapter**
  Declares the accepted external contract.
- **Host**
  Runs canonical orchestration, drains post-episode effects, and
  manages capture/replay.
- **Ingress channel**
  Brings external events into host execution.
- **Egress channel**
  Takes effect intent out of host execution to real external systems.

The important separation is:

- **Adapters are declarative contracts**
- **Host is the orchestration and dispatch locus**
- **Ingress and egress channels are prod I/O realizations**

---

## 3. End-To-End Flow

The live canonical path is:

```text
ingress channel -> host -> graph executes -> host dispatches -> egress channel
```

In more detail:

1. An ingress channel produces `HostedEvent`.
2. Host validates/binds the event through the adapter contract when the
   run is adapter-bound.
3. The graph executes through kernel/runtime semantics.
4. Actions emit effect intent.
5. Host drains buffered effects after the episode.
6. Host realizes host-internal effects locally and routes external
   effect kinds to egress channels.
7. Egress channels return durable-accept acknowledgments for dispatched
   external intents.
8. Host writes a capture artifact for replay and audit.

---

## 4. Effect Model

Action effects now have two distinct projections:

- **Host-internal projection**
  `effects.writes` and `mirror_writes` project into the host-internal
  `set_context` effect kind.
- **External projection**
  `effects.intents` project into real external effect kinds such as
  `place_order`.

One Action may therefore emit:

- only `set_context`
- only external intent kinds
- both internal and external projections in the same attempt

When both exist, host-internal `set_context` projection precedes
external intent projection.

---

## 5. Replay Posture

Replay is capture-driven.

- Host-internal effects may be replay-realized when needed for
  deterministic reconstruction.
- Truly external effects are re-derived and verified against captured
  effect/intention integrity.
- Truly external effects are **not** re-executed against live systems
  during replay.

Adapter and runtime provenance are replay-strict.

Egress provenance is currently **audit-only** for replay: it is stored
for traceability but does not gate strict replay success.

---

## 6. Current v1 Limits

The current v1 shape still has a few explicit limits:

- Canonical host run intentionally supports **one ingress channel per
  run**. Projects that need many live feeds must multiplex them
  upstream into one ingress channel.
- Egress routing is configured through `EgressConfig` and current
  path-based runs often pass `--egress-config`; project-mode
  resolution now exists through the shared loader + SDK surface.
- Rust-crate scaffolding now exists through `ergo init`; optional
  future CLI project conveniences remain secondary to the SDK-first
  path.
- The scaffolded sample ingress and egress channel programs now target
  Python 3 instead of POSIX shell. Projects still need a local
  `python3` command available for the generated live-profile examples.

These are current product-surface limits, not hidden semantic gaps.

---

## 7. Where To Read Next

- [Kernel/Prod Separation](kernel-prod-separation.md)
- [Project Convention](../authoring/project-convention.md)
- [Getting Started with Ergo SDK](../authoring/getting-started-sdk.md)
- [Action Primitive Manifest](../primitives/action.md)
- [Adapter Manifest](../primitives/adapter.md)
- [Ingress Channel Guide](../authoring/ingress-channel-guide.md)
- [Egress Channel Guide](../authoring/egress-channel-guide.md)
- [Replay Phase Invariants](../invariants/08-replay.md)
