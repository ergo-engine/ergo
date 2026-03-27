---
Authority: CONTRACTS
Version: v0
Last Updated: 2026-03-26
Derived From: crates/kernel/runtime/src/runtime/tests.rs::hello_world_graph_executes_with_core_catalog_and_registries
Scope: Low-level ExpandedGraph and ExecutionReport contract
Change Rule: Review required
---

# UI ↔ Runtime Contract

This document defines the low-level graph shape a UI or other external tool
must emit if it talks directly to `ergo-runtime`.

Higher-level product callers should normally prefer host or SDK entrypoints
instead of constructing `ExpandedGraph` directly.

## Trust Boundary Notice

**Client implementations are non-canonical integrations, not contract
authorities.**

- **Authority:** Runtime contract authority is Rust types in
  `crates/kernel/runtime/src/cluster.rs`,
  `crates/kernel/runtime/src/runtime/types.rs`, and this document.
- **Clients:** CLI and SDK layers delegate canonical execution to host entrypoints.
- **Validation:** All low-level contract enforcement happens in runtime validation
  and execution.

## 1. Data Shape

### ExpandedGraph

```rust
pub struct ExpandedGraph {
    pub nodes: HashMap<String, ExpandedNode>,
    pub edges: Vec<ExpandedEdge>,
    pub boundary_inputs: Vec<InputPortSpec>,
    pub boundary_outputs: Vec<OutputPortSpec>,
}
```

`boundary_inputs` and `boundary_outputs` are retained for signature inference.
Runtime execution does not allow `ExternalInput` endpoints to survive validation.

### ExpandedNode

```rust
pub struct ExpandedNode {
    pub runtime_id: String,
    pub authoring_path: Vec<(String, NodeId)>,
    pub implementation: ImplementationInstance,
    pub parameters: HashMap<String, ParameterValue>,
}
```

On the direct runtime path, `ExpandedGraph.nodes` map keys and
`ExpandedEndpoint::NodePort.node_id` remain the authoritative node identity;
validation copies the map key into validated runtime IDs.

### ImplementationInstance

```rust
pub struct ImplementationInstance {
    pub impl_id: String,
    pub requested_version: String,
    pub version: String,
}
```

### ExpandedEdge

```rust
pub struct ExpandedEdge {
    pub from: ExpandedEndpoint,
    pub to: ExpandedEndpoint,
}

pub enum ExpandedEndpoint {
    NodePort { node_id: String, port_name: String },
    ExternalInput { name: String },
}
```

### ParameterValue

```rust
pub enum ParameterValue {
    Int(i64),
    Number(f64),
    Bool(bool),
    String(String),
    Enum(String),
}
```

### OutputPortSpec

```rust
pub struct OutputPortSpec {
    pub name: String,
    pub maps_to: OutputRef,
}

pub struct OutputRef {
    pub node_id: String,
    pub port_name: String,
}
```

## 2. Execution Flow

### Step 1: Construct `ExpandedGraph`

The caller assembles:

- a node map whose keys are the canonical node IDs referenced by edges
- implementation references (`impl_id`, `requested_version`, and resolved `version`)
- literal parameter values
- edges between node ports
- boundary outputs naming which node ports to observe

### Step 2: Validate

```rust
let validated = validate(&expanded, &catalog)?;
```

Validation checks primitive existence, wiring legality, required inputs, edge
type compatibility, action gating, and other graph-shape rules.

### Step 3: Run

```rust
let report = run(&expanded, &catalog, &registries, &ctx)?;
```

`run()` performs validation internally and then executes the validated graph.

### Step 4: Read Outputs And Effects

```rust
pub struct ExecutionReport {
    pub outputs: HashMap<String, RuntimeValue>,
    pub effects: Vec<ActionEffect>,
}

pub enum RuntimeValue {
    Number(f64),
    Series(Vec<f64>),
    Bool(bool),
    Event(RuntimeEvent),
    String(String),
}
```

`outputs` are keyed by the names declared in `boundary_outputs`. `effects`
contains emitted runtime action effects for callers that care about side-effect
intent.

## 3. Metadata Requirement For Intent Effects

Low-level callers that execute graphs directly through `ergo-runtime` must
provide execution metadata whenever an action manifest declares
`effects.intents`.

- `execute(&graph, &registries, &ctx)` rejects such graphs with
  `ExecError::IntentMetadataRequired { node }`.
- `execute_with_metadata(&graph, &registries, &ctx, graph_id, event_id)` is the
  low-level lane that supplies the stable identifiers required to derive
  deterministic `intent_id` values.
- Host and SDK entrypoints already own this metadata and are the preferred
  production surfaces.

## 4. Non-Goals

The UI or external caller:

- does not define primitive semantics
- does not replace runtime validation
- does not become the canonical orchestration surface
- does not widen host or SDK product guarantees

## 5. Reference Hello-World Graph

```text
number_source(value=3.0) -> gt:a
number_source(value=1.0) -> gt:b
gt.result -> emit_if_true.input
emit_if_true.event -> ack_action.event
```

Result:

- boundary output can expose `act:outcome`
- runtime may also emit action effects through `ExecutionReport.effects`
