---
Authority: STABLE
Version: v0.2
Last Updated: 2025-12-22
Scope: Data structures, inference algorithm, validation rules
Change Rule: Additive only
---

# Cluster Specification

---

> **Changelog (v0.2):** Terminology alignment with TERMINOLOGY.md. No semantic changes.
> - `NodeKind::Primitive` → `NodeKind::Impl`
> - `primitive_id` → `impl_id`
> - `PrimitiveInstance` → `ImplementationInstance`
> - `primitive` field → `implementation` field
>
> The term "primitive" now refers exclusively to the four ontological roles (Source, Compute, Trigger, Action). Concrete executable nodes are called "implementations."

This document defines the formal specification for clusters.
It includes data structures, inference algorithms, and validation rules.

This specification implements the authoring layer defined in `AUTHORING_LAYER.md`.

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
    default: Option<ParameterValue>,
    required: bool,
}

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
p.cardinality = inferred from usage or declared
```

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

If a cluster declares an explicit signature, it must be **compatible** with the inferred signature.

Compatibility rules:

```
declared.kind == inferred.kind

declared.inputs ⊆ inferred.inputs  
    (declared may omit optional inputs)

declared.outputs ⊆ inferred.outputs  
    (declared may expose fewer outputs)

declared.has_side_effects == inferred.has_side_effects
    (cannot hide side effects)

declared.is_origin == inferred.is_origin
    (cannot claim origin if it isn't)
```

A declared signature may not mark a port as wireable if the inferred port is non-wireable. Declarations constrain; they cannot grant capabilities.

If declared signature is incompatible, the cluster definition is invalid.

---

## 5. Wiring Matrix

The wiring matrix for clusters mirrors the ontological primitive wiring matrix:

```
SourceLike  → ComputeLike  : allowed
SourceLike  → TriggerLike  : forbidden (v0)
ComputeLike → ComputeLike  : allowed
ComputeLike → TriggerLike  : allowed
ComputeLike → ActionLike   : forbidden (must be mediated by Trigger)
TriggerLike → TriggerLike  : allowed
TriggerLike → ActionLike   : allowed
ActionLike  → *            : forbidden (terminal)
*           → SourceLike   : forbidden (origin)
```

This matrix applies at every nesting level.

---

## 6. Validation Rules

### 6.1 Definition-Time Validation

When a cluster is saved, validate:

1. **Internal DAG structure**
   - Cluster must contain at least one node
   - No cycles
   - All edges reference existing nodes and ports
   - All edges satisfy wiring matrix (including nested cluster boundary kinds)

2. **Port validity**
   - Every output port references a valid internal node output
   - Every input port has a unique name
   - Every output port has a unique name

3. **Parameter validity**
   - All parameters have valid types
   - Defaults are type-compatible
   - No duplicate parameter names

4. **Signature inference succeeds**
   - Algorithm completes without error
   - If declared signature exists, compatibility check passes
   - Declared wireable must not exceed inferred wireability (declared.wireable ⇒ inferred.wireable)

5. **Context independence**
   - Definition-time validation must not depend on parent context. A cluster is valid or invalid independent of where it is instantiated.

### 6.2 Instantiation-Time Validation

When a cluster is placed in a parent context, validate:

1. **Wiring compatibility**
   - Parent edge source's kind allows wiring to this cluster's kind
   - Port types match at connection points

2. **Parameter completeness**
   - All required parameters are either bound or exposed
   - Bound values are type-compatible
   - Exposed parameters exist in parent context

3. **Version compatibility**
   - Requested version exists
   - Version constraints are satisfied

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
the phase invariants in `docs/CANONICAL/PHASE_INVARIANTS.md`.

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
| D.8 | Parameter defaults type-compatible | `cluster.rs::validate_cluster_definition` | `ExpandError::ParameterDefaultTypeMismatch` |
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
| I.6 | Version constraints satisfied | — | **Not implemented** (exact-match only; see `TODO(I.6)`) |
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
| E.7 | Boundary ports retained for inference only | `ExpandedGraph` doc contract | No runtime error |
| E.8 | Deterministic runtime IDs | `cluster.rs::expand_with_context` (sorted keys) | Verified by tests; no error type |
| E.9 | Referenced nested clusters exist | `cluster.rs::expand_with_context` (`NodeKind::Cluster` load) | `ExpandError::MissingCluster` |

**Validation-Time (V.*)**

| ID | Rule | Enforcement Locus | Error Type / Notes |
|----|------|-------------------|--------------------|
| V.1 | No cycles in graph | `runtime/validate.rs::topological_sort` | `ValidationError::CycleDetected` |
| V.2 | Edges satisfy wiring matrix | `runtime/validate.rs::enforce_wiring_matrix` | `ValidationError::InvalidEdgeKind` |
| V.3 | Required inputs connected | `runtime/validate.rs::enforce_required_inputs` | `ValidationError::MissingRequiredInput` |
| V.4 | Type constraints satisfied at edges | `runtime/validate.rs::enforce_types` | `ValidationError::TypeMismatch` |
| V.5 | Actions gated by triggers | `runtime/validate.rs::enforce_action_gating` | `ValidationError::ActionNotGated` |
| V.6 | All nodes pass validation before any action executes | `runtime::validate()` before `execute()` | Structural; no dedicated error |
| V.7 | Each input has ≤1 inbound edge | `runtime/validate.rs::enforce_single_edge_per_input` | `ValidationError::MultipleInboundEdges` |
| V.8 | Referenced primitive implementations exist in catalog | `runtime/validate.rs::validate` (catalog lookup per node) | `ValidationError::MissingPrimitive` |

---

## 7. Expansion Algorithm

### 7.1 Full Expansion

```
expand(cluster_def) -> ExpandedGraph:
    graph = initialize_graph()
    
    for node in cluster_def.nodes:
        if node.kind is Impl:
            graph.add_implementation(node)
        else if node.kind is Cluster:
            nested_def = load_cluster(node.cluster_id, node.version)
            nested_def = apply_bindings(nested_def, node.parameter_bindings)
            nested_graph = expand(nested_def)  # Recursive
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
}
```

The `authoring_path` traces back through the cluster hierarchy.

### 7.3 Expansion Output Invariant

The expansion process may introduce internal representation types for
implementation convenience.

However, the **expanded graph is a structural artifact only**.

The expansion output MUST contain only:
- Graph topology (nodes and edges)
- Implementation identity (`impl_id`, `version`)
- Authoring trace information (`authoring_path` or equivalent)

The expansion output MUST NOT contain:
- Resolved types or manifests
- Side-effect semantics
- Execution behavior
- Validation results
- Inferred properties (including BoundaryKind, wireability, or flags)
- Any other semantic or behavioral information

All semantics beyond identity are introduced strictly in later phases:
signature inference, validation, and execution.

Any expansion output that carries semantic information beyond identity
is non-compliant with this specification.

---

## 8. Signature Hash

The signature hash enables breaking change detection.

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

It implements `AUTHORING_LAYER.md` and is subordinate to `ontology.md`.
