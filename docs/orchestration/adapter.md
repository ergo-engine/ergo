---
Authority: FROZEN
Version: v0
Last Updated: 2026-03-26
Scope: Trust boundary, replay guarantees, capture requirements
Verified Against Tag: v1.0.0-alpha.1
Change Rule: v1 only
---

# Adapter Contract — v0

This document defines the minimal compliance requirements for adapter
manifests used by adapter-bound runs.

In the current product surface, adapters are declarative compatibility
contracts. Host-owned ingress and egress channels perform real external
I/O; adapters declare the event, effect, context, capture, and runtime
compatibility surfaces the host validates before execution.

---

## 1. Determinism Under Replay

An adapter must produce identical outputs given identical captured input sequences.

- Internal caching or state is permitted only if it does not affect replay outputs.
- External nondeterminism must be capturable.
- The adapter must be a pure function of its captured inputs for replay purposes.

---

## 2. Capture Support

Adapters must declare capture support for adapter-bound replay.

- Adapter manifests declare `capture.format_version` and `capture.fields`.
- Current composition accepts adapter capture format `1`.
- Captured ingress inputs must be sufficient to reproduce adapter-bound
  execution during replay.
- The host writes the replay bundle and enriches it with host-owned
  provenance and effect records.

---

## 3. Declared Semantic Shaping

Any transformation that alters the semantic meaning of values must be
made explicit in the live declarative contract.

Examples of semantic shaping that must be declared (non-exhaustive):

- Currency conversion
- Unit normalization
- Timezone conversion
- Aggregation (e.g., tick → bar)
- Missing data interpolation or fill logic
- Filtering or sampling

If a transformation changes the meaning of values as observed by downstream Compute nodes,
it must be declared.

Current enforcement is concrete rather than generic. The live surface is:

- adapter manifest structure (`context_keys`, `event_kinds`,
  `accepts.effects`, `capture`)
- source/action composition against that declared structure
- specific validation such as required event-field/context compatibility

There is no separate generic "adapter metadata" declaration channel for
sources to reference today. Undeclared semantic shaping remains
forbidden by contract, but enforcement is limited to the concrete
manifest/composition rules above.

---

## 4. Scope and Responsibility

Adapters are responsible for:

- Declaring accepted event kinds, effect kinds, and context keys
- Declaring capture requirements and runtime compatibility
- Supporting deterministic adapter-bound replay for the declared surface
- Providing the declarative contract Source/Action composition validates against

Adapters are not responsible for:

- Owning live ingress or egress I/O processes
- Graph execution semantics
- Trigger or action behavior
- Orchestration decisions
- Validation of downstream node logic

---

## 5. Trust Boundary

External nondeterminism enters through ingress payloads and leaves
through post-episode host dispatch to boundary channels. Adapters are
still trusted declarative components because the correctness of
adapter-bound Source guarantees and replay compatibility depends on the
adapter manifest accurately describing that boundary surface.

Without adapter compliance:

- Replay determinism is not guaranteed
- Adapter-bound Source outputs may have different meanings across deployments
- Historical/paper/live alignment cannot be assured

---

## 6. Out of Scope

This contract does not define:

- Adapter SDKs or APIs
- Multi-adapter coordination
- Capture/adapter migration strategy across incompatible versions
- Custom discovery/plugin systems beyond the current project/profile loading model

These concerns are deferred beyond v0.
