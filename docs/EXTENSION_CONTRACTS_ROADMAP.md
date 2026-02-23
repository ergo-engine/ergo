# Extension Contracts Roadmap

**Status:** v1.0.0-alpha.9 — Breaking changes permitted.

**Goal:** Define extension contracts completely enough that compliance is mechanically verifiable.

**Success Criteria:**
1. A developer reads the contract and knows exactly what to build
2. The system validates compliance without human judgment
3. Non-compliance produces a specific, actionable error
4. Compliant pieces compose with any other compliant pieces

**Non-Goals:**
- Trust tiers
- Human review processes
- Sandboxing
- Attestation

The contract is the only gate.

---

## Current vs Planned

This roadmap distinguishes between:

| Label | Meaning |
|-------|---------|
| **CURRENT** | Matches existing code behavior |
| **PLANNED** | v1 work item, not yet implemented |

Breaking changes are expected during v1 alpha.

---

## Locked Decisions

These decisions are final and affect all phases.

### ValueType (Multiple Enums by Context)

The codebase has multiple type enums for different contexts:

**Extension Manifest ValueType** (`common::ValueType`):
```rust
enum ValueType {
    Number,   // f64
    Bool,     // bool
    String,   // String
    Series,   // Vec<f64>
}
```
Used for: Source outputs, Compute inputs/outputs, adapter context keys.

**Trigger Input/Output Types** (`TriggerValueType`):
```rust
enum TriggerValueType {
    Number,
    Series,
    Bool,
    Event,  // Triggers can receive and emit events
}
```

**Action Input Types** (`ActionValueType`):
```rust
enum ActionValueType {
    Event,   // Required - actions are event-gated
    Number,
    Bool,
    String,  // No Series
}
```

**Cluster Wiring Types** (`cluster::ValueType`):
```rust
enum ValueType {
    Number,
    Series,
    Bool,
    Event,   // For wiring validation
    String,
}
```

**Key distinction:** `Event` is NOT in extension manifest ValueType (source/compute). Triggers and Actions have their own type enums that include Event.

### JSON Schema

- Pin to **Draft 2020-12**
- `additionalProperties: false` by default
- No `$ref` to external schemas (self-contained)
- No `oneOf`/`anyOf` (keeps validation simple)

### Event Kinds

**CURRENT (v1 alpha):**
- `ExternalEventKind` remains the transport enum (`Pump`, `DataAvailable`, `Command`) for supervisor scheduling mechanics.
- Adapter semantic event kinds are open-world strings declared in adapter manifests.
- Semantic event consistency is enforced in adapter layer:
  - ADP-18 (required semantic payload fields must map to context keys with compatible types)
  - ADP-19 (materialized field shapes must map to supported runtime value types)
  - Runtime binder validates `(semantic_kind, payload)` before emitting `ExternalEvent`.

**PLANNED (later v1):**
- Supervisor policy/routing by semantic event kind string.

### Schema vs Rules

- **Schema** = shape and types (what fields exist, what types they have)
- **Rules** = cross-field constraints (uniqueness, references, compatibility)

### Enforcement Anchors

No file:line references (they drift). Use:
- Function path: `adapter::registry::register`
- Error variant: `InvalidAdapter::DuplicateContextKey`
- Test name: `adp_5_duplicate_context_key_rejected`

### Source of Truth

Authoritative rules live in STABLE docs. The code registry mirrors those rules for enforcement
and CLI output. Each rule in code must point to its doc anchor.

If registry and docs disagree, docs win and code must be corrected. Generation is a sync tool,
not a source of authority.

### Error Information Requirements (Contractual)

Every validation error must provide certain information to satisfy Success Criterion 3 ("Non-compliance produces a specific, actionable error").

**MUST provide:**

| Field | Type | Purpose | Example |
|-------|------|---------|----------|
| `rule_id` | `&'static str` | Stable identifier for the violated rule | `"ADP-5"` |
| `phase` | `Phase` | When the violation is detected | `Phase::Registration` |
| `doc_anchor` | `&'static str` | Link to authoritative rule definition | `"STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-5"` |
| `summary` | `Cow<'static, str>` | Human-readable description | `"Duplicate context key name"` |

**SHOULD provide (where meaningful):**

| Field | Type | Purpose | When omittable |
|-------|------|---------|----------------|
| `path` | `Option<Cow<'static, str>>` | JSON pointer to violation location | Composition/Execution errors without manifest locus |
| `fix` | `Option<Cow<'static, str>>` | Actionable resolution guidance | When multiple valid fixes exist |

**NOT contractual (implementation freedom):**
- Internal representation (typed enums vs unified struct)
- Whether errors accumulate or early-exit
- CLI output formatting

The contract specifies the *information content* of an error. The internal carrier type is free as long as the required information can be obtained and rendered.

**Trait definition:**

```rust
pub trait ErrorInfo {
    fn rule_id(&self) -> &'static str;
    fn phase(&self) -> Phase;
    fn doc_anchor(&self) -> &'static str;
    fn summary(&self) -> Cow<'static, str>;
    fn path(&self) -> Option<Cow<'static, str>>;
    fn fix(&self) -> Option<Cow<'static, str>>;
}
```

**Phase scoping:**
- **Phase 1:** `InvalidAdapter` implements `ErrorInfo` fully.
- **Phases 2-6:** New error enums implement `ErrorInfo` as created.
- **Phase 7:** Retrofit all existing validation error enums + CLI presentation layer.

### RuleViolation (Phase 7 Presentation Format)

Phase 7 introduces a unified presentation struct for CLI rendering:

```rust
pub struct RuleViolation {
    pub rule_id: &'static str,
    pub phase: Phase,
    pub doc_anchor: &'static str,
    pub summary: Cow<'static, str>,
    pub path: Option<Cow<'static, str>>,
    pub fix: Option<Cow<'static, str>>,
}

pub enum Phase {
    Registration,   // Manifest-only validation
    Composition,    // Cross-artifact validation
    Execution,      // Runtime invariants
}

impl<E: ErrorInfo> From<E> for RuleViolation {
    fn from(e: E) -> Self {
        RuleViolation {
            rule_id: e.rule_id(),
            phase: e.phase(),
            doc_anchor: e.doc_anchor(),
            summary: e.summary(),
            path: e.path(),
            fix: e.fix(),
        }
    }
}
```

**This is a presentation format, not an internal representation.**

Validators continue to use typed error enums internally. The CLI maps these to `RuleViolation` for uniform rendering via the `ErrorInfo` trait. This preserves:
- Compile-time exhaustiveness checking on error handling
- Type-safe error construction
- Existing test assertions

**Accumulation:** Phase 7 may optionally implement error accumulation (collect all violations before returning). This is a UX enhancement, not a contract requirement. Early-exit semantics remain acceptable.

### Provides/Requires Model

Composition uses explicit provides/requires:

**Adapter provides:**
```yaml
provides:
  context:
    - name: price
      type: Number
      required: true
      writable: false
    - name: volume
      type: Number
      required: false
      writable: false
  events:
    - Pump
    - Signal
```

**Source requires:**
```yaml
requires:
  context:
    - name: price
      type: Number
      required: true
```

**Composition predicate:**
```
requires.context.filter(required).keys ⊆ provides.context.keys
∀ r ∈ requires.context where required: r.type == provides.context[r.name].type
```

**Note:** In code, the `type` field maps to `ContextKeySpec.ty` and `ContextRequirement.ty`.

### Provenance Rule (State Threading)

ExecutionContext may include environment state created or modified by prior actions **if obtained via adapter environment reads at episode start**. Episode outcomes must never be forwarded via supervisor/runtime context injection.

This pins SUPERVISOR.md §2.2 interpretation:
- **Allowed:** Adapter reads from external store → populates context
- **Forbidden:** Supervisor forwards episode results → injects into context

**Status:** Amended in FROZEN/SUPERVISOR.md v0 (2026-01-11).

### Capture Selectors (Formal Enumeration)

Valid capture field selectors (current, REP-SCOPE):
- `event.<kind>` for each `event_kinds[].name`
- `meta.adapter_id`, `meta.adapter_version`, `meta.timestamp`

Planned extension (requires REP-SCOPE update):
- `context.<key>` for each `context_keys[].name`
- `effect.<name>` for each `accepts.effects[].name`

### Workstream References

