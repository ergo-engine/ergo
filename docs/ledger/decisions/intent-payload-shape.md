---
Authority: PROJECT
Date: 2026-03-15
Decision-Owner: Sebastian (Architect)
Participants: Claude (Structural Auditor), Codex (Ontology Guardian)
Status: DECIDED
Scope: v1
Parent-Decision: v1-external-effect-intent-model.md
Resolves: GW-EFX-3G (partial — payload shape only)
---

# Decision: ActionEffect v1 Intent Payload Shape

## Context

The v1 external effect intent model decision established that external
intents like `place_order` are first-class effect kinds with manifest-
declared fields. It did not resolve the concrete payload representation:
typed fields or arbitrary JSON.

Current `ActionEffect` is `kind: String` + `writes: Vec<EffectWrite>`.
External intents need a payload field. This decision defines its shape.

---

## The Fork

### Option 1 — Typed fields (chosen)

The manifest declares each intent field's name, type, and source. The
runtime constructs a typed record from the action's input snapshot. The
validator rejects at registration time if sources don't match declared
inputs or types mismatch.

```rust
IntentRecord {
    kind: "place_order",
    intent_id: "...",
    fields: vec![
        IntentField { name: "symbol", value: Value::String("EURUSD") },
        IntentField { name: "side", value: Value::String("buy") },
        IntentField { name: "qty", value: Value::Number(100.0) },
    ],
}
```

### Option 2 — Arbitrary JSON (rejected)

The manifest declares the intent kind and a JSON Schema. The runtime
constructs a `serde_json::Value`. Validation against the schema happens
at runtime, not registration.

```rust
IntentRecord {
    kind: "place_order",
    intent_id: "...",
    payload: json!({
        "symbol": "EURUSD",
        "side": "buy",
        "qty": 100.0
    }),
}
```

---

## Ruling

**Option 1 — typed fields — for the core intent model.**

Intent payloads are typed program data represented as
`Vec<IntentField>`, where each `IntentField` has a `name: String` and
`value: Value`. Fields are declared in the action manifest's
`effects.intents[].fields` with `from_input` or `from_param` source
bindings. The runtime constructs fields mechanically from the action's
input snapshot. No action implementation code touches the intent payload.

At the **egress dispatch boundary only**, the host projects the typed
intent to JSON for transmission to the egress process. This is a
one-way lowering — the canonical representation inside Ergo is typed,
and the wire format is a derived projection.

A **compatibility check at startup** verifies that the typed intent
field schema and the adapter's `accepts.effects` JSON Schema for that
effect kind are structurally compatible. Mismatches are caught before
any events are processed.

---

## Why Option 2 Was Rejected

1. **Intent is domain semantics, not transport encoding.** JSON is a
   wire concern. Ergo's core should represent effect intent as typed
   program data, then project to wire format at the boundary.

2. **The flexibility is fake today.** Runtime values are
   `Number | Bool | String | Series | Event`. Action inputs are these
   same types. There are no arbitrary object graphs in the type system.
   Typed fields aren't a limitation — they reflect what the system
   actually supports.

3. **Replay determinism.** Replay compares full effect equality and
   hash of serialized bytes. JSON payloads introduce representational
   ambiguity (key ordering, number formatting, whitespace). Typed
   fields give tighter determinism.

4. **Pattern consistency.** `effects.writes` is already typed — each
   write has a name, type, and value. Making `effects.intents` untyped
   would be an inconsistency users notice immediately.

5. **Validation timing.** Typed fields catch bad intent declarations at
   registration time. JSON Schema validation defers errors to runtime.
   Earlier is better.

---

## Concrete Types

### IntentField

```rust
pub struct IntentField {
    pub name: String,
    pub value: Value,
}
```

Where `Value` is the existing `ergo_runtime::common::Value` enum:

```rust
pub enum Value {
    Number(f64),
    Bool(bool),
    String(String),
    Series(Vec<f64>),
}
```

