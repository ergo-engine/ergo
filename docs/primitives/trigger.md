---
Authority: STABLE
Version: v1
Last Updated: 2026-02-02
Last Amended: 2026-02-02
---

> **Amended 2026-02-02** by Codex (Implementation Assistant)
> Phase 4 (Trigger Contract) completion: schema updates, rule table, enforcement mapping,
> composition rules, and examples.

# Trigger Primitive Manifest — v1

A Trigger Primitive is a deterministic event extractor that converts
typed inputs into discrete events.

Triggers:

- do not perform actions
- do not manage external state
- do not execute side effects
- exist solely to detect when something happens

A trigger answers one question:

*"Did an event occur at this evaluation point?"*

---

## 1. Definition

A Trigger Primitive is a deterministic event extractor:

```
inputs (typed) -> trigger -> event
```

It converts boolean/series/value inputs (or upstream events) into a single
emitted/not-emitted event.

---

## 2. Required Manifest Fields

Every trigger primitive must declare all of the following.

---

### 2.1 Identity

```yaml
id: string
version: string
kind: trigger
```

Rules:

- `id` must start with a lowercase letter and contain only lowercase letters, digits, and underscores (`^[a-z][a-z0-9_]*$`)
- `version` must be valid semver
- `kind` must be literal `trigger`

---

### 2.2 Inputs

```yaml
inputs:
  - name: string
    type: number | bool | series | event
    required: bool
    cardinality: single
```

Rules:

- At least one input is required
- Input names must be unique
- Input types must be `Number`, `Bool`, `Series`, or `Event` (no `String`)
- Cardinality must be `single` (multiple is reserved)

---

### 2.3 Outputs

```yaml
outputs:
  - name: string
    type: event
```

Rules:

- Exactly one output is required
- Output type must be `event`
- Output name is not enforced today

---

### 2.4 Parameters

```yaml
parameters:
  - name: string
    type: int | number | bool | string | enum
    default: any
    required: bool
    bounds: optional
```

Rules:

- Parameters are static presets
- Parameters must be serializable
- No runtime mutation allowed
- If `default` is present, its type must match the declared parameter `type`

---

### 2.5 Execution Semantics

```yaml
execution:
  cadence: continuous | event
  deterministic: true
```

Rules:

- Determinism is required
- `continuous` = evaluated every tick
- `event` = evaluated only when upstream event occurs

---

### 2.6 State (Prohibited)

```yaml
state:
  allowed: false
  description: optional
```

**Triggers are stateless.** The `state.allowed` field must be `false` for all trigger
implementations. The registry rejects any trigger manifest with `allowed: true`.

TRG-STATE-1 (invariants/INDEX.md) formalizes this requirement.

---

### 2.7 Side Effects

```yaml
side_effects: false
```

---

## 3. Rules Definition

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| TRG-1 | ID format valid | `regex(id, /^[a-z][a-z0-9_]*$/)` |
| TRG-2 | Version valid semver | `semver.valid(version)` |
| TRG-3 | Kind is "trigger" | `kind == "trigger"` |
| TRG-4 | At least one input | `inputs.len >= 1` |
| TRG-5 | Input names unique | `unique(inputs[].name)` |
| TRG-6 | Input types valid | `all(inputs[].type ∈ TriggerValueType)` |
| TRG-7 | Exactly one output | `outputs.len == 1` |
| TRG-8 | Output is event type | `outputs[0].type == "event"` |
| TRG-9 | State not allowed | `state.allowed == false` |
| TRG-10 | Side effects not allowed | `side_effects == false` |
| TRG-11 | Execution deterministic | `execution.deterministic == true` |
| TRG-12 | Input cardinality single | `inputs[].cardinality == single` |
| TRG-13 | ID unique in registry | `id ∉ TriggerRegistry.ids` |
| TRG-14 | Parameter default type matches declared type | `parameters[].default == None \|\| typeof(parameters[].default) == parameters[].type` |

**Note on TRG-13:** Uniqueness is by id only; version is not considered. Two primitives with the same id but different versions are rejected.

**TriggerValueType:** Number | Bool | Series | Event (no String)

---

## 4. Enforcement Mapping

| Rule ID | Phase | Error Variant | Test Name |
|---------|-------|---------------|-----------|
| TRG-1 | Registration | `TriggerValidationError::InvalidId` | `trg_1_invalid_id_rejected` |
| TRG-2 | Registration | `TriggerValidationError::InvalidVersion` | `trg_2_invalid_version_rejected` |
| TRG-3 | Registration | Type (TriggerKind::Trigger only) | `trg_3_kind_trigger_accepted` |
| TRG-4 | Registration | `TriggerValidationError::NoInputsDeclared` | `trg_4_no_inputs_rejected` |
| TRG-5 | Registration | `TriggerValidationError::DuplicateInput` | `trg_5_duplicate_input_rejected` |
| TRG-6 | Registration | Type (TriggerValueType enum) | `trg_6_input_types_valid` |
| TRG-7 | Registration | `TriggerValidationError::TriggerWrongOutputCount` | `trg_7_wrong_output_count_rejected` |
| TRG-8 | Registration | `TriggerValidationError::InvalidOutputType` | `trg_8_output_not_event_rejected` |
| TRG-9 | Registration | `TriggerValidationError::StatefulTriggerNotAllowed` | `trg_9_trigger_has_state_rejected` |
| TRG-10 | Registration | `TriggerValidationError::SideEffectsNotAllowed` | `trg_10_trigger_has_side_effects_rejected` |
| TRG-11 | Registration | `TriggerValidationError::NonDeterministicExecution` | `trg_11_non_deterministic_execution_rejected` |
| TRG-12 | Registration | `TriggerValidationError::InvalidInputCardinality` | `trg_12_invalid_input_cardinality_rejected` |
| TRG-13 | Registration | `TriggerValidationError::DuplicateId` | `trg_13_duplicate_id_rejected` |
| TRG-14 | Registration | `TriggerValidationError::InvalidParameterType` | `trg_14_invalid_parameter_type_default_rejected` |

**Enforcement location:** `crates/kernel/runtime/src/trigger/registry.rs`

**TRG-9 link:** TRG-STATE-1 in `invariants/INDEX.md`.

---

## 5. Composition Rules

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| COMP-7 | Trigger input from Compute or Trigger | `upstream.kind ∈ {"compute", "trigger"}` |
| COMP-8 | Trigger output to Action or Trigger | `downstream.kind ∈ {"action", "trigger"}` |

**Enforcement location:** `crates/kernel/runtime/src/runtime/validate.rs` (wiring matrix validation)

---

## 6. Example Manifests

### 6.1 Minimal Valid Trigger

```yaml
kind: trigger
id: emit_if_true
version: 1.0.0

inputs:
  - name: input
    type: bool
    required: true
    cardinality: single

outputs:
  - name: event
    type: event

parameters: []

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
```

### 6.2 Event + Bool Gated Trigger

```yaml
kind: trigger
id: emit_if_event_and_true
version: 0.1.0

inputs:
  - name: event
    type: event
    required: true
    cardinality: single
  - name: condition
    type: bool
    required: true
    cardinality: single

outputs:
  - name: event
    type: event

parameters: []

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
```