- **B.2 (Compute error semantics):** compute primitives return `Result<HashMap<String, Value>, ComputeError>`.
  `ComputeError` includes `DivisionByZero` and `NonFiniteResult`; the runtime maps these to
  `ExecError::ComputeFailed`. NUM-FINITE-1 rejects non-finite outputs after source/compute.
  This roadmap uses "B.2" to refer to this error-semantics workstream.

---

## Current State

| Contract | Schema | Rules | Enforcement | Composition | Overall |
|----------|--------|-------|-------------|-------------|---------|
| Source | 100% | 100% | 100% | 100% | ✓ Done |
| Compute | 100% | 100% | 100% | 100% | ✓ Done |
| Trigger | 100% | 100% | 100% | 100% | ✓ Done |
| Action | 100% | 100% | 100% | 100% | ✓ Done |
| Adapter | 100% | 100% | 100% | 100% | ✓ Done |
| Cluster | 90% | 85% | 80% | 80% | ~85% |

---

## Target State

Each contract contains four complete sections:

| Section | Purpose | Completeness Test |
|---------|---------|-------------------|
| **Schema** | Every field, type, constraints | Developer can write valid manifest without looking at examples |
| **Rules** | Valid/invalid combinations | Each rule is a boolean predicate, machine-checkable |
| **Enforcement** | Where checked, what error | Every rule maps to code location + error type + phase |
| **Composition** | How pieces connect | Developer knows what this piece requires and provides |

---

## Phase 1: Adapter Contract

**Why first:** Adapter is the foundation. Source depends on it. Currently at ~20%. Biggest gap, highest leverage.

### 1.1 Schema Definition

```yaml
kind: adapter                 # MUST be literal "adapter"
id: string                    # ^[a-z][a-z0-9_]*$
version: semver               # Adapter version
runtime_compatibility: semver # Minimum runtime version supported

context_keys:                 # What the adapter populates in ExecutionContext
  - name: string              # Key name (e.g., "price", "timestamp")
    type: ValueType           # Number | Bool | String | Series
    required: bool            # Always present vs optional
    writable: bool            # If true, actions may write via effects
    description: string       # Optional metadata; human-readable purpose

event_kinds:                  # What ExternalEvent types this adapter emits
  - name: string              # Event kind identifier (open world)
    payload_schema: JsonSchema # Draft 2020-12, self-contained

accepts:                      # Optional; what effects this adapter handles
  effects:
    - name: string            # Effect kind identifier (e.g., "set_context")
      payload_schema: JsonSchema # Draft 2020-12, self-contained
    
capture:
  format_version: string      # Capture bundle format version
  fields:                     # What fields are captured for replay decisions (event/meta only per REP-SCOPE)
    - string                  # MUST be in CaptureFieldSet (see ADP-10)
```

**Phase 1.1 schema clarification:**
- `accepts` is optional (adapters may accept zero effects).
- `context_keys[].description` is optional metadata.

**CaptureFieldSet(adapter) (current, REP-SCOPE):**
- `event.<event_kind_name>` for each `event_kinds[].name`
- `meta.adapter_id`, `meta.adapter_version`, `meta.timestamp`

**Planned extension (requires REP-SCOPE update):**
- `context.<key_name>` for each `context_keys[].name`
- `effect.<effect_name>` for each `accepts.effects[].name`

**Note (planned, Phase 8):** Adapters do NOT need to pre-seed initial values for writable keys. The `ctx_get_or_default` Source implementation will handle missing keys with a default.

**Deliverables:**
- [x] Complete schema definition in `adapter_contract.md`
- [x] JSON Schema for manifest validation
- [x] Example valid manifest
- [x] Example invalid manifests (one per rule violation)

### 1.2 Rules Definition

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| ADP-1 | ID format valid | `regex(id, /^[a-z][a-z0-9_]*$/)` |
| ADP-2 | Version valid semver | `semver.valid(version)` |
| ADP-3 | Runtime compatibility satisfied | `runtime.version >= runtime_compatibility` |
| ADP-4 | Provides something | `context_keys.len > 0 OR event_kinds.len > 0` |
| ADP-5 | Context key names unique | `unique(context_keys[].name)` |
| ADP-6 | Context key types valid | `all(context_keys[].type ∈ ValueType)` |
| ADP-7 | Event kind names unique | `unique(event_kinds[].name)` |
| ADP-8 | Event schemas valid JSON Schema | `json_schema.validate(payload_schema, draft: 2020-12)` |
| ADP-9 | Capture format version present | `capture.format_version != ""` |
| ADP-10 | Capture fields referentially valid | `all(fields[] ∈ CaptureFieldSet(adapter))` |
| ADP-11 | Writable flag must be present | `all(context_keys[].writable is present)` |
| ADP-12 | Effect names unique | `unique(accepts.effects[].name)` |
| ADP-13 | Effect schemas valid | `all(accepts.effects[].payload_schema validates as JsonSchema Draft2020-12)` |
| ADP-14 | Writable implies set_context accepted | `any(context_keys[].writable == true) => accepts.effects contains "set_context"` |
| ADP-15 | Writable keys must be capturable (planned; REP-SCOPE update required) | `∀k where writable == true: "context." + k.name ∈ capture.fields` |
| ADP-16 | Write effect must be capturable (planned; REP-SCOPE update required) | `any(writable == true) => "effect.set_context" ∈ capture.fields` |
| ADP-17 | Writable keys cannot be required | `∀k where writable == true: k.required == false` |

**Note on ADP-17:** Writable keys may not exist initially (no prior write). Setting `required: true` on a writable key would cause validation failures on first episode. Sources will handle missing keys via defaults once `ctx_get_or_default` lands (Phase 8).

**Note on ADP-15/ADP-16:** Planned extension. These rules depend on capture including context/effect, which conflicts with REP-SCOPE until it is explicitly expanded.

**Deliverables:**
- [x] Complete rule table in `adapter_contract.md`
- [x] Each rule has ID, description, predicate
- [x] Rules are machine-checkable (no human judgment)

### 1.3 Enforcement Mapping

| Rule ID | Phase | Error Variant | Test Name |
|---------|-------|---------------|-----------|
| ADP-1 | Registration | `InvalidAdapter::InvalidId` | `adp_1_invalid_id_rejected` |
| ADP-2 | Registration | `InvalidAdapter::InvalidVersion` | `adp_2_invalid_version_rejected` |
| ADP-3 | Registration | `InvalidAdapter::IncompatibleRuntime` | `adp_3_incompatible_runtime_rejected` |
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
| ADP-15 | Registration | `InvalidAdapter::WritableKeyNotCaptured` | `adp_15_writable_key_not_captured_rejected` | **Deferred: REP-SCOPE** |
| ADP-16 | Registration | `InvalidAdapter::SetContextNotCaptured` | `adp_16_set_context_not_captured_rejected` | **Deferred: REP-SCOPE** |
| ADP-17 | Registration | `InvalidAdapter::WritableKeyRequired` | `adp_17_writable_key_required_rejected` |

**Error structure (typed enum with ErrorInfo):**

