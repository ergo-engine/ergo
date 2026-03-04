---
Authority: STABLE
Version: v1
Last Updated: 2026-02-12
Last Amended: 2026-02-12
---

> **Amended 2026-02-02** by Codex (Implementation Assistant)
> Phase 3 (Compute Contract) completion: schema updates (cardinality, may_error, errors, resettable),
> rules table, enforcement mapping, composition rules, and examples.

# Compute Primitive Manifest — v1

A Compute Primitive is a pure, deterministic transform that maps
named, typed inputs to named, typed outputs on a declared cadence.

Computes:
- have no side effects
- do not access external I/O
- do not observe the execution mode
- are composable only via graph wiring

The primitive is atomic; composition happens at the graph level.

---

## 1. Definition

A Compute Primitive is a deterministic data transform:

```
inputs (typed) -> compute -> outputs (typed)
```

It answers one question:

*"Given these inputs, what are the outputs?"*

---

## 2. Required Manifest Fields

Every compute primitive must declare all of the following.

---

### 2.1 Identity

```yaml
id: string
version: string
kind: compute
```

Rules:
- `id` must start with a lowercase letter and contain only lowercase letters, digits, and underscores (`^[a-z][a-z0-9_]*$`)
- `version` must be valid semver
- `kind` must be literal `compute`

---

### 2.2 Inputs

```yaml
inputs:
  - name: string
    type: number | bool | series
    required: bool
    cardinality: single
```

Rules:
- At least one input is required
- Input names must be unique
- Input types must be `Number`, `Bool`, or `Series` (no `String`, no `Event`)
- Cardinality must be `single` (multiple is reserved)

---

### 2.3 Outputs

```yaml
outputs:
  - name: string
    type: number | bool | series | string
```

Rules:
- At least one output is required
- Output names must be unique
- Output types must be `Number`, `Bool`, `Series`, or `String`
- On success, all declared outputs must be produced
- Undeclared outputs are not permitted

---

### 2.4 Parameters (Presets Only)

```yaml
parameters:
  - name: string
    type: int | number | bool
    default: any
    required: bool
    bounds: optional
```

Rules:
- Parameter types are limited to `int | number | bool`
- Parameters are static presets (do not change during execution)
- Parameters must be serializable

---

### 2.5 Execution Semantics

```yaml
execution:
  cadence: continuous
  deterministic: true
  may_error: true
```

Rules:
- Cadence is `continuous` only (event cadence is unsatisfiable in v0)
- Determinism is required
- `may_error` indicates whether the compute may return a `ComputeError` (informational)

---

### 2.6 Error Semantics

```yaml
errors:
  allowed: bool
  types: [ErrorType]
  deterministic: true
```

Valid ErrorType values:
- `DivisionByZero`
- `NonFiniteResult`

Rules:
- If `errors.allowed == true`, then `errors.deterministic` must be true
- When execution succeeds, all declared outputs must be produced
- When execution fails, no outputs are produced
- Errors surface as `ExecError::ComputeFailed` at runtime

---

### 2.7 State

```yaml
state:
  allowed: bool
  resettable: bool
  description: optional
```

Rules:
- If `state.allowed == true`, then `state.resettable` must be true
- External or hidden state is forbidden

---

### 2.8 Side Effects

```yaml
side_effects: false
```

Rules:
- Compute primitives may not perform I/O
- Compute primitives may not access external state

---

## 3. Rules Definition

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| CMP-1 | ID format valid | `regex(id, /^[a-z][a-z0-9_]*$/)` |
| CMP-2 | Version valid semver | `semver.valid(version)` |
| CMP-3 | Kind is "compute" | `kind == "compute"` |
| CMP-4 | At least one input | `inputs.len >= 1` |
| CMP-5 | Input names unique | `unique(inputs[].name)` |
| CMP-6 | At least one output | `outputs.len >= 1` |
| CMP-7 | Output names unique | `unique(outputs[].name)` |
| CMP-8 | Side effects not allowed | `side_effects == false` |
| CMP-9 | State resettable if allowed | `state.allowed => state.resettable` |
| CMP-10 | Errors deterministic | `errors.allowed => errors.deterministic` |
| CMP-11 | All outputs produced on success | `compute() -> Ok(outputs)` must include every declared output |
| CMP-12 | No outputs produced on error | `compute() -> Err(_)` emits no outputs |
| CMP-13 | Input types valid | `inputs[].type ∈ {Number, Bool, Series}` |
| CMP-14 | Input cardinality single | `inputs[].cardinality == single` |
| CMP-15 | Parameter types valid | `parameters[].type ∈ {Int, Number, Bool}` |
| CMP-16 | Cadence is continuous | `execution.cadence == continuous` |
| CMP-17 | Execution deterministic | `execution.deterministic == true` |
| CMP-18 | ID unique in registry | `id ∉ ComputeRegistry.ids` |
| CMP-19 | Parameter default type matches declared type | `parameters[].default == None || typeof(parameters[].default) == parameters[].type` |
| CMP-20 | Output types valid | `all(outputs[].type ∈ ValueType)` |

