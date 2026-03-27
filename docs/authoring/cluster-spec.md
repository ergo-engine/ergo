---
Authority: STABLE
Version: v0.3
Last Updated: 2026-03-26
Scope: Data structures, inference algorithm, validation rules
Change Rule: Additive only
---

# Cluster Specification

---

> **Changelog (v0.3):** Accuracy corrections to match the current implementation surface.
>
> - `ParameterSpec.default` now documents `ParameterDefault`
> - signature inference/declared-signature enforcement text now matches current runtime behavior
> - expansion pseudocode now reflects version selector resolution and `derive_key` handling
> - signature hashing is marked as design-only, not currently implemented
>
> **Changelog (v0.2):** Terminology alignment with terminology.md. No semantic changes.
>
> - `NodeKind::Primitive` → `NodeKind::Impl`
> - `primitive_id` → `impl_id`
> - `PrimitiveInstance` → `ImplementationInstance`
> - `primitive` field → `implementation` field
>
> The term "primitive" now refers exclusively to the four ontological roles (Source, Compute, Trigger, Action). Concrete executable nodes are called "implementations."

This document defines the formal specification for clusters.
It includes data structures, inference algorithms, and validation rules.

This specification implements the authoring layer defined in `concepts.md`.

---

## 1. Data Structures

### 1.1 Cluster Definition

```
ClusterDefinition {
    id: String,
    version: Version,
    
    // Internal structure
    nodes: Map<NodeId, NodeInstance>,
    edges: List<Edge>,
    
    // Boundary
    input_ports: List<InputPortSpec>,
    output_ports: List<OutputPortSpec>,
    parameters: List<ParameterSpec>,
    
    // Optional declared signature (verified against inferred)
    declared_signature: Option<Signature>,
}
```

### 1.2 Node Instance

```
NodeInstance {
    id: NodeId,
    kind: NodeKind,
    parameter_bindings: Map<String, ParameterBinding>,
}

NodeKind =
    | Impl { impl_id: String, version: Version }
    | Cluster { cluster_id: String, version: Version }
```

### 1.3 Edge

```
Edge {
    from: OutputRef,
    to: InputRef,
}

OutputRef {
    node_id: NodeId,
    port_name: String,
}

InputRef {
    node_id: NodeId,
    port_name: String,
}
```

### 1.4 Port Specifications

```
InputPortSpec {
    name: String,
    maps_to: GraphInputPlaceholder,
}

OutputPortSpec {
    name: String,
    maps_to: OutputRef,  // References internal node output
}

GraphInputPlaceholder {
    name: String,
    ty: ValueType,
    required: bool,
}
```

### 1.5 Parameter Specification

```
ParameterSpec {
    name: String,
    ty: ParameterType,
    default: Option<ParameterDefault>,
    required: bool,
}

ParameterDefault =
    | Literal(ParameterValue)
    | DeriveKey { slot_name: String }

ParameterBinding =
    | Literal { value: ParameterValue }
    | Exposed { parent_param: String }
```

---

## 2. Signature

The **Signature** is the canonical description of a cluster's boundary.

```
Signature {
    kind: BoundaryKind,
    inputs: List<PortSpec>,
    outputs: List<PortSpec>,
    has_side_effects: bool,
    is_origin: bool,
}

PortSpec {
    name: String,
    ty: ValueType,
    cardinality: Cardinality,
    wireable: bool,
}

BoundaryKind = SourceLike | ComputeLike | TriggerLike | ActionLike

ValueType = Number | Series | Bool | Event | String

Cardinality = Single | Multiple
```

---

## 3. Signature Inference Algorithm

Given a cluster definition, the signature is inferred as follows:

### Step 0: Expand to Implementations

```
G_expanded = expand_all_clusters(cluster.nodes, cluster.edges)
```

All nested clusters are recursively expanded until only implementations remain. Partial expansion is not permitted.

### Step 1: Compute Boundary Port Sets

```
B_in = cluster.input_ports
B_out = cluster.output_ports
```

Validate:

- Every `B_out` reference exists in `G_expanded`
- Port names are unique
- Each port's type is inferrable from its source

### Step 2: Infer Port Types and Wireability

For each output port `p` in `B_out`:

```
referenced_node = G_expanded.nodes[p.maps_to.node_id]
referenced_output = referenced_node.manifest.outputs[p.maps_to.port_name]

p.ty = referenced_output.value_type
p.cardinality = referenced_output.cardinality

if referenced_node.kind == Action:
    p.wireable = false  # Action outputs are never wireable
else:
    p.wireable = true
```

Action outputs are never wireable, regardless of output type. No future Action manifest may override this rule.