```rust
/// Typed error enum with one variant per rule.
/// Implements ErrorInfo for contractual information requirements.
/// All list-locus variants include index for precise path() generation.
pub enum InvalidAdapter {
    InvalidId { id: String },
    InvalidVersion { version: String },
    IncompatibleRuntime { required: String, actual: String },
    ProvidesNothing,
    DuplicateContextKey { name: String, first_index: usize, second_index: usize },
    InvalidContextKeyType { name: String, got: String, index: usize },
    DuplicateEventKind { name: String, index: usize },
    InvalidPayloadSchema { event: String, error: String, index: usize },
    NoCaptureFormat,
    InvalidCaptureField { field: String, index: usize },
    MissingWritableFlag { key: String, index: usize },
    DuplicateEffectName { name: String, index: usize },
    InvalidEffectSchema { effect: String, error: String, index: usize },
    WritableWithoutSetContext { keys: Vec<String> },
    WritableKeyNotCaptured { key: String, index: usize },  // Deferred: REP-SCOPE
    SetContextNotCaptured,                                  // Deferred: REP-SCOPE
    WritableKeyRequired { key: String, index: usize },
}

impl ErrorInfo for InvalidAdapter {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::InvalidId { .. } => "ADP-1",
            Self::InvalidVersion { .. } => "ADP-2",
            Self::IncompatibleRuntime { .. } => "ADP-3",
            Self::ProvidesNothing => "ADP-4",
            Self::DuplicateContextKey { .. } => "ADP-5",
            Self::InvalidContextKeyType { .. } => "ADP-6",
            Self::DuplicateEventKind { .. } => "ADP-7",
            Self::InvalidPayloadSchema { .. } => "ADP-8",
            Self::NoCaptureFormat => "ADP-9",
            Self::InvalidCaptureField { .. } => "ADP-10",
            Self::MissingWritableFlag { .. } => "ADP-11",
            Self::DuplicateEffectName { .. } => "ADP-12",
            Self::InvalidEffectSchema { .. } => "ADP-13",
            Self::WritableWithoutSetContext { .. } => "ADP-14",
            Self::WritableKeyNotCaptured { .. } => "ADP-15",
            Self::SetContextNotCaptured => "ADP-16",
            Self::WritableKeyRequired { .. } => "ADP-17",
        }
    }
    
    fn phase(&self) -> Phase {
        Phase::Registration  // All ADP-* are registration-phase
    }
    
    fn doc_anchor(&self) -> &'static str {
        match self {
            Self::InvalidId { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-1",
            Self::InvalidVersion { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-2",
            Self::IncompatibleRuntime { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-3",
            Self::ProvidesNothing => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-4",
            Self::DuplicateContextKey { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-5",
            Self::InvalidContextKeyType { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-6",
            Self::DuplicateEventKind { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-7",
            Self::InvalidPayloadSchema { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-8",
            Self::NoCaptureFormat => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-9",
            Self::InvalidCaptureField { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-10",
            Self::MissingWritableFlag { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-11",
            Self::DuplicateEffectName { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-12",
            Self::InvalidEffectSchema { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-13",
            Self::WritableWithoutSetContext { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-14",
            Self::WritableKeyNotCaptured { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-15",
            Self::SetContextNotCaptured => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-16",
            Self::WritableKeyRequired { .. } => "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-17",
        }
    }
    
    fn summary(&self) -> Cow<'static, str> {
        match self {
            Self::InvalidId { id } => format!("Invalid adapter ID: '{}'", id).into(),
            Self::InvalidVersion { version } => format!("Invalid version: '{}'", version).into(),
            Self::IncompatibleRuntime { required, actual } => 
                format!("Runtime {} < required {}", actual, required).into(),
            Self::ProvidesNothing => "Adapter provides no context keys or events".into(),
            Self::DuplicateContextKey { name, .. } => 
                format!("Duplicate context key: '{}'", name).into(),
            // ... etc for all variants
        }
    }
    
    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvalidId { .. } => Some("$.id".into()),
            Self::InvalidVersion { .. } => Some("$.version".into()),
            Self::IncompatibleRuntime { .. } => Some("$.runtime_compatibility".into()),
            Self::ProvidesNothing => None,  // No single locus; spans two arrays
            Self::DuplicateContextKey { second_index, .. } => 
                Some(format!("$.context_keys[{}].name", second_index).into()),
            Self::InvalidContextKeyType { index, .. } => 
                Some(format!("$.context_keys[{}].type", index).into()),
            Self::DuplicateEventKind { index, .. } => 
                Some(format!("$.event_kinds[{}].name", index).into()),
            Self::InvalidPayloadSchema { index, .. } => 
                Some(format!("$.event_kinds[{}].payload_schema", index).into()),
            Self::NoCaptureFormat => Some("$.capture.format_version".into()),
            Self::InvalidCaptureField { index, .. } => 
                Some(format!("$.capture.fields[{}]", index).into()),
            Self::MissingWritableFlag { index, .. } => 
                Some(format!("$.context_keys[{}].writable", index).into()),
            Self::DuplicateEffectName { index, .. } => 
                Some(format!("$.accepts.effects[{}].name", index).into()),
            Self::InvalidEffectSchema { index, .. } => 
                Some(format!("$.accepts.effects[{}].payload_schema", index).into()),
            Self::WritableWithoutSetContext { .. } => Some("$.accepts.effects".into()),  // Cross-field
            Self::WritableKeyNotCaptured { index, .. } => 
                Some(format!("$.context_keys[{}]", index).into()),
            Self::SetContextNotCaptured => Some("$.capture.fields".into()),
            Self::WritableKeyRequired { index, .. } => 
                Some(format!("$.context_keys[{}]", index).into()),
        }
    }
    
    fn fix(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvalidId { .. } => 
                Some("ID must start with lowercase letter, contain only lowercase letters, digits, and underscores (no hyphens)".into()),
            Self::InvalidVersion { .. } => 
                Some("Version must be valid semver (e.g., '1.0.0')".into()),
            Self::IncompatibleRuntime { required, .. } => 
                Some(format!("Upgrade runtime to {} or higher", required).into()),
            Self::ProvidesNothing => 
                Some("Add at least one context_key or event_kind".into()),
            Self::DuplicateContextKey { name, .. } => 
                Some(format!("Rename '{}' to a unique value", name).into()),
            Self::InvalidContextKeyType { got, .. } => 
                Some(format!("Type '{}' is not valid; use Number, Bool, String, or Series", got).into()),
            Self::DuplicateEventKind { name, .. } => 
                Some(format!("Rename event kind '{}' to a unique value", name).into()),
            Self::InvalidPayloadSchema { .. } => 
                Some("Provide a valid JSON Schema (Draft 2020-12)".into()),
            Self::NoCaptureFormat => 
                Some("Set capture.format_version to a non-empty string".into()),
            Self::InvalidCaptureField { field, .. } => 
                Some(format!("'{}' is not in CaptureFieldSet; use event.<kind> or meta.<field>", field).into()),
            Self::MissingWritableFlag { key, .. } => 
                Some(format!("Add 'writable: true' or 'writable: false' to context key '{}'", key).into()),
            Self::DuplicateEffectName { name, .. } => 
                Some(format!("Rename effect '{}' to a unique value", name).into()),
            Self::InvalidEffectSchema { .. } => 
                Some("Provide a valid JSON Schema (Draft 2020-12)".into()),
            Self::WritableWithoutSetContext { .. } => 
                Some("Add 'set_context' to accepts.effects when using writable keys".into()),
            Self::WritableKeyNotCaptured { key, .. } => 
                Some(format!("Add 'context.{}' to capture.fields", key).into()),
            Self::SetContextNotCaptured => 
                Some("Add 'effect.set_context' to capture.fields".into()),
            Self::WritableKeyRequired { key, .. } => 
                Some(format!("Set 'required: false' on writable key '{}'", key).into()),
        }
    }
}
```

Phase 7 maps this to `RuleViolation` for CLI rendering. Internal code continues to use typed variants.

**Implementation notes (critical for testability):**

1. **ADP-6 dead-letter prevention:** `ContextKeySpec.ty` must be parsed as `String` (not `ValueType`) in the manifest struct. If parsed as `ValueType`, serde fails on invalid types before `validate_adapter()` runs, making ADP-6 untestable. Validation converts String → ValueType and emits `InvalidContextKeyType` on failure.

2. **ADP-9 semantics:** `NoCaptureFormat` means `capture.format_version == ""` (empty string), not missing. `capture.format_version` is a serde-required `String`, so it cannot be absent.

3. **ADP-10 semantics:** `InvalidCaptureField` means the field value is not in `CaptureFieldSet(adapter)` — i.e., referential validity against the set of legal capture selectors (`event.<kind>`, `meta.*`). It is NOT identifier format validation.

4. **ADP-11 semantics:** `MissingWritableFlag` means the `writable` field is absent from a context key definition. This requires `writable: Option<bool>` in the manifest struct. If `writable: bool`, serde fails before validation, making ADP-11 untestable.

5. **doc_anchor format:** All `doc_anchor()` values must use exact format: `STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-N` (not ad-hoc fragments like `#adp-5-duplicate-context-key`).

**Deliverables:**
- [x] `InvalidAdapter` error enum in code
- [x] `InvalidAdapter` implements `ErrorInfo` trait fully
- [x] Error messages reference doc anchor
- [x] One test per rule

### 1.4 Composition Rules

**Adapter provides:**
```rust
struct AdapterProvides {
    context: HashMap<String, ContextKeySpec>,
    events: HashSet<String>,
    effects: HashSet<String>,
}

struct ContextKeySpec {
    ty: ValueType,          // maps from context_keys[].type
    required: bool,
    writable: bool,
}
```

