---
Authority: STABLE
Version: v1
Last Updated: 2026-02-01
Last Amended: 2026-02-01
---

> **Amended 2026-02-01** by Claude (Structural Auditor)
> Added `requires.context` schema (§2.7), validation rules table (§4), enforcement mapping (§4.2),
> composition rules (§4.3). Removed `context` from output types (§2.2) — context access is via
> `requires.context`. Added example source manifest with context requirements (§6).
> Origin: Phase 2 (Source Contract) of EXTENSION_CONTRACTS_ROADMAP.md

# Source Primitive Manifest — v1

A source defines what data exists at an evaluation point.
It introduces originating values into the graph without transformation, inference, or side effects.

This is the authoritative contract.

---

## 1. Definition

A Source Primitive is a deterministic data origin that introduces
external or contextual data into the graph as typed values or series.

Sources:
- do not transform data
- do not infer signals
- do not emit events
- do not cause side effects
- are pure providers

A source answers one question:

*"What data is available at this evaluation point?"*

---

## 2. Required Manifest Fields

Every source primitive must declare all of the following.

---

### 2.1 Identity

```yaml
id: string
version: string
kind: source
```

Rules:
- `id` must start with lowercase letter, contain only lowercase letters, digits, underscores (`^[a-z][a-z0-9_]*$`)
- `version` must be valid semver
- `kind` must be `source`

---

### 2.2 Outputs (Primary Contract)

```yaml
outputs:
  - name: string
    type: series | number | bool | string
```

Rules:
- Sources do not take inputs
- All outputs are named and typed
- Output types must be valid ValueType: `Number`, `Bool`, `String`, or `Series`
- Output names must be unique
- At least one output is required
- Outputs must always be produced when evaluated

---

### 2.3 Parameters (Configuration Only)

```yaml
parameters:
  - name: string
    type: int | number | bool | string | enum
    default: any
    bounds: optional
```

Rules:
- Parameters configure what data is exposed
- Parameters are static presets
- Parameters must be serializable
- Parameters do not change at runtime
- If `default` is present, its type must match the declared parameter `type`

Examples:
- identifier
- interval
- lookback window
- field selection

---

### 2.4 Execution Semantics

```yaml
execution:
  cadence: continuous
  deterministic: true
```

Rules:
- Sources are evaluated on every engine tick
- Cadence is always continuous in v0
- Determinism is required

---

### 2.5 State

```yaml
state:
  allowed: false
```

Rules:
- Sources may not hold internal state
- Caching, buffering, or accumulation is forbidden
- Any temporal behavior must be modeled downstream

---

### 2.6 Side Effects

```yaml
side_effects: false
```

Rules:
- Sources may not:
  - write files
  - mutate global state
  - emit events
  - perform actions
- External reads are permitted only through orchestrator-managed adapters

The source primitive itself is declarative, not imperative.

---

### 2.7 Context Requirements

```yaml
requires:
  context:
    - name: string            # Key from adapter's context_keys
      type: ValueType         # Must match adapter's declared type: Number | Bool | String | Series
      required: bool          # If false, key may be absent at runtime
```

Rules:
- `requires.context` declares what adapter-provided context keys this source needs
- Keys with `required: true` must exist in the adapter's `context_keys` with matching type
- Keys with `required: false` may be absent at runtime; the source uses a default value
- Type must match adapter's declared type exactly (enforced at composition time via COMP-1/COMP-2)
- At runtime, missing required keys produce `ExecError::MissingRequiredContextKey`; type mismatches produce `ExecError::ContextKeyTypeMismatch`

---

## 3. Input Prohibition (Critical)

Source primitives take no inputs.

Hard rule:
- No `inputs` section allowed
- No graph wiring into sources
- All dependencies must be parameters or adapter context (via `requires.context`)

This prevents feedback loops and preserves causality.

---

## 4. Validation Rules (SRC-*)

### 4.1 Registration Rules

These rules are checked when a source manifest is registered.

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| SRC-1 | ID format valid | `regex(id, /^[a-z][a-z0-9_]*$/)` |
| SRC-2 | Version valid semver | `semver.valid(version)` |
| SRC-3 | Kind is "source" | `kind == "source"` |
| SRC-4 | No inputs declared | `inputs.len == 0` |
| SRC-5 | At least one output | `outputs.len >= 1` |
| SRC-6 | Output names unique | `unique(outputs[].name)` |
| SRC-7 | Output types valid | `all(outputs[].type ∈ ValueType)` |
| SRC-8 | State not allowed | `state.allowed == false` |
| SRC-9 | Side effects not allowed | `side_effects == false` |
| SRC-12 | Execution deterministic | `execution.deterministic == true` |
| SRC-13 | Cadence is continuous | `execution.cadence == continuous` |
| SRC-14 | ID unique in registry | `id ∉ SourceRegistry.ids` |
| SRC-15 | Parameter default type matches declared type | `parameters[].default == None || typeof(parameters[].default) == parameters[].type` |
| SRC-16 | $key context references bound to declared parameter | `∀ ctx where name starts with "$": referenced param exists in parameters[]` |
| SRC-17 | $key context references must be String type | `∀ ctx where name starts with "$": referenced param.type == String` |