For each input port `p` in `B_in`:

```
p.ty = p.maps_to.ty
p.cardinality = Single
p.wireable = false
```

Current implementation hard-sets every inferred boundary input port to
`Cardinality::Single` and `wireable: false`. Input cardinality is not inferred from usage today.

### Step 3: Infer Flags

```
has_side_effects = G_expanded.nodes.any(n => n.kind == Action)

is_origin = B_in.is_empty() AND 
            G_expanded.roots.all(n => n.kind == Source)
```

Where `roots` are nodes with no incoming edges from other nodes in the subgraph.

### Step 4: Infer BoundaryKind

```
has_wireable_outputs = B_out.any(p => p.wireable)
wireable_out_types = B_out.filter(p => p.wireable).map(p => p.ty).to_set()
has_wireable_event_out = Event ∈ wireable_out_types

if !has_wireable_outputs:
    kind = ActionLike

else if B_in.is_empty() AND wireable_out_types ⊆ {Number, Series, Bool, String}:
    kind = SourceLike

else if has_wireable_event_out:
    kind = TriggerLike

else:
    kind = ComputeLike
```

BoundaryKind is determined solely by boundary wireability, not by internal node kinds except as they affect wireability. The `has_side_effects` flag does not influence BoundaryKind.

### Step 5: Assemble Signature

```
Signature {
    kind: kind,
    inputs: B_in.map(to_port_spec),
    outputs: B_out.map(to_port_spec),
    has_side_effects: has_side_effects,
    is_origin: is_origin,
}
```

---

## 4. Declared Signature Verification

Current implementation does not yet perform a full declared-signature compatibility check.
Today it enforces one compatibility rule:

```
for every declared port:
    declared.wireable must not exceed inferred.wireable
```

A declared signature may therefore restrict wireability, but it cannot grant it.
Kind equality, input/output subset checks, `has_side_effects`, and `is_origin`
remain design intent rather than current prod enforcement.

---

## 5. Wiring Matrix

The wiring matrix for clusters mirrors the ontological primitive wiring matrix:

```
SourceLike  → ComputeLike  : allowed
SourceLike  → TriggerLike  : forbidden (v0)
SourceLike  → ActionLike   : allowed for scalar payload inputs only (TriggerLike event gate still required)
ComputeLike → ComputeLike  : allowed
ComputeLike → TriggerLike  : allowed
ComputeLike → ActionLike   : allowed for scalar payload inputs only (TriggerLike event gate still required)
TriggerLike → TriggerLike  : allowed
TriggerLike → ActionLike   : allowed
ActionLike  → *            : forbidden (terminal)
*           → SourceLike   : forbidden (origin)
```

`ActionLike` rows are a coarse boundary-level summary. The executable rule is refined by
expanded Action input type:

- `event` inputs are gating inputs and must originate from `Trigger`
- scalar inputs are payload inputs and may originate from `Source` or `Compute`
- scalar payload inputs do not satisfy trigger gating; `V.5` still requires Trigger-gated action execution

This matrix applies at every nesting level.

---

## 6. Validation Rules

### 6.1 Definition-Time Validation

Current prod validation is split across four stages:

1. **Loader decode**
   - graph text is parsed
   - shorthand edges, typed defaults, and version coercions are normalized
   - identifier constraints and declared external-input references are checked

2. **`validate_cluster_definition()`**
   - duplicate input/output/parameter names are rejected
   - parameter defaults are type-checked
   - malformed `derive_key` defaults are rejected

3. **Expansion**
   - empty clusters are rejected
   - parameter bindings are validated
   - version selectors are resolved
   - boundary-output references are mapped

4. **`runtime::validate()`**
   - cycles, wiring legality, type checks, required-input checks, and catalog-backed primitive existence checks are enforced on the expanded graph

Definition-time validation is therefore context-independent in intent, but it is not a single
"save-time" pass that fully proves executable correctness before expansion/runtime validation.

### 6.2 Instantiation-Time Validation

When a cluster is placed in a parent context, validate:

1. **Wiring compatibility**
   - Current prod enforcement happens after expansion on primitive edges and primitive kinds
   - `BoundaryKind` is inferred/parsed, but parent/child wiring is not directly validated against cluster boundary kinds at instantiation time

2. **Parameter completeness**
   - All required parameters are either bound or exposed
   - Bound values are type-compatible
   - Exposed parameters exist in parent context

3. **Version compatibility**
   - Requested selectors must be valid strict semver or semver constraints
   - Expansion resolves the highest satisfying available version

### 6.3 Expansion-Time Validation

Before execution, after full expansion, validate:

1. **Full DAG validation**
   - No cycles in expanded graph
   - All edges are valid
   - All required inputs are connected

2. **Type compatibility**
   - All edge connections have matching types

3. **Execution preconditions**
   - All nodes pass validation before any action executes
   - All parameters are bound to concrete values

### 6.4 Enforcement Mapping (Phase 6)

This section maps cluster rules to enforcement loci and error types. It mirrors
the phase invariants in `docs/invariants/INDEX.md`.

**Definition-Time (D.*)**

| ID | Rule | Enforcement Locus | Error Type / Notes |
|----|------|-------------------|--------------------|
| D.1 | Cluster contains ≥1 node | `cluster.rs::expand_with_context` | `ExpandError::EmptyCluster` |
| D.2 | Edges reference existing nodes/ports | `runtime/validate.rs` (post-expansion) | `ValidationError::UnknownNode` / `MissingInputMetadata` / `MissingOutputMetadata` |
| D.3 | Edges satisfy wiring matrix | `runtime/validate.rs::enforce_wiring_matrix` | `ValidationError::InvalidEdgeKind` |
| D.4 | Output ports reference valid internal node outputs | `cluster.rs::map_boundary_outputs` + `infer_signature` | `ExpandError::UnmappedBoundaryOutput` / `ExpandError::SignatureInferenceFailed` |
| D.5 | Input port names unique | `cluster.rs::validate_cluster_definition` | `ExpandError::DuplicateInputPort` |
| D.6 | Output port names unique | `cluster.rs::validate_cluster_definition` | `ExpandError::DuplicateOutputPort` |
| D.7 | Parameter types valid | Type (enum) | No runtime error |
| D.8 | Parameter defaults type-compatible | `cluster.rs::validate_cluster_definition` | `ExpandError::ParameterDefaultTypeMismatch`, `ExpandError::InvalidDeriveKeySlot` |
| D.9 | No duplicate parameter names | `cluster.rs::validate_cluster_definition` | `ExpandError::DuplicateParameter` |
| D.10 | Declared signature compatible with inferred | `cluster.rs::expand` → `validate_declared_signature` | `ExpandError::DeclaredSignatureInvalid` (currently wireability-only) |
| D.11 | Declared wireability ≤ inferred | `cluster.rs::validate_declared_signature` | `ClusterValidationError::WireabilityExceedsInferred` |

**Instantiation-Time (I.*)**

| ID | Rule | Enforcement Locus | Error Type / Notes |
|----|------|-------------------|--------------------|
| I.1 | Wiring from parent edge source to cluster kind is legal | `runtime/validate.rs::enforce_wiring_matrix` | `ValidationError::InvalidEdgeKind` |
| I.2 | Port types match at connection points | `runtime/validate.rs::enforce_types` | `ValidationError::TypeMismatch` |
| I.3 | Required parameters bound or exposed | `cluster.rs::validate_parameter_bindings` / `build_resolved_params` | `ExpandError::MissingRequiredParameter` / `UnresolvedExposedBinding` |
| I.4 | Bound parameter values type-compatible | `cluster.rs::validate_parameter_bindings` | `ExpandError::ParameterBindingTypeMismatch` / `ExposedParameterTypeMismatch` |
| I.5 | Exposed parameters exist in parent | `cluster.rs::validate_parameter_bindings` | `ExpandError::ExposedParameterNotFound` |
| I.6 | Version constraints satisfied | `cluster.rs::expand_with_context` (selector resolution) | `ExpandError::InvalidVersionSelector` / `UnsatisfiedVersionConstraint` / `InvalidAvailableVersion` |
| I.7 | Parameter bindings reference only declared parameters | `cluster.rs::resolve_impl_parameters` / `build_resolved_params` / `validate_parameter_bindings` | `ExpandError::UndeclaredParameter` |

**Expansion-Time (E.*)**

| ID | Rule | Enforcement Locus | Error Type / Notes |
|----|------|-------------------|--------------------|
| E.1 | Output contains only primitives | Type (`ExpandedNode` uses `ImplementationInstance`) | No runtime error |
| E.2 | Placeholder edges rewritten to node-to-node edges | `cluster.rs::redirect_placeholder_edges` | Verified by tests; no error type |
| E.3 | `ExternalInput` not an edge sink | `cluster.rs::expand` debug assertion | Assertion only |
| E.4 | Authoring path preserved | `cluster.rs::expand_with_context` | Verified by tests; no error type |
| E.5 | Empty clusters rejected | `cluster.rs::expand_with_context` | `ExpandError::EmptyCluster` |
| E.6 | Definitions not mutated | Clone semantics | No runtime error |
| E.7 | Expanded graph retains boundary ports and resolved parameters for later phases | `ExpandedGraph` / `ExpandedNode` data contract | No runtime error |
| E.8 | Deterministic runtime IDs | `cluster.rs::expand_with_context` (sorted keys) | Verified by tests; no error type |
| E.9 | Referenced nested clusters exist | `cluster.rs::expand_with_context` (`NodeKind::Cluster` load) | `ExpandError::MissingCluster` |

