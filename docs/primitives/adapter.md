---
Authority: STABLE
Version: v1
Last Updated: 2026-03-16
Last Amended: 2026-03-16
Scope: Declarative adapter contract for context, events, and accepted effects
---

> **Amended 2026-03-16** by Codex (Implementation Assistant)
> Clarified the stable adapter contract after the ingress/egress split:
> adapters declare accepted effect vocabulary and payload schemas, but
> do not own routing policy or concrete external I/O realization.

# Adapter Manifest — v1

An adapter is the bridge between the runtime and the external world.
It defines what context data is available to the graph, what external events
can trigger execution, and what effect kinds the runtime may request
against the adapter's declared acceptance surface.

This is the authoritative contract.

Adapters define:

- what context the graph may observe
- what semantic events may enter execution
- what effect kinds and payload schemas the graph may request

Adapters do **not** define routing policy, launch processes, or perform
real external I/O themselves.

---

## 1. Definition

An Adapter Primitive is the external interface layer that:

- populates ExecutionContext for graph evaluation
- emits ExternalEvents that trigger episodes
- declares which effects requested by Actions are accepted at the
  contract boundary
- captures replay-relevant data

An adapter answers three questions:

*"What data is available at this evaluation point?"* (context_keys)

*"What events can trigger graph execution?"* (event_kinds)

*"What effects can actions request?"* (accepts.effects)

Adapters are declarative contracts. Concrete post-episode dispatch
belongs to host. Concrete external I/O belongs to prod boundary
channels; host-internal effects may still be realized by host handlers.

---

## 2. Required Manifest Fields

Every adapter must declare all of the following.

---

### 2.1 Identity

```yaml
kind: adapter                    # MUST be literal "adapter"
id: string                       # ^[a-z][a-z0-9_]*$
version: semver                  # Adapter version (e.g., "1.0.0")
runtime_compatibility: semver    # Minimum runtime version supported
```

Rules:

- `kind` must be literal `"adapter"`
- `id` must start with lowercase letter, contain only lowercase letters, digits, underscores
- `version` must be valid semver
- `runtime_compatibility` must be valid semver; runtime must satisfy `runtime.version >= runtime_compatibility`

---

### 2.2 Context Keys

```yaml
context_keys:
  - name: string              # Key name (e.g., "price", "timestamp")
    type: String              # ValueType as string: "Number" | "Bool" | "String" | "Series"
    required: bool            # Always present vs optional
    writable: bool            # If true, actions may write via effects
    description: string       # Optional: human-readable purpose
```

Rules:

- `context_keys[].name` must be unique across all context keys
- `context_keys[].type` must be a valid ValueType: `Number`, `Bool`, `String`, or `Series`
- `context_keys[].writable` must be present (true or false)
- If `writable: true`, then `required: false` (writable keys may not exist initially)
- If any key has `writable: true`, adapter must accept `set_context` effect
- `description` is optional metadata

---

### 2.3 Event Kinds

```yaml
event_kinds:
  - name: string              # Event kind identifier (open world)
    payload_schema: JsonSchema # Draft 2020-12, self-contained
```

Rules:

- `event_kinds[].name` must be unique across all event kinds
- `event_kinds[].payload_schema` must be valid JSON Schema Draft 2020-12

---

### 2.4 Accepts (Effects)

```yaml
accepts:                      # Optional section
  effects:
    - name: string            # Effect kind identifier (e.g., "set_context")
      payload_schema: JsonSchema # Draft 2020-12, self-contained
```

Rules:

- `accepts.effects[].name` must be unique across all effects
- `accepts.effects[].payload_schema` must be valid JSON Schema Draft 2020-12
- `accepts` section is optional (adapters may accept zero effects)
- `accepts.effects` declares accepted effect vocabulary only. It does
  not, by itself, choose whether an accepted effect is realized by a
  host-internal handler or by a prod boundary channel
- During Action ↔ adapter composition, every declared
  `effects.intents[].name` must be accepted here and its typed field set
  must be structurally compatible with the corresponding
  `accepts.effects[].payload_schema`

---

### 2.5 Capture

```yaml
capture:
  format_version: string      # Capture bundle format version (must be non-empty)
  fields:                     # What fields are captured for replay
    - string                  # MUST be in CaptureFieldSet
```

Rules:

- `capture.format_version` must be a non-empty string
- `capture.fields[]` must reference valid CaptureFieldSet selectors

**CaptureFieldSet (current):**

- `event.<event_kind_name>` for each declared `event_kinds[].name`
- `meta.adapter_id`
- `meta.adapter_version`
- `meta.timestamp`

**Planned manifest extension (ADP-15/ADP-16):**