**Composition with Source:**

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| COMP-1 | Source context requirements satisfied | `source.requires.context.filter(required).keys ⊆ adapter.provides.context.keys` |
| COMP-2 | Source context types match | `∀ k where required: source.requires.context[k].ty == adapter.provides.context[k].ty` |

**Composition with Supervisor:**

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| COMP-3 | Capture format version supported | `runtime.supports_capture(adapter.capture.format_version)` |

**Deliverables:**
- [x] `AdapterProvides` struct in code
- [x] Composition validation function
- [x] COMP-* rules in docs
- [x] Tests for composition failures

### 1.5 Implementation

- [x] Create `crates/adapter/src/manifest.rs` with schema structs
- [x] Create `crates/adapter/src/validate.rs` with rule checks
- [x] Each rule check returns `Result<(), InvalidAdapter>` (early-exit on first error)
- [x] `InvalidAdapter` implements `ErrorInfo` trait (rule_id, phase, doc_anchor, summary; path/fix best-effort)
- [x] `validate_adapter() -> Result<(), InvalidAdapter>` (single error, not accumulated)
- [x] Wire into adapter registration path
- [x] Write 15 tests (ADP-1..14, ADP-17; ADP-15/16 deferred until REP-SCOPE expansion)
- [x] Write 3 tests (COMP-1, COMP-2, COMP-3)

### 1.6 Documentation

- [x] Create `STABLE/PRIMITIVE_MANIFESTS/adapter.md` (v1 adapter contract); do not modify FROZEN
- [x] Add ADP-* invariants to PHASE_INVARIANTS.md
- [x] Add COMP-* invariants to PHASE_INVARIANTS.md
- [x] Create example adapter manifest in docs

---

## Phase 2: Source Contract

**Why second:** Source reads from Adapter. Now that Adapter defines `provides`, Source can define `requires`.

### 2.1 Schema Completion

Current schema in `source.md` is ~80% complete. Add:

```yaml
kind: source                  # MUST be literal "source"
id: string                    # ^[a-z][a-z0-9_]*$
version: semver

inputs: []                    # MUST be empty (sources take no inputs)

outputs:
  - name: string
    type: ValueType           # Number | Bool | String | Series

parameters:
  - name: string
    type: ParameterType       # int | number | bool | string | enum
    default: any
    required: bool

requires:                     # NEW: What this source needs from adapter
  context:
    - name: string            # Key from adapter's context_keys
      type: ValueType         # Must match adapter's declared type (maps to ContextRequirement.ty)
      required: bool          # If false, key may be absent at runtime

execution:
  cadence: continuous
  deterministic: true

state:
  allowed: false

side_effects: false
```

**Deliverables:**
- [x] Add `requires.context` field to schema
- [x] Define composition: `source.requires.context ⊆ adapter.provides.context`
- [x] Example valid manifests
- [x] Example invalid manifests

### 2.2 Rules Definition

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
| SRC-10 | Required context keys exist in adapter | `source.requires.context.filter(required).keys ⊆ adapter.provides.context.keys` |
| SRC-11 | Required context types match adapter | `∀ k where required: source.requires.context[k].ty == adapter.provides.context[k].ty` |
| SRC-12 | Execution deterministic | `execution.deterministic == true` |
| SRC-13 | Cadence is continuous | `execution.cadence == continuous` |

**Note on SRC-10/SRC-11:** Only keys with `required: true` must exist in adapter. Keys with `required: false` may be absent (source uses default value).

**Phases:**
- SRC-1 through SRC-9, SRC-12, SRC-13: Registration (manifest-only validation)
- SRC-10 through SRC-11: Composition (adapter + source validation)

**Deliverables:**
- [x] Complete rule table in `source.md`
- [x] Each rule has ID, description, predicate
- [x] Phase clearly indicated

### 2.3 Enforcement Mapping

| Rule ID | Phase | Error Variant | Test Name |
|---------|-------|---------------|-----------|
| SRC-1 | Registration | `InvalidManifest::InvalidId` | `src_1_invalid_id_rejected` |
| SRC-2 | Registration | `InvalidManifest::InvalidVersion` | `src_2_invalid_version_rejected` |
| SRC-3 | Registration | `InvalidManifest::WrongKind` | `src_3_wrong_kind_rejected` |
| SRC-4 | Registration | `InvalidManifest::SourceHasInputs` | `src_4_source_has_inputs_rejected` |
| SRC-5 | Registration | `InvalidManifest::NoOutputs` | `src_5_no_outputs_rejected` |
| SRC-6 | Registration | `InvalidManifest::DuplicateOutput` | `src_6_duplicate_output_rejected` |
| SRC-7 | Registration | `InvalidManifest::InvalidOutputType` | `src_7_invalid_output_type_rejected` |
| SRC-8 | Registration | `InvalidManifest::SourceHasState` | `src_8_source_has_state_rejected` |
| SRC-9 | Registration | `InvalidManifest::SourceHasSideEffects` | `src_9_source_has_side_effects_rejected` |
| SRC-10 | Composition | `CompositionError::MissingContextKey` | `src_10_missing_context_key_rejected` |
| SRC-11 | Composition | `CompositionError::ContextTypeMismatch` | `src_11_context_type_mismatch_rejected` |
| SRC-12 | Registration | `SourceValidationError::NonDeterministicExecution` | `src_12_non_deterministic_execution_rejected` |
| SRC-13 | Registration | `SourceValidationError::InvalidCadence` | `src_13_invalid_cadence_rejected` |

**Deliverables:**
- [x] Enforcement table in `source.md`
- [x] Each rule links to error variant + test
- [x] Error messages reference doc anchor (completed in Phase 7 unified error model)

### 2.4 Composition Rules

**Source requires:**
```rust
struct SourceRequires {
    context: HashMap<String, ContextRequirement>,
}

struct ContextRequirement {
    ty: ValueType,          // maps from requires.context[].type
    required: bool,
}
```

**Composition with Adapter (COMP-1, COMP-2 from Phase 1):**
- Source's required keys must exist in adapter's provided keys
- Types must match exactly

**Composition with Compute:**
| Rule ID | Rule | Predicate |
|---------|------|-----------|
| COMP-4 | Source output type equals Compute input type | `source.output.type == compute.input.type` |

**Deliverables:**
- [x] `SourceRequires` struct in code
- [x] Composition validation wired into runtime bind step (`RuntimeHandle::run`)
- [x] COMP-4 enforced at wiring validation

### 2.5 Implementation

- [x] Add `requires` field to Source manifest struct
- [x] Implement SRC-1 through SRC-9 in `source::registry::validate_manifest`
- [x] Implement SRC-10, SRC-11 in composition validator
- [x] Write 11 tests (one per rule)
- [x] Update existing source implementations with `requires`

### 2.6 Documentation

- [x] Update `STABLE/PRIMITIVE_MANIFESTS/source.md`
- [x] Remove `context` from source outputs in `STABLE/PRIMITIVE_MANIFESTS/source.md` and document
      that context access is via `requires.context`
- [x] Add SRC-* invariants to PHASE_INVARIANTS.md
- [x] Create example source manifest with context requirements

---

## Phase 3: Compute Contract

**Why third:** Most common extension point. Already at ~60%. Build on B.2 work.

### 3.1 Schema Completion

```yaml
kind: compute                 # MUST be literal "compute"
id: string                    # ^[a-z][a-z0-9_]*$
version: semver

inputs:
  - name: string
    type: ComputeInputType    # Number | Bool | Series (no String, no Event)
    required: bool
    cardinality: single       # v1: only single supported

outputs:
  - name: string
    type: ComputeOutputType   # Number | Bool | Series | String (no Event)

parameters:
  - name: string
    type: ComputeParamType    # Int | Number | Bool (no String, no Enum per runtime)
    default: any
    required: bool

execution:
  cadence: continuous         # event cadence unsatisfiable in v0 (no Trigger → Compute)
  deterministic: true
  may_error: true             # Declared in manifest

errors:                       # In doc (§2.8), not yet in code struct
  allowed: bool
  types: [ErrorType]          # DivisionByZero | NonFiniteResult | ...
  deterministic: true         # Required

state:
  allowed: bool
  resettable: bool            # Required if allowed

side_effects: false
```

**Reconciliation notes (roadmap vs current state):**

