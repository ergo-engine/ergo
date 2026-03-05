---
Authority: STABLE
Version: v1
Last Updated: 2026-02-02
Last Amended: 2026-02-02
---

> **Amended 2026-02-02** by Codex (Implementation Assistant)
> Phase 5 (Action Contract) completion: schema updates, rule table, enforcement mapping,
> composition rules, and examples.

# Action Primitive Manifest — v1

This is the authoritative contract.

---

## 1. Definition

An Action Primitive is a deterministic command that attempts to apply a
side effect to an external execution environment, gated by an event, and
emits a terminal outcome event.

Actions:

- are the only primitives allowed to cause side effects
- do not compute signals
- do not infer intent
- do not decide when to act (triggers do that)
- only decide what to attempt

An action answers one question:

*"Given this event, what command should be attempted?"*

---

## 2. Required Manifest Fields

Every action primitive must declare all of the following.

---

### 2.1 Identity

```yaml
id: string
version: string
kind: action
```

Rules:

- `id` must start with a lowercase letter and contain only lowercase letters, digits, and underscores (`^[a-z][a-z0-9_]*$`)
- `version` must be valid semver
- `kind` must be literal `action`

---

### 2.2 Inputs

```yaml
inputs:
  - name: string
    type: event | number | bool | string
    required: bool
    cardinality: single
```

Rules:

- At least one input must be an event
- Event inputs are **gating inputs** (causal `when`) and are wired from Trigger outputs
- Non-event inputs are **payload inputs** (command `what`) and may be wired from Source or Compute outputs
- Payload inputs are non-gating; action execution timing is determined only by trigger event inputs
- Inputs are explicit, named, and typed
- No implicit access to external state or context

---

### 2.3 Outputs

```yaml
outputs:
  - name: outcome
    type: event
```

Rules:

- Actions always emit exactly one outcome event
- Output name must be `outcome`
- Output type must be `event`

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
- Parameters do not change at runtime
- If `default` is present, its type must match the declared parameter `type`

---

### 2.5 Execution Semantics

```yaml
execution:
  deterministic: true
  retryable: false
```

Rules:

- Determinism is required
- Retry behavior must be explicit (actions are non-retryable in v0)

---

### 2.6 State

```yaml
state:
  allowed: false
```

Rules:

- Action primitives may not hold internal state

---

### 2.7 Side Effects

```yaml
side_effects: true
```

Rules:

- Action primitives are the only primitives where this is allowed
- Side effects are limited to declared external operations

---

### 2.8 Effects (Write Declarations)

```yaml
effects:
  writes:
    - name: string
      type: number | bool | string
      from_input: string   # Required scalar action input supplying the write value
```

Rules:

- `effects` block must exist (may contain empty `writes`)
- Write names must be unique
- Write types must be Number, Bool, or String
- `from_input` is required and must name a declared scalar input (ACT-22/ACT-23)

---

## 3. Outcome Event Semantics (Critical)

An outcome event is:

- discrete
- emitted exactly once per action attempt
- terminal (no persistence)

Rules:

- Outcome events do not carry payloads in v0
- Outcome events are **non-wireable** in v0
- Outcome events may be consumed only by external sinks (logging/audit/replay)

---

## 4. Rules Definition

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| ACT-1 | ID format valid | `regex(id, /^[a-z][a-z0-9_]*$/)` |
| ACT-2 | Version valid semver | `semver.valid(version)` |
| ACT-3 | Kind is "action" | `kind == "action"` |
| ACT-4 | At least one event input | `any(inputs[].type == "event")` |
| ACT-5 | Input names unique | `unique(inputs[].name)` |
| ACT-6 | Input types valid | `all(inputs[].type ∈ ActionValueType)` |
| ACT-7 | Exactly one output | `outputs.len == 1` |
| ACT-8 | Output named "outcome" | `outputs[0].name == "outcome"` |
| ACT-9 | Output is event type | `outputs[0].type == "event"` |
| ACT-10 | State not allowed | `state.allowed == false` |
| ACT-11 | Side effects required | `side_effects == true` |
| ACT-12 | Gated by trigger | (validation phase, R.7) |
| ACT-13 | Effects block present | `effects is present` |
| ACT-14 | Write names unique | `unique(effects.writes[].name)` |
| ACT-15 | Write types valid | `all(effects.writes[].type ∈ {Number, Bool, String})` |
| ACT-16 | Retryable false | `execution.retryable == false` |
| ACT-17 | Execution deterministic | `execution.deterministic == true` |
| ACT-18 | ID unique in registry | `id ∉ ActionRegistry.ids` |
| ACT-19 | Parameter default type matches declared type | `parameters[].default == None \|\| typeof(parameters[].default) == parameters[].type` |
| ACT-20 | $key write references bound to declared parameter | `∀ write where name starts with "$": referenced param exists in parameters[]` |
| ACT-21 | $key write references must be String type | `∀ write where name starts with "$": referenced param.type == String` |
| ACT-22 | Write from_input references declared input | `∀ write: from_input ∈ inputs[].name` |
| ACT-23 | Write from_input type compatible with write type | `∀ write: inputs[from_input].type is scalar AND matches write.value_type` |

**Note on ACT-18:** Uniqueness is by id only; version is not considered. Two primitives with the same id but different versions are rejected.

**ActionValueType:** Event | Number | Bool | String (no Series)

---

## 5. Enforcement Mapping