- `context.<key_name>` for each `context_keys[].name`
- `effect.<effect_name>` for each `accepts.effects[].name`

Note: same-ingestion Scope A replay already captures and verifies full
`set_context` effects via host-owned enrichment (see `08-replay.md`).
The manifest extension would make that coverage declarative in the
adapter contract.

---

## 3. JSON Schema Restrictions

All `payload_schema` fields (in `event_kinds` and `accepts.effects`) must comply with:

| Restriction | Rationale |
|-------------|-----------|
| Draft 2020-12 only | Pin to single standard |
| `additionalProperties: false` required for object schemas | Prevent schema evolution ambiguity |
| No external `$ref` | Self-contained schemas only |
| No `oneOf` / `anyOf` | Keeps validation simple |

---

## 4. Validation Rules (ADP-*)

These rules are checked during adapter registration (manifest-only validation).

### 4.1 Registration Rules

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| ADP-1 | ID format valid | `regex(id, /^[a-z][a-z0-9_]*$/)` |
| ADP-2 | Version valid semver | `semver.valid(version)` |
| ADP-3 | Runtime compatibility satisfied | `runtime.version >= runtime_compatibility` |
| ADP-4 | Provides something | `context_keys.len > 0 OR event_kinds.len > 0` |
| ADP-5 | Context key names unique | `unique(context_keys[].name)` |
| ADP-6 | Context key types valid | `all(context_keys[].type in {Number, Bool, String, Series})` |
| ADP-7 | Event kind names unique | `unique(event_kinds[].name)` |
| ADP-8 | Event schemas valid JSON Schema | `json_schema.validate(payload_schema, draft: 2020-12)` |
| ADP-9 | Capture format version present | `capture.format_version != ""` |
| ADP-10 | Capture fields referentially valid | `all(capture.fields[] in CaptureFieldSet(adapter))` |
| ADP-11 | Writable flag must be present | `all(context_keys[].writable is present)` |
| ADP-12 | Effect names unique | `unique(accepts.effects[].name)` |
| ADP-13 | Effect schemas valid | `all(accepts.effects[].payload_schema is valid Draft 2020-12)` |
| ADP-14 | Writable implies set_context accepted | `any(writable == true) => accepts contains "set_context"` |
| ADP-17 | Writable keys cannot be required | `all(writable == true => required == false)` |
| ADP-18 | Required semantic event fields map to context keys | `all(required(event.payload_schema) fields exist in context_keys with matching types)` |
| ADP-19 | Materialized event field types are supported | `event payload object fields map only to Number/Bool/String/Series` |

**ADP-18 scope note:** This rule intentionally validates only fields listed in `payload_schema.required`. If `required` is omitted, ADP-18 vacuously passes.

### 4.2 Deferred Rules (Adapter Manifest Completeness)

The following rules are deferred as adapter-manifest completeness items.
Same-ingestion Scope A replay already verifies host-owned effect
integrity including `set_context` writes. These rules would require the
manifest to explicitly declare that coverage in `capture.fields`. If
this work is revived, open a dedicated gap-work file first to decide
whether manifests canonically declare context/effect capture coverage
and what guarantee that implies across ingestion modes:

| Rule ID | Rule | Status |
|---------|------|--------|
| ADP-15 | Writable keys must be capturable | Deferred: REP-SCOPE |
| ADP-16 | Write effect must be capturable | Deferred: REP-SCOPE |

**Predicates (for future implementation):**

- ADP-15: `all(writable == true => "context." + name in capture.fields)`
- ADP-16: `any(writable == true) => "effect.set_context" in capture.fields`

### 4.3 Enforcement Mapping