| Field | STABLE/compute.md (amended) | Runtime mapping (execute.rs) | Status |
|-------|----------------------------|------------------------------|--------|
| input types | `series \| number \| bool` | Number, Series, Bool | ✓ Aligned |
| output types | `series \| number \| bool \| string` | Number, Series, Bool, String | ✓ Aligned |
| parameter types | `int \| number \| bool` | Int, Number, Bool | ✓ Aligned |
| cadence | `continuous` only | Continuous only (event unsatisfiable) | ✓ Aligned |
| cardinality | Declared required (§2.2) | Cardinality::Single enforced | ✓ Aligned |
| may_error | Present (§2.5) | ExecutionSpec.may_error | ✓ Aligned |
| errors | Present (§2.8) | ErrorSpec in manifest | ✓ Aligned |

**Note:** Compute manifest structs use `common::ValueType`; runtime mapping narrows accepted types at execution.

**Deliverables:**
- [x] Formalize `errors` field schema (v1 work)
- [x] Document all valid ErrorTypes
- [x] Example manifests showing error declaration

### 3.2 Rules Definition

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
| CMP-11 | All outputs produced on success | (runtime invariant) |
| CMP-12 | No outputs produced on error | (runtime invariant) |
| CMP-13 | Input types valid | `inputs[].type ∈ {Number, Bool, Series}` |
| CMP-14 | Input cardinality single | `inputs[].cardinality == single` |
| CMP-15 | Parameter types valid | `parameters[].type ∈ {Int, Number, Bool}` |
| CMP-16 | Cadence is continuous | `execution.cadence == continuous` |
| CMP-17 | Execution deterministic | `execution.deterministic == true` |

**Deliverables:**
- [x] Complete rule table in `compute.md`
- [x] Each rule has ID, description, predicate
- [x] Runtime invariants (CMP-11, CMP-12) documented

### 3.3 Enforcement Mapping

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

**Deliverables:**
- [x] Enforcement table in `compute.md`
- [x] Each rule links to error variant + test
- [x] CMP-11, CMP-12 enforcement implemented

### 3.4 Composition Rules

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| COMP-5 | Input type equals upstream output type | `upstream.output.type == compute.input.type` |
| COMP-6 | Output type equals downstream input type | `compute.output.type == downstream.input.type` |

**Deliverables:**
- [x] Composition rules in `compute.md`
- [x] Type equality enforced at wiring validation

### 3.5 Implementation

- [x] Add CMP-11, CMP-12 enforcement if missing
- [x] Write 17 tests (one per rule)
- [x] Audit existing compute implementations for compliance

### 3.6 Documentation

- [x] Update `STABLE/PRIMITIVE_MANIFESTS/compute.md`
- [x] Add CMP-* invariants to PHASE_INVARIANTS.md

---

## Phase 4: Trigger Contract

**Why fourth:** Similar structure to Compute. Statelessness is the key constraint.

### 4.1 Schema Definition

```yaml
kind: trigger                 # MUST be literal "trigger"
id: string                    # ^[a-z][a-z0-9_]*$
version: semver

inputs:
  - name: string
    type: TriggerValueType    # Number | Bool | Series | Event (no String)
    required: bool
    cardinality: single       # CURRENT: only single supported

outputs:
  - name: string              # Output name (not enforced today)
    type: event               # MUST be event type

parameters:
  - name: string
    type: ParameterType       # Int | Number | Bool | String | Enum
    default: any
    required: bool

execution:
  cadence: continuous | event
  deterministic: true

state:
  allowed: false              # MUST be false (TRG-STATE-1)

side_effects: false
```

**Reconciliation notes (roadmap vs current state):**

| Field | STABLE/trigger.md | Code (TriggerValueType) | Roadmap | Action |
|-------|-------------------|-------------------------|---------|--------|
| input types | `series \| number \| bool \| event` | Number, Series, Bool, Event | Includes Event | ✓ Aligned |
| output name | Not enforced | Not enforced | "not enforced today" | ✓ Documented |

**v1 changes flagged:**
- Event inputs enable trigger chaining (Trigger → Trigger wiring) which is already allowed by wiring matrix
- Output name flexibility is for future extensibility

**Deliverables:**
- [x] Confirm schema completeness
- [x] Statelessness requirement is unambiguous

### 4.2 Rules Definition

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

**TriggerValueType:** Number | Bool | Series | Event (no String)

**Deliverables:**
- [x] Complete rule table in `trigger.md`
- [x] TRG-9 links to TRG-STATE-1 invariant

### 4.3 Enforcement Mapping

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

**Deliverables:**
- [x] Enforcement table in `trigger.md`
- [x] TRG-9 enforcement verified (TRG-STATE-1)

### 4.4 Composition Rules

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| COMP-7 | Trigger input from Compute or Trigger | `upstream.kind ∈ {"compute", "trigger"}` (per wiring matrix) |
| COMP-8 | Trigger output to Action or Trigger | `downstream.kind ∈ {"action", "trigger"}` |

**Deliverables:**
- [x] Composition rules reference wiring matrix
- [x] Enforced at validation phase

### 4.5 Implementation

- [x] Verify TRG-9 enforcement exists
- [x] Write 12 tests (one per rule)

### 4.6 Documentation

- [x] Update `STABLE/PRIMITIVE_MANIFESTS/trigger.md`
- [x] Update trigger inputs to include `event` and clarify that output name is not enforced by
      runtime (single event output is enforced)
- [x] Add TRG-* invariants to PHASE_INVARIANTS.md

---

## Phase 5: Action Contract

**Why fifth:** Terminal node. Similar structure to Trigger. Side effects are the key distinction.

### 5.1 Schema Definition

```yaml
kind: action                  # MUST be literal "action"
id: string                    # ^[a-z][a-z0-9_]*$
version: semver

inputs:
  - name: string
    type: ActionValueType     # Event | Number | Bool | String (no Series)
    required: bool
    cardinality: single       # CURRENT: only single supported

outputs:
  - name: outcome             # MUST be named "outcome"
    type: event               # MUST be event type

parameters:
  - name: string
    type: ParameterType       # Int | Number | Bool | String | Enum
    default: any
    required: bool

execution:
  deterministic: true         # Command emission is deterministic
  retryable: false            # Retry behavior explicit

state:
  allowed: false              # Actions are stateless

side_effects: true            # MUST be true (only primitive with side effects)

effects:                      # What effects this action may emit
  writes:
    - name: string            # Context key name to write
      type: ValueType         # Type of value written
```

**ActionValueType:** Event | Number | Bool | String (no Series)

**Note:** `effects.writes` declares intent. Actual write happens via `set_context` effect emitted at runtime.

**Deliverables:**
- [x] Confirm schema completeness
- [x] Side effects requirement is unambiguous

### 5.2 Rules Definition

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
| ACT-12 | Gated by trigger | (wiring validation, R.7) |
| ACT-13 | Effects block must exist | `effects is present` (may have empty writes) |
| ACT-14 | Write names unique | `unique(effects.writes[].name)` |
| ACT-15 | Write types valid | `all(effects.writes[].type ∈ {Number, Bool, String})` |
| ACT-16 | Retryable false | `execution.retryable == false` |
| ACT-17 | Execution deterministic | `execution.deterministic == true` |

**ActionValueType:** Event | Number | Bool | String (no Series)

**WriteValueType:** Number | Bool | String (no Series, no Event — matching ActionValueType excluding Event)

**Deliverables:**
- [x] Complete rule table in `action.md`
- [x] ACT-12 links to R.7 invariant

### 5.3 Enforcement Mapping

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

**Deliverables:**
- [x] Enforcement table in `action.md`
- [x] ACT-12 links to existing R.7 enforcement

### 5.4 Composition Rules

| Rule ID | Rule | Predicate |
|---------|------|-----------|
| COMP-9 | Action input from Trigger only | `upstream.kind == "trigger"` (all inputs today) |
| COMP-10 | Action output not wireable | `downstream.len == 0` (terminal) |
| COMP-11 | Action writes target provided keys | `action.effects.writes.names ⊆ adapter.context_keys.names` |
| COMP-12 | Action writes only writable keys | `∀n ∈ writes: adapter.key[n].writable == true` |
| COMP-13 | Action write types match | `∀n ∈ writes: action.type[n] == adapter.key[n].type` |
| COMP-14 | If action writes, adapter accepts set_context | `writes.len > 0 => accepts.effects contains "set_context"` |
| COMP-15 | If action writes, capture includes effect + keys (planned; REP-SCOPE update required) | `writes.len > 0 => ("effect.set_context" ∈ capture.fields AND ∀n: "context." + n ∈ capture.fields)` |