| Rule ID | Phase | Error Variant | Test Name |
|---------|-------|---------------|-----------|
| ACT-1 | Registration | `ActionValidationError::InvalidId` | `act_1_invalid_id_rejected` |
| ACT-2 | Registration | `ActionValidationError::InvalidVersion` | `act_2_invalid_version_rejected` |
| ACT-3 | Registration | Type (ActionKind::Action only) | `act_3_kind_action_accepted` |
| ACT-4 | Registration | `ActionValidationError::EventInputRequired` | `act_4_no_event_input_rejected` |
| ACT-5 | Registration | `ActionValidationError::DuplicateInput` | `act_5_duplicate_input_rejected` |
| ACT-6 | Registration | Type (ActionValueType enum) | `act_6_input_types_valid` |
| ACT-7 | Registration | `ActionValidationError::UndeclaredOutput` | `act_7_wrong_output_count_rejected` |
| ACT-8 | Registration | `ActionValidationError::OutputNotOutcome` | `act_8_output_not_outcome_rejected` |
| ACT-9 | Registration | `ActionValidationError::InvalidOutputType` | `act_9_output_not_event_rejected` |
| ACT-10 | Registration | `ActionValidationError::StateNotAllowed` | `act_10_action_has_state_rejected` |
| ACT-11 | Registration | `ActionValidationError::SideEffectsRequired` | `act_11_action_no_side_effects_rejected` |
| ACT-12 | Validation | `ValidationError::ActionNotGated` | `act_12_action_not_gated_rejected` |
| ACT-13 | Registration | Type (effects field required) | `act_3_kind_action_accepted` |
| ACT-14 | Registration | `ActionValidationError::DuplicateWriteName` | `act_14_duplicate_write_name_rejected` |
| ACT-15 | Registration | `ActionValidationError::InvalidWriteType` | `act_15_invalid_write_type_rejected` |
| ACT-16 | Registration | `ActionValidationError::RetryNotAllowed` | `act_16_retryable_not_allowed_rejected` |
| ACT-17 | Registration | `ActionValidationError::NonDeterministicExecution` | `act_17_non_deterministic_execution_rejected` |
| ACT-18 | Registration | `ActionValidationError::DuplicateId` | `act_18_duplicate_id_rejected` |
| ACT-19 | Registration | `ActionValidationError::InvalidParameterType` | `act_19_invalid_parameter_type_default_rejected` |
| ACT-20 | Registration | `ActionValidationError::UnboundWriteKeyReference` | `act_20_dollar_key_write_referencing_nonexistent_param_rejected` |
| ACT-21 | Registration | `ActionValidationError::WriteKeyReferenceNotString` | `act_21_dollar_key_write_referencing_non_string_param_rejected` |
| ACT-22 | Registration | `ActionValidationError::WriteFromInputNotFound` | `act_22_from_input_not_found_rejected` |
| ACT-23 | Registration | `ActionValidationError::WriteFromInputTypeMismatch` | `act_23_from_input_event_type_rejected` |

**Registration enforcement location:** `crates/kernel/runtime/src/action/registry.rs`

**Validation enforcement location:** `crates/kernel/runtime/src/runtime/validate.rs`

**ACT-12 mapping note:** Doctrine rule `ACT-12` is enforced by validation surface `V.5` (`ValidationError::ActionNotGated`).

---

## 6. Composition Rules

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| COMP-9 | Action inputs follow gate/payload split | `∀ action input i: (i.type == event => upstream(i).kind == "trigger") ∧ (i.type != event => upstream(i).kind ∈ {"source","compute"})` |
| COMP-10 | Action output not wireable | `downstream.len == 0` |
| COMP-11 | Action writes target provided keys | `effects.writes.names ⊆ adapter.context_keys.names` |
| COMP-12 | Action writes only writable keys | `∀n ∈ writes: adapter.key[n].writable == true` |
| COMP-13 | Action write types match | `∀n ∈ writes: action.type[n] == adapter.key[n].type` |
| COMP-14 | If action writes, adapter accepts set_context | `writes.len > 0 => accepts.effects contains "set_context"` |
| COMP-15 | Writes captured (planned) | `writes.len > 0 => capture includes effect + keys` (deferred: REP-SCOPE) |

**Composition enforcement:**

- COMP-10 is enforced by wiring matrix validation (`runtime/validate.rs`).
- COMP-9 refines Action input legality by destination input type (event gate vs scalar payload). Runtime validation implements this with destination-input-type-aware checks in `runtime/validate.rs` in addition to the coarse wiring matrix path.
- COMP-11 through COMP-14 are enforced in `crates/kernel/adapter/src/composition.rs` when binding adapter ↔ graph.
- COMP-15 is deferred until REP-SCOPE expands.

---

## 7. Example Manifests

### 7.1 Minimal Valid Action (No Writes)

```yaml
kind: action
id: ack_action
version: 1.0.0

inputs:
  - name: event
    type: event
    required: true
    cardinality: single

outputs:
  - name: outcome
    type: event

parameters:
  - name: accept
    type: bool
    default: true
    required: false

execution:
  deterministic: true
  retryable: false

state:
  allowed: false

side_effects: true

effects:
  writes: []
```

### 7.2 Action With Writes

```yaml
kind: action
id: set_price
version: 1.0.0

inputs:
  - name: event
    type: event
    required: true
    cardinality: single
  - name: price
    type: number
    required: true
    cardinality: single

outputs:
  - name: outcome
    type: event

parameters: []

execution:
  deterministic: true
  retryable: false

state:
  allowed: false

side_effects: true

effects:
  writes:
    - name: price
      type: number
      from_input: price
```