### IntentRecord (new, lives in ActionEffect or alongside it)

```rust
pub struct IntentRecord {
    pub kind: String,
    pub intent_id: String,  // format TBD per intent_id decision
    pub fields: Vec<IntentField>,
}
```

### ActionEffect evolution

Current:
```rust
pub struct ActionEffect {
    pub kind: String,
    pub writes: Vec<EffectWrite>,
}
```

v1 extends with:
```rust
pub struct ActionEffect {
    pub kind: String,
    pub writes: Vec<EffectWrite>,
    pub intents: Vec<IntentRecord>,  // new
}
```

Canonical stream semantics (enforced):

- **Internal effect:** `kind == "set_context"`, `writes` may be non-empty,
  `intents` must be empty.
- **External effect:** `kind == <intent kind>`, `writes` must be empty,
  `intents` must be non-empty, and every `intent.kind` in that record
  must equal `effect.kind`.

An action with both internal writes and external intents emits multiple
`ActionEffect` records in deterministic order:
1. internal `set_context` effect (top-level writes + mirror writes),
2. one external effect per intent kind.

---

## Manifest Declaration (recap from parent decision)

```yaml
effects:
  intents:
    - name: place_order
      fields:
        - name: symbol
          type: String
          from_input: symbol
        - name: side
          type: String
          from_input: side
        - name: qty
          type: Number
          from_input: qty
      mirror_writes:
        - name: last_order_symbol
          type: String
          from_field: symbol
```

### Field source rules

- `from_input` references a declared action input. Must type-match.
- `from_param` references a declared action parameter. Must type-match.
- Exactly one of `from_input` or `from_param` per field.
- All computation stays in the graph. Intent field sources are
  pass-through, not expressions.

---

## Egress Boundary Projection

When the host dispatches an intent to an egress process:

1. Each `IntentField` is converted to its JSON representation using the
   same `Value → serde_json::Value` conversion that already exists for
   `EffectWrite` (see `effects.rs::runtime_value_to_json()`).
2. The resulting JSON object is the egress wire payload.
3. The adapter's `accepts.effects` JSON Schema for the intent kind is
   used at startup to verify compatibility — not at dispatch time.

This means the egress process receives:

```json
{
  "kind": "place_order",
  "intent_id": "...",
  "fields": {
    "symbol": "EURUSD",
    "side": "buy",
    "qty": 100.0
  }
}
```

The exact wire format (field naming, envelope shape) is an egress
protocol concern, not a core model concern. The above is illustrative.

---

## Registration-Time Validation

The action manifest validator must check at registration:

1. Every `from_input` references a declared action input by name.
2. Every `from_param` references a declared action parameter by name.
3. Source type matches declared field type.
4. Field names within an intent are unique.
5. `mirror_writes[].from_field` references a declared field in the
   same intent (per parent decision).

These should become PHASE_INVARIANTS entries when implemented.

---

## Startup Compatibility Check

At run startup (after manifest registration, before events):

1. For each intent kind declared by actions in the graph, check that
   the adapter's `accepts.effects` includes a matching entry.
2. Verify that the typed field schema (names + types) is compatible
   with the adapter's JSON Schema for that effect kind.
3. Reject the run if incompatible.

This is a new validation step in the host run path, analogous to
existing source/action adapter composition checks.

---

## What This Does NOT Decide

- **`intent_id` format and generation.** Separate decision (1b-ii).
- **Egress wire protocol.** How the JSON projection is framed and
  transmitted to the egress process.
- **Complex/nested payloads.** If v2 needs richer structure, `Value`
  can be extended. This decision does not foreclose that.

---

## Impacted Files

- `ergo_runtime::common` — new `IntentField`, `IntentRecord` types
- `ActionEffect` — gains `intents: Vec<IntentRecord>` field
- Action manifest validator — new registration checks
- Host run path — startup compatibility check
- `effects.rs` — JSON projection at egress boundary
- PHASE_INVARIANTS.md — new invariant entries for registration checks