**Note (planned):** COMP-15 depends on capture including context/effect; it is deferred until REP-SCOPE expands.

**Note (CURRENT):** Wiring only permits Trigger → Action, and triggers emit only event outputs.
Non-event action inputs are therefore not satisfiable without wiring changes.

**Deliverables:**
- [x] Composition rules in `action.md`
- [x] Terminal nature explicit

### 5.5 Composition Enforcement Mapping

| Rule ID | Phase | Error Variant | Test Name |
|---------|-------|---------------|-----------|
| COMP-11 | Composition | `CompositionError::WriteTargetNotProvided` | `comp_11_write_target_not_provided_rejected` |
| COMP-12 | Composition | `CompositionError::WriteTargetNotWritable` | `comp_12_write_target_not_writable_rejected` |
| COMP-13 | Composition | `CompositionError::WriteTypeMismatch` | `comp_13_write_type_mismatch_rejected` |
| COMP-14 | Composition | `CompositionError::MissingSetContextEffect` | `comp_14_missing_set_context_rejected` |
| COMP-15 | Composition | `CompositionError::WritesNotCaptured` | `comp_15_writes_not_captured_rejected` | **Deferred: REP-SCOPE**

### 5.6 Implementation

- [x] Verify all ACT-* rules enforced
- [x] Write 17 tests (one per rule)
- [x] Write 4 composition tests (COMP-11 through COMP-14; COMP-15 deferred)

### 5.7 Documentation

- [x] Update `STABLE/PRIMITIVE_MANIFESTS/action.md`
- [x] Add ACT-* invariants to PHASE_INVARIANTS.md

---

## Phase 6: Cluster Contract Completion

**Why sixth:** Already most complete (~70%). Finishing touch.

### 6.1 Remaining Gaps

- [x] Add enforcement loci for all expansion rules
- [x] Add enforcement loci for all validation rules
- [x] Map each rule to error type

### 6.2 Deliverables

- [x] Enforcement table in `CLUSTER_SPEC.md`
- [x] All E-* (expansion) invariants mapped to code
- [x] All V-* (validation) invariants mapped to code

**Phase 6 closure note:** Mapping completeness is satisfied even where a rule has no dedicated runtime error variant (for example, type-level or assertion-level enforcement). `I.6` remains explicitly mapped as **not implemented** in `CLUSTER_SPEC.md` and `PHASE_INVARIANTS.md`.

---

## Phase 7: Integration and Tooling

### 7.1 Unified Error Model

**Current state:** Validation + composition errors implement a common trait (`ErrorInfo`). The CLI renders errors uniformly via `RuleViolation`.

**Phase 7 target:** All validation error enums implement `ErrorInfo`. CLI renders errors uniformly via `RuleViolation`.

```rust
use std::borrow::Cow;

pub trait ErrorInfo {
    fn rule_id(&self) -> &'static str;
    fn phase(&self) -> Phase;
    fn doc_anchor(&self) -> &'static str;
    fn summary(&self) -> Cow<'static, str>;
    fn path(&self) -> Option<Cow<'static, str>>;
    fn fix(&self) -> Option<Cow<'static, str>>;
}

pub struct RuleViolation {
    pub rule_id: &'static str,
    pub phase: Phase,
    pub doc_anchor: &'static str,
    pub summary: Cow<'static, str>,
    pub path: Option<Cow<'static, str>>,
    pub fix: Option<Cow<'static, str>>,
}

impl<E: ErrorInfo> From<E> for RuleViolation {
    fn from(e: E) -> Self {
        RuleViolation {
            rule_id: e.rule_id(),
            phase: e.phase(),
            doc_anchor: e.doc_anchor(),
            summary: e.summary(),
            path: e.path(),
            fix: e.fix(),
        }
    }
}

pub enum Phase {
    Registration,
    Composition,
    Execution,
}
```

**Deliverables:**
- [x] `ErrorInfo` trait in `crates/runtime/src/common/error_info.rs`
- [x] `Phase` enum
- [x] `RuleViolation` presentation struct
- [x] Retrofit `ErrorInfo` to existing validation error enums:
  - [x] `ValidationError` (graph validation) + `ValidationError` (compute manifest)
  - [x] `SourceValidationError`
  - [x] `TriggerValidationError`
  - [x] `ActionValidationError`
  - [x] `ExpandError` (+ nested signature errors)
  - [x] `ExecError` (execution rule IDs: CMP-11/12, SRC-10/11, NUM-FINITE-1, X.11)
- [x] CLI maps typed errors → `RuleViolation` for uniform rendering
- [ ] (Optional) Accumulation mode for improved author UX (explicitly deferred; early-exit semantics remain acceptable)

**Phase 7 closure note:** Required deliverables for 7.1 through 7.5 are complete. Optional accumulation remains deferred by design and is not a blocker for Phase 7 closure.

### 7.2 CLI: `ergo validate`

```bash
# Validate any extension artifact
ergo validate <manifest.yaml>

# Success output:
✓ Manifest valid
  Kind: compute
  ID: my_compute
  Version: 0.1.0
  Inputs: 2
  Outputs: 1
  Rules passed: 12/12

# Failure output:
✗ Manifest invalid

  CMP-4  No inputs declared
         Path: $.inputs
         Fix: Add at least one entry to inputs field
         Docs: STABLE/PRIMITIVE_MANIFESTS/compute.md#CMP-4

  CMP-8  Compute has side effects
         Path: $.side_effects
         Fix: Set side_effects to false
         Docs: STABLE/PRIMITIVE_MANIFESTS/compute.md#CMP-8
```

**Deliverables:**
- [x] `ergo validate` command
- [x] Auto-detects kind from manifest
- [x] Validates: Source, Compute, Trigger, Action, Adapter
- [x] Returns exit code 0 on success, 1 on failure
- [x] Machine-readable output option: `--format json`

### 7.3 CLI: `ergo check-compose`

```bash
# Validate that pieces compose
ergo check-compose <adapter.yaml> <source.yaml>

# Success output:
✓ Composition valid
  Adapter provides: price, volume, timestamp
  Source requires: price, volume
  All requirements satisfied

# Failure output:
✗ Composition invalid

  SRC-10  Missing context key
          Path: $.requires.context.bid
          Fix: Add "bid" to adapter's context_keys, or remove from source's requires
          Docs: STABLE/PRIMITIVE_MANIFESTS/source.md#SRC-10
```

**Deliverables:**
- [x] `ergo check-compose` command
- [x] Validates Source ↔ Adapter compatibility
- [x] Validates Action ↔ Adapter compatibility (write path)
- [x] Returns same `Vec<RuleViolation>` structure
- [x] Machine-readable output option: `--format json`

### 7.4 Rule Registry (Implementation Mirror)

```rust
// Rules are encoded in code for enforcement; STABLE docs define the rules.
// Any generated tables must match the committed docs.
static RULES: &[RuleDefinition] = &[
    RuleDefinition {
        id: "ADP-5",
        phase: Phase::Registration,
        summary: "Context key names unique",
        predicate: "unique(context_keys[].name)",
        doc_anchor: "STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-5",
        check_fn: check_adp_5,
    },
    // ... all other rules
];

fn generate_docs() -> String {
    // Generate markdown tables from RULES
}
```

**Deliverables:**
- [x] `RuleDefinition` struct
- [x] Static rule registry per extension type
- [x] `ergo gen-docs` command generates markdown from rules (`docs/STABLE/RULE_REGISTRY.md`)
- [x] CI check: generated docs match committed docs (`ergo gen-docs --check`)

### 7.5 Test Harness

Every rule has a canonical test:

```rust
#[test]
fn adp_5_duplicate_context_key_rejected() {
    let manifest = r#"
        kind: adapter
        id: test_adapter
        version: 0.1.0
        context_keys:
          - name: price
            type: Number
          - name: price  # duplicate!
            type: Number
    "#;
    
    let result = validate_adapter(manifest);
    
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert_eq!(violations[0].rule_id, "ADP-5");
    assert_eq!(violations[0].path, "$.context_keys[1].name");
}
```

**Deliverables:**
- [x] One test per rule (ADP-1..17, SRC-1..11, CMP-1..17, TRG-1..12, ACT-1..17, COMP-1..15, plus cluster D./E./V. coverage)
- [x] Tests assert rule_id and path (where meaningful; otherwise path == None)
- [x] Tests serve as documentation

---

## Phase 8: Stdlib State Implementations