| Rule ID | Phase | Error Variant | Test Name |
|---------|-------|---------------|-----------|
| ADP-1 | Registration | `InvalidAdapter::InvalidId` | `adp_1_invalid_id_rejected` |
| ADP-2 | Registration | `InvalidAdapter::InvalidVersion` | `adp_2_invalid_version_rejected` |
| ADP-3 | Registration | `InvalidAdapter::InvalidRuntimeCompatibility` / `InvalidAdapter::IncompatibleRuntime` | `adp_3_invalid_runtime_compatibility_rejected` / `adp_3_incompatible_runtime_rejected` |
| ADP-4 | Registration | `InvalidAdapter::ProvidesNothing` | `adp_4_empty_adapter_rejected` |
| ADP-5 | Registration | `InvalidAdapter::DuplicateContextKey` | `adp_5_duplicate_context_key_rejected` |
| ADP-6 | Registration | `InvalidAdapter::InvalidContextKeyType` | `adp_6_invalid_context_type_rejected` |
| ADP-7 | Registration | `InvalidAdapter::DuplicateEventKind` | `adp_7_duplicate_event_kind_rejected` |
| ADP-8 | Registration | `InvalidAdapter::InvalidPayloadSchema` | `adp_8_invalid_schema_rejected` |
| ADP-9 | Registration | `InvalidAdapter::NoCaptureFormat` | `adp_9_no_capture_format_rejected` |
| ADP-10 | Registration | `InvalidAdapter::InvalidCaptureField` | `adp_10_invalid_capture_field_rejected` |
| ADP-11 | Registration | `InvalidAdapter::MissingWritableFlag` | `adp_11_missing_writable_flag_rejected` |
| ADP-12 | Registration | `InvalidAdapter::DuplicateEffectName` | `adp_12_duplicate_effect_name_rejected` |
| ADP-13 | Registration | `InvalidAdapter::InvalidEffectSchema` | `adp_13_invalid_effect_schema_rejected` |
| ADP-14 | Registration | `InvalidAdapter::WritableWithoutSetContext` | `adp_14_writable_without_set_context_rejected` |
| ADP-15 | Registration | `InvalidAdapter::WritableKeyNotCaptured` | — (Deferred) |
| ADP-16 | Registration | `InvalidAdapter::SetContextNotCaptured` | — (Deferred) |
| ADP-17 | Registration | `InvalidAdapter::WritableKeyRequired` | `adp_17_writable_key_required_rejected` |
| ADP-18 | Registration | `InvalidAdapter::RequiredEventFieldNotProvided` / `InvalidAdapter::RequiredEventFieldTypeMismatch` | `adp_18_*` |
| ADP-19 | Registration | `InvalidAdapter::EventPayloadSchemaNotObject` / `InvalidAdapter::UnsupportedEventFieldType` | `adp_19_*` |

**Enforcement location:** `crates/kernel/adapter/src/validate.rs`
**Test location:** `crates/kernel/adapter/tests/validation.rs`

---

## 5. Composition Rules (COMP-*)

These rules validate adapter compatibility with other components.

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| COMP-1 | Source context requirements satisfied | `source.requires.context.filter(required).keys ⊆ adapter.provides.context.keys` |
| COMP-2 | Source context types match | `∀ k where required: source.requires.context[k].ty == adapter.provides.context[k].ty` |
| COMP-3 | Capture format version supported | `runtime.supports_capture(adapter.capture.format_version)` |

### 5.1 Composition Enforcement Mapping

| Rule ID | Phase | Error Variant | Test Name |
|---------|-------|---------------|-----------|
| COMP-1 | Composition | `CompositionError::MissingContextKey` | `comp_1_missing_context_key_rejected` |
| COMP-2 | Composition | `CompositionError::ContextTypeMismatch` | `comp_2_context_type_mismatch_rejected` |
| COMP-3 | Composition | `CompositionError::UnsupportedCaptureFormat` | `comp_3_unsupported_capture_format_rejected` |

**Enforcement location:** `crates/kernel/adapter/src/composition.rs`
**Test location:** `crates/kernel/adapter/tests/composition_tests.rs`

---

## 6. Adapter Provides

At composition time, the adapter is represented by an `AdapterProvides` structure:

```rust
pub struct AdapterProvides {
    pub context: HashMap<String, ContextKeyProvision>,
    pub events: HashSet<String>,
    pub effects: HashSet<String>,
    pub event_schemas: HashMap<String, serde_json::Value>,
    pub capture_format_version: String,
    pub adapter_fingerprint: String,
}

pub struct ContextKeyProvision {
    pub ty: String,           // ValueType as string
    pub required: bool,
    pub writable: bool,
}
```

Built from `AdapterManifest` via `AdapterProvides::from_manifest()` after registration validation passes.

---

## 7. Prohibited Behavior

An adapter may not:

- Inject context keys not declared in manifest
- Emit event kinds not declared in manifest
- Accept effects not declared in manifest
- Forward episode results via context (see Provenance Rule in supervisor.md §2.2)

Violation invalidates the adapter.

---

## 8. Canonical Adapter Example

### Minimal Valid Manifest

