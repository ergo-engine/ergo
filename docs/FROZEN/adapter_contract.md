---
Authority: FROZEN
Version: v0
Last Updated: 2025-12-22
Scope: Trust boundary, replay guarantees, capture requirements
Verified Against Tag: v0.28-kernel-closed
Change Rule: v1 only
---

# Adapter Contract — v0

This document defines the minimal compliance requirements for adapters that supply data to
Source primitives.

Adapters form a trust boundary between the external world and the execution engine.

---

## 1. Determinism Under Replay

An adapter must produce identical outputs given identical captured input sequences.

- Internal caching or state is permitted only if it does not affect replay outputs.
- External nondeterminism must be capturable.
- The adapter must be a pure function of its captured inputs for replay purposes.

---

## 2. Capture Support

Adapters must support input capture at the request of the orchestrator.

- Capture format may be implementation-defined in v0.
- Captured inputs must be sufficient to reproduce adapter outputs during replay.

Capture format standardization is a v1 concern required for:
- Cross-environment replay
- Third-party adapters
- Adapter migration without re-capture

---

## 3. Declared Semantic Shaping

Any transformation that alters the semantic meaning of values must be declared.

Examples of semantic shaping that must be declared (non-exhaustive):
- Currency conversion
- Unit normalization
- Timezone conversion
- Aggregation (e.g., tick → bar)
- Missing data interpolation or fill logic
- Filtering or sampling

If a transformation changes the meaning of values as observed by downstream Compute nodes,
it must be declared.

Declaration may occur via:
- Source manifest parameters, or
- Adapter metadata explicitly referenced by the Source

Undeclared semantic shaping is forbidden.

Enforcement of declared semantic shaping is trust-based in v0; adapters are expected to comply,
and violations invalidate Source guarantees. Automated validation may be introduced in future
versions.

---

## 4. Scope and Responsibility

Adapters are responsible for:
- Interfacing with external systems
- Capturing inputs for replay
- Enforcing determinism guarantees
- Declaring any semantic transformations

Adapters are not responsible for:
- Graph execution semantics
- Trigger or action behavior
- Orchestration decisions
- Validation of downstream node logic

---

## 5. Trust Boundary

Adapters are the only place where:
- External nondeterminism enters the system
- Semantics can be silently altered (if shaping is undeclared)

Therefore, adapters are trusted components. The correctness of Source guarantees depends on
adapter compliance with this contract.

Without adapter compliance:
- Replay determinism is not guaranteed
- Source outputs may have different meanings across deployments
- Backtest/paper/live alignment cannot be assured

---

## 6. Out of Scope

This contract does not define:

- Adapter SDKs or APIs
- Capture file formats (implementation-defined in v0)
- Adapter discovery or loading mechanisms
- Multi-adapter coordination
- Adapter versioning or migration

These concerns are deferred beyond v0.