**Note on SRC-14:** Uniqueness is by id only; version is not considered. Two primitives with the same id but different versions are rejected.

### 4.2 Composition Rules

These rules are checked when a source is composed with an adapter.

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| SRC-10 | Required context keys exist in adapter | `source.requires.context.filter(required).keys ⊆ adapter.provides.context.keys` |
| SRC-11 | Required context types match adapter | `∀ k where required: source.requires.context[k].ty == adapter.provides.context[k].ty` |

**Note:** SRC-10 and SRC-11 are enforced by the same composition function as COMP-1 and COMP-2 (see adapter.md §5). The predicates are identical; SRC-10/SRC-11 are the source contract's declaration of these requirements.

### 4.3 Enforcement Mapping

| Rule ID | Phase | Error Variant | Test Name |
|---------|-------|---------------|-----------|
| SRC-1 | Registration | `SourceValidationError::InvalidId` | `src_1_invalid_id_rejected` |
| SRC-2 | Registration | `SourceValidationError::InvalidVersion` | `src_2_invalid_version_rejected` |
| SRC-3 | Registration | `SourceValidationError::WrongKind` | `src_3_kind_source_accepted` |
| SRC-4 | Registration | `SourceValidationError::InputsNotAllowed` | `src_4_source_has_inputs_rejected` |
| SRC-5 | Registration | `SourceValidationError::OutputsRequired` | `src_5_no_outputs_rejected` |
| SRC-6 | Registration | `SourceValidationError::DuplicateOutput` | `src_6_duplicate_output_rejected` |
| SRC-7 | Registration | (exhaustive match) | `src_7_output_types_valid` |
| SRC-8 | Registration | `SourceValidationError::StateNotAllowed` | `src_8_source_has_state_rejected` |
| SRC-9 | Registration | `SourceValidationError::SideEffectsNotAllowed` | `src_9_source_has_side_effects_rejected` |
| SRC-10 | Composition | `CompositionError::MissingContextKey` | `src_10_missing_context_key_rejected` |
| SRC-11 | Composition | `CompositionError::ContextTypeMismatch` | `src_11_context_type_mismatch_rejected` |
| SRC-12 | Registration | `SourceValidationError::NonDeterministicExecution` | `src_12_non_deterministic_execution_rejected` |
| SRC-13 | Registration | `SourceValidationError::InvalidCadence` | (structurally enforced) |
| SRC-14 | Registration | `SourceValidationError::DuplicateId` | `src_14_duplicate_id_rejected` |
| SRC-15 | Registration | `SourceValidationError::InvalidParameterType` | `src_15_invalid_parameter_type_default_rejected` |
| SRC-16 | Registration | `SourceValidationError::UnboundContextKeyReference` | `src_16_dollar_key_referencing_nonexistent_param_rejected` |
| SRC-17 | Registration | `SourceValidationError::ContextKeyReferenceNotString` | `src_17_dollar_key_referencing_non_string_param_rejected` |

**Note on SRC-13:** Currently untestable because `Cadence` enum only has `Continuous` variant. Enforcement code exists at `registry.rs:77-78`; test will be added when cadence variants expand.

**Registration enforcement location:** `crates/runtime/src/source/registry.rs`
**Registration test location:** `crates/runtime/src/source/tests.rs`
**Composition enforcement location:** `crates/adapter/src/composition.rs`
**Composition test location:** `crates/adapter/tests/composition_tests.rs`

---

## 5. Orchestrator Contract

The orchestrator guarantees:
- Source outputs are available before compute evaluation
- Values are correctly typed
- Data is aligned to the evaluation clock
- Source execution is deterministic per tick

The orchestrator does not:
- infer missing data
- backfill implicitly
- mutate source outputs

---

## 6. Prohibited Behavior

A source primitive may not:
- Accept inputs
- Emit events
- Perform computation
- Hold state
- Branch on execution mode
- Access external state directly (must use adapter via `requires.context`)
- Mutate external systems

Violation invalidates the primitive.

---

## 7. Canonical Source Examples

### Source with context requirements

```yaml
kind: source
id: context_number_source
version: 0.1.0

outputs:
  - name: value
    type: Number

parameters:
  - name: key
    type: string
    default: "x"

requires:
  context:
    - name: $key
      type: Number
      required: false

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
```

This source reads the context key named by parameter `key` (default `"x"`). Since
`required: false`, the source uses a default value when the resolved key is absent.

### Source with bool context requirements

```yaml
kind: source
id: context_bool_source
version: 0.1.0

outputs:
  - name: value
    type: Bool

parameters:
  - name: key
    type: string
    default: "x"

requires:
  context:
    - name: $key
      type: Bool
      required: false

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
```

This source reads the context key named by parameter `key` (default `"x"`). Since
`required: false`, missing keys and wrong-type values resolve to the source default
(`false`).