```yaml
kind: adapter
id: minimal_adapter
version: 1.0.0
runtime_compatibility: 0.1.0

context_keys:
  - name: price
    type: Number
    required: true
    writable: false

event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false

capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

This manifest passes all 17 implemented ADP rules:

- ADP-1: `id` matches `^[a-z][a-z0-9_]*$`
- ADP-2: `version` is valid semver
- ADP-3: `runtime_compatibility` is valid semver
- ADP-4: Has both `context_keys` and `event_kinds`
- ADP-5: Single context key, no duplicates
- ADP-6: `type: Number` is valid ValueType
- ADP-7: Single event kind, no duplicates
- ADP-8: Valid JSON Schema with `additionalProperties: false`
- ADP-9: `format_version: "1"` is non-empty
- ADP-10: All capture fields are in CaptureFieldSet
- ADP-11: `writable: false` is present
- ADP-12: No effects, no duplicate check needed
- ADP-13: No effect schemas to validate
- ADP-14: No writable keys, so no `set_context` required
- ADP-17: No writable keys, so no required conflict
- ADP-18: No required semantic event fields, so required-field context mapping is satisfied
- ADP-19: Event payload shape remains within supported materialization types

### Full-Featured Manifest (with writable keys)

```yaml
kind: adapter
id: trading_adapter
version: 1.0.0
runtime_compatibility: 0.1.0

context_keys:
  - name: price
    type: Number
    required: true
    writable: false
    description: Current price from market data feed

  - name: last_signal
    type: String
    required: false
    writable: true
    description: State for once/latch patterns

event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false

  - name: command
    payload_schema:
      type: object
      properties:
        action:
          type: string
      required:
        - action
      additionalProperties: false

accepts:
  effects:
    - name: set_context
      payload_schema:
        type: object
        properties:
          writes:
            type: array
            items:
              type: object
              properties:
                key:
                  type: string
                value: {}
              required:
                - key
                - value
              additionalProperties: false
        required:
          - writes
        additionalProperties: false

capture:
  format_version: "1"
  fields:
    - event.pump
    - event.command
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

### Invalid Manifest Examples (One per Rule)

Each example below is a complete manifest with a single intentional violation.

**ADP-1: ID format invalid**

```yaml
kind: adapter
id: Bad-Id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: true
    writable: false
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-2: Version invalid semver**

```yaml
kind: adapter
id: good_id
version: nope
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: true
    writable: false
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-3: Runtime compatibility not satisfied**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 999.0.0
context_keys:
  - name: price
    type: Number
    required: true
    writable: false
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-4: Provides nothing (no context_keys, no event_kinds)**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys: []
event_kinds: []
capture:
  format_version: "1"
  fields:
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-5: Duplicate context key names**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: true
    writable: false
  - name: price
    type: Number
    required: false
    writable: false
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-6: Invalid context key type**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Foo
    required: true
    writable: false
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-7: Duplicate event kind names**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: true
    writable: false
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-8: Invalid event payload schema (banned oneOf)**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: true
    writable: false
event_kinds:
  - name: pump
    payload_schema:
      oneOf:
        - type: string
        - type: number
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-9: Empty capture.format_version**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: true
    writable: false
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: ""
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-10: Capture field not in CaptureFieldSet**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: true
    writable: false
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.unknown
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-11: Missing writable flag**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: true
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-12: Duplicate effect names**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: true
    writable: false
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
accepts:
  effects:
    - name: set_context
      payload_schema:
        type: object
        additionalProperties: false
    - name: set_context
      payload_schema:
        type: object
        additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-13: Invalid effect payload schema (external $ref)**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: true
    writable: false
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
accepts:
  effects:
    - name: set_context
      payload_schema:
        $ref: "https://example.com/schema.json"
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-14: Writable key without set_context effect**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: false
    writable: true
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
accepts:
  effects:
    - name: other_effect
      payload_schema:
        type: object
        additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-17: Writable key cannot be required**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: price
    type: Number
    required: true
    writable: true
event_kinds:
  - name: pump
    payload_schema:
      type: object
      additionalProperties: false
accepts:
  effects:
    - name: set_context
      payload_schema:
        type: object
        additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.pump
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-18: Required semantic event field not mapped to context**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: symbol
    type: String
    required: true
    writable: false
event_kinds:
  - name: price_tick
    payload_schema:
      type: object
      properties:
        price:
          type: number
      required: [price]
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.price_tick
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

**ADP-19: Unsupported materialization type in event payload schema**

```yaml
kind: adapter
id: good_id
version: 1.0.0
runtime_compatibility: 0.1.0
context_keys:
  - name: metadata
    type: String
    required: false
    writable: false
event_kinds:
  - name: price_tick
    payload_schema:
      type: object
      properties:
        metadata:
          type: object
          additionalProperties: false
      additionalProperties: false
capture:
  format_version: "1"
  fields:
    - event.price_tick
    - meta.adapter_id
    - meta.adapter_version
    - meta.timestamp
```

---

## 9. Scope

This document defines Adapter Manifest v1.

Out of scope:

- Trust tiers
- Human review processes
- Sandboxing
- Attestation

The contract is the only gate.

---

## 10. Contract Stability

This contract is STABLE.

Breaking changes require a manifest version bump.
