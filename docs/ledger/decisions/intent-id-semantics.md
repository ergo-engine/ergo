---
Authority: PROJECT
Date: 2026-03-15
Decision-Owner: Sebastian (Architect)
Participants: Claude (Structural Auditor), Codex (Ontology Guardian)
Status: DECIDED
Scope: v1
Parent-Decision: v1-external-effect-intent-model.md
Resolves: GW-EFX-3G (partial — intent_id only)
---

# Decision: Intent ID Correlation Semantics

## Context

The v1 external effect intent model establishes that one action attempt
produces two correlated projections sharing a common `intent_id`: an
optional mirror write and an external intent record. The intent payload
shape decision establishes typed fields on `IntentRecord`.

This decision defines how `intent_id` is generated, what guarantees it
provides, and how it interacts with replay.

---

## Replay Constraint

This is the hard constraint that eliminates most options.

`hash_effect()` in `replay.rs` serializes the entire `ActionEffect` to
JSON via `serde_json::to_vec()`, then SHA-256 hashes the bytes.
`compare_decisions()` checks both structural equality and hash equality.
Both must match or replay fails.

Therefore: `intent_id` lives inside `IntentRecord`, which lives inside
`ActionEffect`. Any non-deterministic value in `intent_id` produces a
different hash on every run. Strict replay breaks.

`intent_id` must be deterministic — same inputs, same run, same ID.

---

## The Fork

### Option 1 — Deterministic derivation (chosen)

Derive `intent_id` from values that are identical between capture and
replay using the same hash idiom Ergo already uses for runtime
provenance and effect hashing.

### Option 2 — Random UUID, excluded from hash (rejected)

Generate a random UUID and exclude `intent_id` from the effect hash
computation.

**Why rejected:** Weakens replay integrity. The intent_id is the
correlation key between mirror writes and external intents. Excluding
it from verification means replay can't confirm the same correlation
was produced. Also requires special-case serialization logic to skip
one field during hashing — fragile and unlike any existing pattern.

### Option 3 — Per-step counter (rejected)

Increment a counter per step within a run.

**Why rejected:** Deterministic within a run but fragile across graph
changes. If the graph structure changes between capture and replay
(different number of actions firing), counters shift. Also not globally
unique — useless as a correlation key across runs for status feedback.

---

## Ruling

### Derivation scheme

`intent_id` is derived via SHA-256 from a structured, length-prefixed
input:

```
intent_id = "eid1:sha256:" + hex(sha256(
    len_prefix("eid1") +
    len_prefix(graph_id) +
    len_prefix(event_id) +
    len_prefix(node_runtime_id) +
    len_prefix(intent_kind) +
    len_prefix(intent_ordinal)
))
```

Where:
- `eid1` — version tag for the derivation scheme (effect intent id v1)
- `graph_id` — the run's graph identity
- `event_id` — the triggering event, unique per event in a run
- `node_runtime_id` — the action node's expanded runtime ID
- `intent_kind` — e.g. "place_order"
- `intent_ordinal` — zero-indexed position of this intent within the
  action's manifest `effects.intents` list

### Length-prefix encoding

Each input component is encoded as `u32_be(len) + utf8_bytes`. This
prevents ambiguity from string concatenation (e.g. `"e1" + "act"` vs
`"e1a" + "ct"`).

### Why these inputs

| Input | Why included | Deterministic? |
| --- | --- | --- |
| `eid1` | Version tag. Future scheme changes produce different IDs. | Yes (literal) |
| `graph_id` | Scopes to the run. | Yes (same in capture/replay) |
| `event_id` | Unique per event. Two firings of same action on different events get different IDs. | Yes (from capture bundle) |
| `node_runtime_id` | Unique per action node. Two actions on same event get different IDs. | Yes (expansion is deterministic, tested in cluster.rs) |
| `intent_kind` | Unique per intent kind. Same action emitting different intent kinds gets different IDs. | Yes (from manifest) |
| `intent_ordinal` | Unique per intent within an action. Same action emitting two intents of same kind (if allowed) gets different IDs. | Yes (from manifest declaration order) |

### Why field values are NOT included

The quintuple `(graph_id, event_id, node_runtime_id, intent_kind,
intent_ordinal)` is already unique per intent occurrence. Including
field values would be redundant and would make the ID unpredictable
from graph structure alone, reducing its usefulness as a correlation
key.

### Data availability requirement

The runtime emit path (`execute_action` in `execute.rs`) currently does
not receive `event_id` or `graph_id`. These must be plumbed through to
the intent emission site. This is implementation work, not a design
decision — the values exist at the host level and need to be threaded
to where intents are constructed.

---

## Pattern Consistency

This follows Ergo's existing idiom:

| Existing use | Input | Hash | Prefix |
| --- | --- | --- | --- |
| Runtime provenance | Graph structure + primitives | SHA-256 of JSON | `rpv1:sha256:` |
| Event payload | Raw payload bytes | SHA-256 | hex string |
| Effect hash | Serialized ActionEffect | SHA-256 | hex string |
| **Intent ID (new)** | **Derivation inputs** | **SHA-256** | **`eid1:sha256:`** |

Same tools (`sha2`, `hex`), same approach (structured input →
deterministic hash → prefixed string), same crates.

---

## Uniqueness Guarantee

Within a single run, `intent_id` is unique per intent occurrence
because:

1. `event_id` is unique per event (enforced by `HostedRunner` duplicate
   event ID check)
2. `node_runtime_id` is unique per node (enforced by graph expansion)
3. `intent_ordinal` is unique per intent within an action (manifest
   declaration order)

Across runs, `intent_id` is deterministic given identical graph
structure and event sequence — which is exactly what replay provides.

For cross-run correlation (e.g. egress status feedback referencing a
prior run's intent), the full `intent_id` string is globally unique in
practice because it includes `graph_id` and `event_id`.

---

## Representation

`intent_id` is a `String` field on `IntentRecord`:

```rust
pub struct IntentRecord {
    pub kind: String,
    pub intent_id: String,  // "eid1:sha256:{hex}"
    pub fields: Vec<IntentField>,
}
```

The prefixed format (`eid1:sha256:...`) enables future scheme evolution
without ambiguity. Code that parses intent IDs can check the prefix to
determine the derivation version.

---

## What This Does NOT Decide

- **Egress protocol framing** for intent_id transmission
- **Status feedback mechanism** (how egress reports back keyed by
  intent_id — that's GW-EFX-3H)
- **Mirror write correlation** beyond sharing the same intent_id on
  both projections

---

## Impacted Files

- `ergo_runtime::common` — `IntentRecord` struct gains `intent_id`
- `execute.rs` — intent emission site needs `event_id` and `graph_id`
  plumbed through
- `runner.rs` / host layer — derives intent_id before or during step
- New utility function for length-prefixed SHA-256 derivation