### Source without context requirements

```yaml
kind: source
id: number_source
version: 0.1.0

outputs:
  - name: value
    type: Number

parameters:
  - name: value
    type: number
    default: 0.0

requires:
  context: []

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
```

This source has no context requirements. It produces a constant number from its parameter.

---

### 7.1 Invalid Examples (one per rule)

Each example below violates exactly one SRC-* rule.

#### SRC-1 — Invalid ID format

```yaml
kind: source
id: Bad-Id
version: 0.1.0
inputs: []
outputs:
  - name: value
    type: Number
parameters: []
requires:
  context: []
execution:
  cadence: continuous
  deterministic: true
state:
  allowed: false
side_effects: false
```

#### SRC-2 — Invalid version (not semver)

```yaml
kind: source
id: number_source
version: one
inputs: []
outputs:
  - name: value
    type: Number
parameters: []
requires:
  context: []
execution:
  cadence: continuous
  deterministic: true
state:
  allowed: false
side_effects: false
```

#### SRC-3 — Wrong kind

```yaml
kind: compute
id: number_source
version: 0.1.0
inputs: []
outputs:
  - name: value
    type: Number
parameters: []
requires:
  context: []
execution:
  cadence: continuous
  deterministic: true
state:
  allowed: false
side_effects: false
```

#### SRC-4 — Inputs not allowed

```yaml
kind: source
id: number_source
version: 0.1.0
inputs:
  - name: input
    type: Number
    required: true
outputs:
  - name: value
    type: Number
parameters: []
requires:
  context: []
execution:
  cadence: continuous
  deterministic: true
state:
  allowed: false
side_effects: false
```

#### SRC-5 — No outputs

```yaml
kind: source
id: number_source
version: 0.1.0
inputs: []
outputs: []
parameters: []
requires:
  context: []
execution:
  cadence: continuous
  deterministic: true
state:
  allowed: false
side_effects: false
```

#### SRC-6 — Duplicate output names

```yaml
kind: source
id: number_source
version: 0.1.0
inputs: []
outputs:
  - name: value
    type: Number
  - name: value
    type: Number
parameters: []
requires:
  context: []
execution:
  cadence: continuous
  deterministic: true
state:
  allowed: false
side_effects: false
```

#### SRC-7 — Invalid output type

```yaml
kind: source
id: number_source
version: 0.1.0
inputs: []
outputs:
  - name: value
    type: Float
parameters: []
requires:
  context: []
execution:
  cadence: continuous
  deterministic: true
state:
  allowed: false
side_effects: false
```

#### SRC-8 — State not allowed

```yaml
kind: source
id: number_source
version: 0.1.0
inputs: []
outputs:
  - name: value
    type: Number
parameters: []
requires:
  context: []
execution:
  cadence: continuous
  deterministic: true
state:
  allowed: true
side_effects: false
```

#### SRC-9 — Side effects not allowed

```yaml
kind: source
id: number_source
version: 0.1.0
inputs: []
outputs:
  - name: value
    type: Number
parameters: []
requires:
  context: []
execution:
  cadence: continuous
  deterministic: true
state:
  allowed: false
side_effects: true
```

#### SRC-10 — Required context key missing in adapter (composition)

Source manifest (requires `$key`, defaulting to `x`):
```yaml
kind: source
id: context_number_source
version: 0.1.0
inputs: []
outputs:
  - name: value
    type: Number
parameters:
  - name: key
    type: string
    default: "x"
requires:
  context:
    - name: $key
      type: Number
      required: true
execution:
  cadence: continuous
  deterministic: true
state:
  allowed: false
side_effects: false
```

Adapter provides (missing `x`):
```yaml
context_keys: []
```

#### SRC-11 — Required context type mismatch (composition)

Source manifest (requires `$key: Number`, defaulting to `x`):
```yaml
kind: source
id: context_number_source
version: 0.1.0
inputs: []
outputs:
  - name: value
    type: Number
parameters:
  - name: key
    type: string
    default: "x"
requires:
  context:
    - name: $key
      type: Number
      required: true
execution:
  cadence: continuous
  deterministic: true
state:
  allowed: false
side_effects: false
```

Adapter provides (type mismatch for `x`):
```yaml
context_keys:
  - name: x
    type: String
    required: false
    writable: false
```

## 8. Composition Rule

Sources start the graph.

- Source → Compute (COMP-4: output type must match input type)
- Source → Trigger (via compute)
- Source → Action (via compute + trigger)

Sources may not consume anything downstream.

---

## 9. Scope

This document defines Source Primitive Manifest v1.

Out of scope:
- event-emitting sources
- multi-identifier fan-out
- streaming adapters
- stateful ingestion
- user-defined IO

Those belong to later versions.

---

## 10. Contract Stability

This contract is STABLE.

Breaking changes require a manifest version bump.

---

## Bottom Line

With Source v1, the ontology is complete:
- Source → origin
- Compute → truth
- Trigger → causality
- Action → agency

Everything else is composition.