**Note on CMP-18:** Uniqueness is by id only; version is not considered. Two primitives with the same id but different versions are rejected.

---

## 4. Enforcement Mapping

| Rule ID | Phase | Error Variant | Test Name |
|---------|-------|---------------|-----------|
| CMP-1 | Registration | `ValidationError::InvalidId` | `cmp_1_invalid_id_rejected` |
| CMP-2 | Registration | `ValidationError::InvalidVersion` | `cmp_2_invalid_version_rejected` |
| CMP-3 | Registration | Type (PrimitiveKind::Compute only) | `cmp_3_kind_compute_accepted` |
| CMP-4 | Registration | `ValidationError::NoInputsDeclared` | `cmp_4_no_inputs_rejected` |
| CMP-5 | Registration | `ValidationError::DuplicateInput` | `cmp_5_duplicate_inputs_rejected` |
| CMP-6 | Registration | `ValidationError::NoOutputsDeclared` | `cmp_6_no_outputs_rejected` |
| CMP-7 | Registration | `ValidationError::DuplicateOutput` | `cmp_7_duplicate_outputs_rejected` |
| CMP-8 | Registration | `ValidationError::SideEffectsNotAllowed` | `cmp_8_side_effects_rejected` |
| CMP-9 | Registration | `ValidationError::StateNotResettable` | `cmp_9_state_not_resettable_rejected` |
| CMP-10 | Registration | `ValidationError::NonDeterministicErrors` | `cmp_10_non_deterministic_errors_rejected` |
| CMP-11 | Execution | `ExecError::MissingOutput` | `cmp_11_missing_output_fails` |
| CMP-12 | Execution | `ExecError::ComputeFailed` | `cmp_12_compute_error_fails` |
| CMP-13 | Registration | `ValidationError::InvalidInputType` | `cmp_13_invalid_input_type_rejected` |
| CMP-14 | Registration | `ValidationError::InvalidInputCardinality` | `cmp_14_invalid_input_cardinality_rejected` |
| CMP-15 | Registration | `ValidationError::UnsupportedParameterType` | `cmp_15_invalid_parameter_type_rejected` |
| CMP-16 | Registration | `ValidationError::InvalidCadence` | `cmp_16_invalid_cadence_rejected` |
| CMP-17 | Registration | `ValidationError::NonDeterministicExecution` | `cmp_17_non_deterministic_execution_rejected` |
| CMP-18 | Registration | `ValidationError::DuplicateId` | `cmp_18_duplicate_id_rejected` |
| CMP-19 | Registration | `ValidationError::InvalidParameterType` | `cmp_19_invalid_parameter_type_default_rejected` |
| CMP-20 | Registration | Type (`ValueType` enum) | `cmp_20_output_types_valid` |

**Enforcement location (registration):** `crates/runtime/src/compute/registry.rs`

**Enforcement location (execution):** `crates/runtime/src/runtime/execute.rs`

**CMP-12 note:** The compute API returns `Result<Outputs, ComputeError>`. An error has no outputs by
construction; the runtime surfaces this as `ExecError::ComputeFailed`.

**CMP-19 note:** Enforced in registration by validating that `parameters[].default` (when present)
matches `parameters[].type`.

**CMP-20 note:** Structurally enforced in registration via exhaustive `ValueType` matching in
`crates/runtime/src/compute/registry.rs`; invalid manifest strings (for example `type: event`)
are rejected during CLI parse before registry validation.

---

## 5. Composition Rules

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| COMP-4 | Source output type equals Compute input type | `source.output.type == compute.input.type` |
| COMP-5 | Input type equals upstream output type | `upstream.output.type == compute.input.type` |
| COMP-6 | Output type equals downstream input type | `compute.output.type == downstream.input.type` |

**Enforcement location:** `crates/runtime/src/runtime/validate.rs` (`ValidationError::TypeMismatch`)

---

## 6. Example Manifests

### 6.1 Minimal Valid Compute

```yaml
kind: compute
id: add_two
version: 1.0.0

inputs:
  - name: a
    type: number
    required: true
    cardinality: single
  - name: b
    type: number
    required: true
    cardinality: single

outputs:
  - name: result
    type: number

parameters: []

execution:
  cadence: continuous
  deterministic: true
  may_error: false

errors:
  allowed: false
  types: []
  deterministic: true

state:
  allowed: false
  resettable: false

side_effects: false
```

### 6.2 Compute With Declared Errors

```yaml
kind: compute
id: divide
version: 1.0.0

inputs:
  - name: a
    type: number
    required: true
    cardinality: single
  - name: b
    type: number
    required: true
    cardinality: single

outputs:
  - name: result
    type: number

parameters: []

execution:
  cadence: continuous
  deterministic: true
  may_error: true

errors:
  allowed: true
  types: [DivisionByZero, NonFiniteResult]
  deterministic: true

state:
  allowed: false
  resettable: false

side_effects: false
```