### Computed Defaults: `derive_key`

Cluster parameter defaults may use `derive_key` to compute a deterministic key from the cluster instantiation authoring path:

```yaml
parameters:
  - name: state_key
    type: String
    default:
      derive_key: has_fired
```

Behavior:

- `derive_key` is resolved at expansion time in `build_resolved_params(...)` during nested cluster instantiation.
- The resolved value is a `ParameterValue::String` using a length-prefixed (UTF-8 byte length), namespaced encoding (`__ergo/...`).
- Different instantiation paths produce different keys; same slot names at the same path produce the same key (intentional aliasing).
- Explicit parameter bindings override `derive_key` defaults.
- `derive_key` is only valid on `ParameterType::String` parameters (D.8).
- Empty `slot_name` is rejected (D.8, `ExpandError::InvalidDeriveKeySlot`).
- `slot_name` must be non-empty. It is not identifier-validated; the injective encoding handles reserved characters safely.
- Root-cluster parameter defaults are not resolved through `build_resolved_params`; `derive_key` is for nested cluster instantiation only.

**Validation-Time (V.*)**

| ID | Rule | Enforcement Locus | Error Type / Notes |
|----|------|-------------------|--------------------|
| V.1 | No cycles in graph | `runtime/validate.rs::topological_sort` | `ValidationError::CycleDetected` |
| V.2 | Edges satisfy wiring matrix (including Action gate/payload input refinement) | `runtime/validate.rs::enforce_wiring_matrix` | `ValidationError::InvalidEdgeKind` |
| V.3 | Required inputs connected | `runtime/validate.rs::enforce_required_inputs` | `ValidationError::MissingRequiredInput` |
| V.4 | Type constraints satisfied at edges | `runtime/validate.rs::enforce_types` | `ValidationError::TypeMismatch` |
| V.5 | Actions gated by triggers | `runtime/validate.rs::enforce_action_gating` | `ValidationError::ActionNotGated` |
| V.6 | All nodes pass validation before any action executes | `runtime::validate()` before `execute()` | Structural; no dedicated error |
| V.7 | Each input has ≤1 inbound edge | `runtime/validate.rs::enforce_single_edge_per_input` | `ValidationError::MultipleInboundEdges` |
| V.8 | Referenced primitive implementations exist in catalog | `runtime/validate.rs::validate` (catalog lookup per node) | `ValidationError::MissingPrimitive` |

---

**Action input split refinement (COMP-9):** `V.2` validation inspects destination Action input
types to distinguish Trigger-gated `event` inputs from scalar payload inputs (`number|series|bool|string`)
that may be wired from Source/Compute. Scalar payload inputs do not satisfy `V.5` trigger gating.

## 7. Expansion Algorithm

### 7.1 Full Expansion

```
expand(cluster_def) -> ExpandedGraph:
    graph = initialize_graph()
    
    for node in cluster_def.nodes:
        if node.kind is Impl:
            resolved_version = resolve_primitive_version(node.impl_id, node.version)
            resolved_bindings = resolve_impl_parameters(...)
            graph.add_implementation(
                impl_id = node.impl_id,
                requested_version = node.version,
                version = resolved_version,
                parameters = resolved_bindings,
            )
        else if node.kind is Cluster:
            resolved_version = resolve_cluster_version(node.cluster_id, node.version)
            nested_def = load_cluster(node.cluster_id, resolved_version)
            validate_parameter_bindings(nested_def, node.parameter_bindings)
            nested_params = build_resolved_params(...)  # includes defaults + derive_key
            nested_graph = expand(nested_def, nested_params)  # Recursive
            graph.inline(node.id, nested_graph)
    
    for edge in cluster_def.edges:
        graph.add_edge(resolve(edge))
    
    return graph
```

### 7.2 Node Identity Preservation

After expansion, preserve authoring-level identity for debugging:

```
ExpandedNode {
    runtime_id: UniqueId,
    authoring_path: List<(ClusterId, NodeId)>,
    implementation: ImplementationInstance,
    parameters: Map<String, ParameterValue>,
}
```

The `authoring_path` traces back through the cluster hierarchy.

### 7.3 Expansion Output Invariant