**Why:** Close the state threading loop with two canonical implementations.

### Core Freeze Exception

**PHASE_INVARIANTS declares:** "Action implementations in core = zero by design; capability atoms live in verticals."

**This phase requires an unfreeze exception** because:

1. **Vertical proof:** State threading is required for any vertical that implements temporal patterns (once, count, latch, debounce). Without `ctx_set_*`, every vertical must implement its own context-write action, creating duplication and consistency risks.

2. **Domain neutrality:** `ctx_set_*` is domain-agnostic (writes typed values to keyed context). It has no trading/vertical-specific semantics. It is infrastructure, not capability.

3. **Minimal surface:** Three type-specific implementations (number, bool, string) with no behavioral parameters.

**To proceed:** Requires Sebastian authorization before implementation begins.

### 8.1 Context Sources (Source Implementation Family)

**Kind:** Source (no inputs, reads ExecutionContext)

These are a family of implementations, one per supported type:
- `context_number_source` (replaces existing hardcoded implementation, bumps to 0.2.0)
- `context_bool_source`
- `context_string_source`

A context source reads a value from the execution context — a key-value map provided by the adapter at each event. The source does not know or care whether the adapter populated that key from external data (market feed, user command) or from a prior episode's `context_set_*` write. Both paths are identical from the source's perspective.

**Manifest (context_number_source):**
```yaml
kind: source
id: context_number_source
version: 0.2.0

inputs: []                    # Sources have no inputs

outputs:
  - name: value
    type: Number

parameters:
  - name: key
    type: String
    required: true
  - name: default
    type: Number
    required: true

execution:
  deterministic: true
  cadence: continuous

state:
  allowed: false

side_effects: false

requires:
  context:
    - name: $key              # Bound at instantiation from parameter
      type: Number
      required: false         # Key may be absent (default used)
```

**Note:** `requires.context[].name` is bound from the `key` parameter at instantiation time. The `required: false` allows the key to be absent, in which case `default` is used.

**Semantics:**
1. If `ExecutionContext[key]` exists and is Number → output it
2. Else → output `default`

**Runtime invariants:**
- Deterministic given ExecutionContext snapshot
- Type mismatch prevented by composition validation (COMP-2: adapter type must match source requirement)

**Note:** Current `SourcePrimitive::produce` returns `HashMap<String, Value>`, not `Result`. Sources cannot return runtime errors. Type safety is enforced at composition time via manifest validation, not at runtime.

### 8.2 Context Set (Action Implementation Family)

**Kind:** Action (terminal, emits effects)

These are a family of implementations, one per supported type:
- `ctx_set_number`
- `ctx_set_bool`
- `ctx_set_string`

**Manifest (ctx_set_bool):**
```yaml
kind: action
id: ctx_set_bool
version: 0.1.0

inputs:
  - name: event
    type: Event
  - name: value
    type: Bool

outputs:
  - name: outcome
    type: Event

parameters:
  - name: key
    type: String
    required: true

execution:
  deterministic: true
  retryable: false

state:
  allowed: false

side_effects: true

effects:
  writes:
    - name: $key              # Bound from parameter at instantiation
      type: Bool
```

**Note:** `effects.writes[].name` is bound from the `key` parameter at instantiation time.

**Semantics:**
1. When triggered, emit effect description:
   ```json
   {
     "kind": "set_context",
     "payload": {
       "writes": [{ "key": "<key>", "value": <value> }]
     }
   }
   ```
2. Adapter applies effect to external store after episode
3. Next episode's context snapshot reflects the write

**Runtime invariants:**
- Effect emitted to adapter only; supervisor never sees it
- Deterministic command emission given inputs

### 8.3 Implementation Deliverables

- [ ] `context_number_source@0.2.0`, `context_bool_source@0.1.0`, `context_string_source@0.1.0` in `crates/runtime/src/source/implementations/`
- [ ] Remove `context_number_source@0.1.0` (hardcoded `"x"`)
- [ ] `context_set_number@0.1.0`, `context_set_bool@0.1.0`, `context_set_string@0.1.0` in `crates/runtime/src/action/implementations/`
- [ ] `$key` parameter resolution in manifest/composition validation
- [ ] Effect routing: runtime collects effects → adapter applies
- [ ] Tests: source reads default, source reads existing, source reads parameterized key, action emits effect (per type)
- [ ] Tests: composition validation catches missing adapter provision for resolved `$key`

### 8.4 Breaking Change Migration

`context_number_source` bumps from 0.1.0 to 0.2.0. The hardcoded key `"x"` is removed. The `key` and `default` parameters are now required. Existing graphs using `context_number_source@0.1.0` without parameters will fail semver resolution against `0.2.0` and must be updated.

**Files requiring migration (same PR as source implementations):**
- [ ] `dual_ma_crossover.yaml:7` — bump version, add `params: { key: "x", default: 0.0 }`
- [ ] `sandbox/trading_vertical/price_breakout.yaml:7` — bump version, add `params: { key: "x", default: 0.0 }`
- [ ] `docs/STABLE/YAML_GRAPH_FORMAT.md:54` — update example
- [ ] `crates/ergo-cli/src/graph_yaml.rs:2240` — update inline test YAML
- [ ] `crates/supervisor/src/demo/demo_1.rs:20` — remove `CONTEXT_NUMBER_KEY` constant, update graph builder

---

## Phase 9: Key Derivation Convention

**Why:** Prevent state key collisions when clusters are instantiated multiple times.

**Location:** Add to CLUSTER_SPEC.md or AUTHORING_LAYER.md (STABLE)

### 9.1 Specification

```
derive_key(authoring_path, slot_name) → string
```

**Requirements:**
- Deterministic
- Collision-free across distinct `authoring_path`
- Stable under replay given identical authoring artifact

**Format:**

`authoring_path` is `Vec<(ClusterId, NodeId)>` as defined in CLUSTER_SPEC.md §7.2.

Recommended key format:
```
"__ergo/" + join(authoring_path.map(|(cluster_id, node_id)| cluster_id + "#" + node_id), "/") + "/" + slot_name
```

**Example:**
```
authoring_path = [("root_cluster", "entry_node"), ("once_cluster", "gate_trigger")]
slot_name = "has_fired"

derive_key(...) → "__ergo/root_cluster#entry_node/once_cluster#gate_trigger/has_fired"
```

### 9.2 Usage in Temporal Clusters

```yaml
OnceCluster:
  parameters:
    - name: state_key
      type: String
      default: derive_key(authoring_path, "has_fired")  # Auto-unique

  input_ports:
    - name: signal
      type: Bool           # Incoming signal to gate

  output_ports:
    - name: outcome
      type: Event
      maps_to: ack.outcome

  nodes:
    has_fired_source:
      impl: context_bool_source@0.1.0
      params:
        key: $state_key
        default: false

    not_fired:
      impl: not@0.1.0       # Negate: true when has NOT fired
      # inputs wired below

    should_fire:
      impl: and@0.1.0       # AND: signal is active AND has not fired
      # inputs wired below

    gate:
      impl: emit_if_true@0.1.0
      # inputs wired below

    fired_value:
      impl: bool_source@0.1.0
      params:
        value: true          # Constant: "I have fired"

    ack:
      impl: ack_action@0.1.0
      params:
        accept: true

    set_fired:
      impl: context_set_bool@0.1.0
      params:
        key: $state_key

  edges:
    # State read → negate
    - from: has_fired_source.value
      to: not_fired.a

    # Incoming signal + not-fired → AND gate
    - from: $signal                    # Cluster input port
      to: should_fire.a
    - from: not_fired.value
      to: should_fire.b

    # AND result → trigger
    - from: should_fire.value
      to: gate.value

    # Trigger gates both actions
    - from: gate.event
      to: ack.event
    - from: gate.event
      to: set_fired.event

    # Constant true feeds the write action
    - from: fired_value.value
      to: set_fired.value
```

**Data flow:**
1. `has_fired_source` reads `$state_key` from context (default: `false`)
2. `not_fired` inverts it — `true` on first episode, `false` after firing
3. `should_fire` ANDs the incoming signal with not-fired
4. `gate` emits event only when both conditions hold
5. `ack` acknowledges the event (output exposed as cluster outcome)
6. `set_fired` writes `true` to `$state_key` — adapter persists it
7. Next episode: `has_fired_source` reads `true`, `not_fired` outputs `false`, gate stays closed

### 9.3 Collision Rules

