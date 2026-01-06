---
Authority: CONTRACTS
Version: v0
Last Updated: 2026-01-05
Derived From: src/runtime/tests.rs::hello_world_graph_executes_with_core_catalog_and_registries
Scope: Data structures UI must emit for runtime
Verified Against Tag: v0.28-kernel-closed
Change Rule: Review required
---

# UI ↔ Runtime Contract

This document defines the exact data structure a UI must emit to drive the Primitive Library runtime.

---

## Trust Boundary Notice

**`crates/ui-authoring` is a Reference Client, not a canonical contract implementation.**

- **Authority:** Runtime contract authority is Rust types in `crates/runtime/src/cluster.rs` + this document
- **TypeScript mirror:** `contractTypes.ts` is a best-effort mirror; not enforcement
- **No guarantees:** Reference UI may have conveniences (e.g., default values, loose typing) that a production client must not rely on
- **Validation:** All contract enforcement happens at runtime; UI-side validation is advisory only

---

## 1. Data Shape (Authoritative)

### ExpandedGraph

The top-level structure the UI must produce.

```rust
pub struct ExpandedGraph {
    pub nodes: HashMap<String, ExpandedNode>,
    pub edges: Vec<ExpandedEdge>,
    pub boundary_inputs: Vec<InputPortSpec>,   // Empty for runtime execution
    pub boundary_outputs: Vec<OutputPortSpec>, // Maps graph outputs to node ports
}
```

### ExpandedNode

Each node in the graph.

```rust
pub struct ExpandedNode {
    pub runtime_id: String,                        // Unique ID within graph
    pub authoring_path: Vec<(String, NodeId)>,     // Empty for flat graphs
    pub implementation: ImplementationInstance,    // Which primitive
    pub parameters: HashMap<String, ParameterValue>, // Literal values
}
```

### ImplementationInstance

Reference to a registered primitive.

```rust
pub struct ImplementationInstance {
    pub impl_id: String,   // e.g., "number_source", "gt", "emit_if_true"
    pub version: String,   // e.g., "0.1.0"
}
```

### ExpandedEdge

Connection between node ports.

```rust
pub struct ExpandedEdge {
    pub from: ExpandedEndpoint,
    pub to: ExpandedEndpoint,
}

pub enum ExpandedEndpoint {
    NodePort { node_id: String, port_name: String },
    ExternalInput { name: String },  // Not allowed in runtime execution
}
```

### ParameterValue

Literal values bound to node parameters.

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

Maps graph boundary outputs to internal node ports.

```rust
pub struct OutputPortSpec {
    pub name: String,       // External name for this output
    pub maps_to: OutputRef, // Which node:port produces it
}

pub struct OutputRef {
    pub node_id: String,
    pub port_name: String,
}
```

---

## 2. Execution Flow

### Step 1: UI Constructs ExpandedGraph

UI assembles:
- Nodes with unique `runtime_id`
- Implementation references (`impl_id` + `version`)
- Parameter literals (no bindings, no expressions)
- Edges connecting `NodePort` → `NodePort`
- Boundary outputs naming which node ports to observe

### Step 2: Backend Calls validate()

```rust
let validated = validate(&expanded, &catalog)?;
```

- Checks all primitives exist in catalog
- Enforces wiring matrix (Source→Compute, Compute→Trigger, etc.)
- Enforces required inputs are connected
- Enforces type compatibility on edges
- Enforces actions are gated by triggers
- Returns `ValidatedGraph` or `ValidationError`

### Step 3: Backend Calls run()

```rust
let report = run(&expanded, &catalog, &registries, &ctx)?;
```

- Executes nodes in topological order
- Sources produce values from parameters
- Computes transform inputs to outputs
- Triggers emit events based on conditions
- Actions execute when triggered
- Returns `ExecutionReport` with boundary outputs

### Step 4: Backend Surfaces Outputs

```rust
pub struct ExecutionReport {
    pub outputs: HashMap<String, RuntimeValue>,
}

pub enum RuntimeValue {
    Number(f64),
    Series(Vec<f64>),
    Bool(bool),
    Event(RuntimeEvent),
    String(String),
}
```

UI reads `report.outputs` by the names declared in `boundary_outputs`.

---

## 3. Explicit Non-Goals

The UI:

- **Does not reason about primitives** — it only references `impl_id` + `version`
- **Does not execute logic** — the runtime does all computation
- **Does not invent semantics** — port names and types come from the catalog
- **Does not validate** — the backend validates; UI may pre-check for UX only

**The UI is a pure graph authoring surface.**

Its sole responsibility is emitting a valid `ExpandedGraph` structure.

---

## Reference: Hello-World Graph

```
number_source(value=3.0) → gt:a
number_source(value=1.0) → gt:b
gt.result → emit_if_true.input
emit_if_true.event → ack_action.event
```

Nodes:

| runtime_id | impl_id       | parameters          |
|------------|---------------|---------------------|
| src_a      | number_source | value: Number(3.0)  |
| src_b      | number_source | value: Number(1.0)  |
| gt1        | gt            | (none)              |
| emit       | emit_if_true  | (none)              |
| act        | ack_action    | accept: Bool(true)  |

Edges:

| from             | to                |
|------------------|-------------------|
| src_a:value      | gt1:a             |
| src_b:value      | gt1:b             |
| gt1:result       | emit:input        |
| emit:event       | act:event         |

Boundary outputs:

| name           | maps_to       |
|----------------|---------------|
| action_outcome | act:outcome   |

Result: `action_outcome = Event(Action(Completed))`

---

*This contract exactly matches the hello-world test. No runtime code was changed.*