The expansion process may introduce internal representation types for
implementation convenience.

However, the **expanded graph is still a pre-validation artifact**.

The expansion output currently contains:

- Graph topology (nodes and edges)
- Implementation identity (`impl_id`, requested selector, resolved version)
- Resolved parameter values
- Boundary inputs and boundary outputs
- Authoring trace information (`authoring_path` or equivalent)

The expansion output MUST NOT contain:

- Resolved types or manifests
- Execution behavior
- Validation results
- Inferred properties (including BoundaryKind, wireability, or flags)
- Any other semantic or behavioral information

Signature inference, validation, and execution still happen in later phases. The expanded graph
may carry the resolved parameters and retained boundary metadata those later phases need, but it
does not cache validation outcomes or inferred cluster semantics.

---

## 8. Signature Hash (Design-Only, Not Implemented)

The scheme below documents a possible signature-hash design for future breaking-change detection.
Current prod runtime/loader/host code does not compute or enforce it.

### 8.1 Hash Computation

```
signature_hash(sig: Signature) -> Hash:
    canonical = canonicalize(sig)
    return hash(serialize(canonical))

canonicalize(sig):
    return {
        kind: sig.kind,
        inputs: sort_by_name(sig.inputs).map(canonicalize_port),
        outputs: sort_by_name(sig.outputs).map(canonicalize_port),
        has_side_effects: sig.has_side_effects,
        is_origin: sig.is_origin,
    }

canonicalize_port(port):
    return {
        name: port.name,
        ty: port.ty,
        cardinality: port.cardinality,
        wireable: port.wireable,
    }
```

### 8.2 Breaking Change Detection

```
is_breaking_change(old_sig, new_sig) -> bool:
    return signature_hash(old_sig) != signature_hash(new_sig)
```

Changes that modify the hash:

- Adding/removing/renaming ports
- Changing port types or cardinality
- Changing wireability
- Changing boundary kind
- Changing side effect or origin flags

---

## 9. Edge Cases

### 9.1 Cluster with Only Actions

A cluster containing only actions (e.g., "Emergency Exit"):

```
Cluster:
    inputs: [trigger: Event]
    internal: [action: ExitAll(trigger)]
    outputs: []  # or only non-wireable ack
```

Inference:

- `has_wireable_outputs = false`
- `BoundaryKind = ActionLike`
- `has_side_effects = true`

Valid. Can only be wired from TriggerLike outputs.

### 9.2 Cluster with Trigger Output and Internal Action

A cluster that emits an event AND executes an action:

```
Cluster:
    inputs: []
    internal:
        [source] → [compute] → [trigger] → [action]
                                    ↓
                            (exposed as output)
    outputs: [signal: Event (wireable)]
```

Inference:

- `has_wireable_outputs = true`
- `has_wireable_event_out = true`
- `BoundaryKind = TriggerLike`
- `has_side_effects = true`

Valid. The cluster behaves as a Trigger (produces wireable events) but also has side effects.

### 9.3 Empty Cluster

A cluster with no nodes:

Invalid at definition time. Clusters must contain at least one node.

### 9.4 Cluster with No Outputs

A cluster with inputs but no outputs:

```
Cluster:
    inputs: [data: Number]
    internal: [compute: LogValue(data)]  # logs but produces no output
    outputs: []
```

Inference:

- `has_wireable_outputs = false`
- `BoundaryKind = ActionLike`
- `has_side_effects = false` (if LogValue is pure)

This is unusual but valid. The cluster is terminal from a wiring perspective.

---

## 10. Implementation Notes

### 10.1 Rust IR Representation

```rust
enum BoundaryKind {
    SourceLike,
    ComputeLike,
    TriggerLike,
    ActionLike,
}

struct Signature {
    kind: BoundaryKind,
    inputs: Vec<PortSpec>,
    outputs: Vec<PortSpec>,
    has_side_effects: bool,
    is_origin: bool,
}

struct PortSpec {
    name: String,
    ty: ValueType,
    cardinality: Cardinality,
    wireable: bool,
}
```

### 10.2 Validation Performance

Signature inference requires expansion, which can be expensive for deeply nested clusters.

Optimization: cache inferred signatures by (cluster_id, version). Signatures are immutable once computed.

Cache invalidation must consider transitive dependencies (nested cluster versions, implementation manifest versions) and ontology version. Incorrect caching is a compliance violation.

Ontology version is implementation-defined in v0; recommended sources include the crate/build version or a hash of the ontology + execution model bundle used at compile time.

---

## Authority

This document specifies cluster mechanics.

It implements `concepts.md` and is subordinate to `ontology.md`.