| Scenario | Behavior |
|----------|----------|
| Default key (no override) | Unique by construction via `authoring_path` |
| Explicit key override | User responsibility — intentional sharing allowed |
| Same cluster instantiated twice | Different `authoring_path` → different derived keys |

### 9.4 Deliverables

- [ ] `derive_key` function in cluster expansion
- [ ] Documentation in CLUSTER_SPEC.md
- [ ] Test: same cluster twice → different keys

---

## Phase 10: Golden Spike Test

**Why:** End-to-end validation that state threading works.

### 10.1 Test Scenario

1. Construct `OnceCluster` twice in same graph (different authoring paths)
2. Verify derived keys differ
3. Run two episodes:
   - Episode 1: triggers, emits `set_context` write
   - Episode 2: reads updated context, does NOT trigger
4. Assert:
   - Adapter-applied state persists across episodes via external store
   - Replay produces identical scheduling decisions (REP-SCOPE)

### 10.2 Test Structure

```rust
#[test]
fn golden_spike_once_cluster_state_threading() {
    // Setup: two OnceCluster instances
    let graph = build_graph_with_two_once_clusters();
    
    // Verify key derivation
    let keys = extract_state_keys(&graph);
    assert_ne!(keys.entry_cluster, keys.exit_cluster);
    
    // Episode 1: fresh state
    let adapter = TestAdapter::new();
    let ctx1 = adapter.build_context(); // has_fired = false (missing, default)
    let result1 = runtime.run(&graph, ctx1);
    
    assert!(result1.triggered());
    assert_eq!(result1.effects.len(), 1);
    assert_eq!(result1.effects[0].kind, "set_context");
    
    // Adapter applies effect
    adapter.apply_effects(&result1.effects);
    
    // Episode 2: state persisted
    let ctx2 = adapter.build_context(); // has_fired = true (from prior write)
    let result2 = runtime.run(&graph, ctx2);
    
    assert!(!result2.triggered());
    assert_eq!(result2.effects.len(), 0);
    
    // Replay verification (scheduling decisions only; REP-SCOPE)
    assert!(replay_decisions_match(&graph, &adapter));
}
```

### 10.3 Deliverables

- [ ] Golden spike test implemented
- [ ] Documents end-to-end state threading flow
- [ ] Serves as reference for cluster authors

---

## Timeline Estimate

| Phase | Scope | Tests | Effort | Dependencies |
|-------|-------|-------|--------|--------------|
| 1. Adapter | Complete from ~20% | 17 ADP + 3 COMP | 3-4 days | None |
| 2. Source | Complete from ~50% | 11 SRC | 1-2 days | Phase 1 |
| 3. Compute | Complete from ~60% | 12 CMP | 1 day | None |
| 4. Trigger | Complete from ~60% | 10 TRG | 1 day | None |
| 5. Action | Complete from ~60% | 15 ACT + 5 COMP | 2 days | Phase 1 |
| 6. Cluster | Complete from ~70% | existing + updates | 1 day | Phases 1-5 |
| 7. Tooling | New CLI + rule registry | integration tests | 2-3 days | Phases 1-6 |
| 8. Stdlib | ctx_get_or_default_*, ctx_set_* | 6 impl tests | 1-2 days | Phase 1 |
| 9. Key derivation | derive_key convention | 2 tests | 0.5 day | Phase 8 |
| 10. Golden spike | End-to-end test | 1 golden spike | 0.5 day | Phases 8, 9 |

**Total: ~14-17 days of focused work**

**Test count: ~80 rule tests + integration tests + golden spike**

---

## Success Metrics

When complete:

1. **Zero ambiguity:** Developer reads contract, knows exactly what to build
2. **Zero human review:** System accepts or rejects, no judgment calls
3. **Zero silent failures:** Every violation produces specific, actionable error
4. **Universal composition:** Any compliant piece works with any other compliant piece
5. **State threading works:** Temporal patterns (once, count, debounce) implementable as clusters

---

## Rule Summary

All rules by extension type:

| Extension | Rules | Count |
|-----------|-------|-------|
| Adapter | ADP-1..ADP-17 | 17 |
| Source | SRC-1..SRC-13 | 13 |
| Compute | CMP-1..CMP-17 | 17 |
| Trigger | TRG-1..TRG-12 | 12 |
| Action | ACT-1..ACT-17 | 17 |
| Composition | COMP-1..COMP-15 | 15 |
| **Total** | | **91** |

---

## Governance

**Status:** v1.0.0-alpha.8 — Breaking changes permitted.

This roadmap is a v1 workstream item.

- **v1 alpha:** FROZEN documents remain immutable; v1 contracts live in STABLE
- Adapter contract v1 is authored in `STABLE/PRIMITIVE_MANIFESTS/adapter.md`
- Primitive manifest contracts are STABLE (additive preferred, breaking allowed in alpha)
- New invariants added to PHASE_INVARIANTS.md
- Progress tracked in GitHub Issues

---

## Related Documents

- `FROZEN/adapter_contract.md` — Adapter contract v0 (frozen)
- `FROZEN/SUPERVISOR.md` — Supervisor contract v0 (frozen, provenance rule source)
- `STABLE/PRIMITIVE_MANIFESTS/adapter.md` — Adapter contract v1 (to be authored)
- `STABLE/PRIMITIVE_MANIFESTS/source.md` — Source contract
- `STABLE/PRIMITIVE_MANIFESTS/compute.md` — Compute contract
- `STABLE/PRIMITIVE_MANIFESTS/trigger.md` — Trigger contract
- `STABLE/PRIMITIVE_MANIFESTS/action.md` — Action contract
- `STABLE/CLUSTER_SPEC.md` — Cluster contract
- `CANONICAL/PHASE_INVARIANTS.md` — Enforcement loci

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| v1.0.0-alpha.1 | 2025-01-10 | Sebastian | Initial roadmap |
| v1.0.0-alpha.2 | 2025-01-11 | Claude (Structural Auditor) | Added state threading: ADP-11..17, ACT-13..15, COMP-11..15, Phases 8-10, provenance rule, capture selectors |
| v1.0.0-alpha.3 | 2026-01-11 | Claude (Structural Auditor) | FROZEN doc amendments applied: SUPERVISOR.md §2.2 provenance rule, ontology.md §2.4 effect descriptions. Sebastian override authorization. |
| v1.0.0-alpha.4 | 2026-01-11 | Claude (Structural Auditor) | Codex audit fixes: (1) ValueType multiple enums clarified, (2) Source of truth governance note, (3) Phase 8 unfreeze justification added, (4) Compute reconciliation table corrected, (5) Trigger schema reconciliation table |
| v1.0.0-alpha.5 | 2026-01-11 | Claude (Structural Auditor) | Codex re-audit fixes: (1) RuleViolation marked as Phase 7 target not current, (2) CMP-12 ExecError::OutputOnError marked TO BE ADDED, (3) Source error path fixed (trait can't return errors), (4) SRC-10/SRC-11 predicates filter by required |
| v1.0.0-alpha.6 | 2026-01-21 | Claude (Structural Auditor) | Error model fork resolved: (1) Added ErrorInfo trait with MUST/SHOULD gradation, (2) RuleViolation is presentation format not internal, (3) Phase 1.3 InvalidAdapter typed enum with ErrorInfo impl, (4) Phase 7.1 retrofits existing enums, (5) path/fix are SHOULD not MUST, (6) ADP-15/16 enforcement deferred until REP-SCOPE expansion |
| v1.0.0-alpha.7 | 2026-01-21 | Claude (Structural Auditor) | Phase 1.3 consistency fixes per ChatGPT review: (1) Added index fields to all list-locus variants, (2) Complete path()/fix() implementations, (3) ADP-6 requires String parsing not ValueType, (4) ADP-10/ADP-11 semantics clarified, (5) doc_anchor format locked to STABLE/PRIMITIVE_MANIFESTS/adapter.md#ADP-N |
| v1.0.0-alpha.8 | 2026-02-05 | Claude Code | Marked Phase 5 (Action) complete; updated Current State table (Phases 1-5 done); corrected Rule Summary counts (91 total rules) |
| v1.0.0-alpha.9 | 2026-02-17 | Codex | Hard break cleanup: capture bundle schema moved to strict v1 (`capture_version: v1`, required `adapter_provenance`, unknown fields denied), legacy `adapter_version` bundles rejected, and canonical/replay strictness docs aligned |
