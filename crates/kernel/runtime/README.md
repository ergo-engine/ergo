# Primitive Library

This crate is the Ergo kernel runtime plus its bundled deterministic stdlib of
Source, Compute, Trigger, and Action implementations.

The runtime owns ontology, expansion/validation/execution physics, and the
catalog/registry surfaces used by downstream host and SDK layers. It is not a
product surface by itself.

## Context Access Contract

`ExecutionContext` is a typed key/value store:

```rust
pub struct ExecutionContext {
    values: HashMap<String, Value>,
}
```

`ExecutionContext::value(key)` returns `Option<&Value>`. The context does not
invent defaults on its own.

The bundled context-reading stdlib sources apply deterministic defaults when the
resolved key is missing or has the wrong type:

| Implementation | Value type | Default |
|----------------|------------|---------|
| `context_number_source` | `Number` | `0.0` |
| `context_bool_source` | `Bool` | `false` |
| `context_string_source` | `String` | `""` |
| `context_series_source` | `Series` | `[]` |

All four implementations read the context key from parameter `key`, which
defaults to `"x"`.

## Core stdlib wiring (42 implementations)

- **Sources (7):** `number_source`, `boolean_source`, `string_source`,
  `context_number_source`, `context_bool_source`, `context_string_source`,
  `context_series_source`
- **Computes (27):** `const_number`, `const_bool`, `add`, `subtract`,
  `multiply`, `divide`, `safe_divide`, `abs`, `negate`, `gt`, `gte`, `lt`,
  `lte`, `eq`, `neq`, `min`, `max`, `and`, `or`, `not`, `select`,
  `select_bool`, `append`, `window`, `mean`, `len`, `sum`
- **Triggers (2):** `emit_if_true`, `emit_if_event_and_true`
- **Actions (6):** `ack_action`, `annotate_action`, `context_set_number`,
  `context_set_bool`, `context_set_string`, `context_set_series`

Helpers:

- `catalog::build_core_catalog()` builds the `PrimitiveCatalog`
- `catalog::core_registries()` returns stdlib runtime registries
- `catalog::CatalogBuilder` widens registration without changing runtime
  semantics

## Hello world graph

Reference graph:

```text
number_source(value=3.0) -> gt:a
number_source(value=1.0) -> gt:b
gt.result -> emit_if_true.input
emit_if_true.event -> ack_action.event
```

Sketch in Rust:

```rust
use ergo_runtime::catalog::{build_core_catalog, core_registries};
use ergo_runtime::cluster::ExpandedGraph;
use ergo_runtime::runtime::{run, ExecutionContext, Registries};

let expanded = ExpandedGraph {
    nodes: todo!(),
    edges: todo!(),
    boundary_inputs: vec![],
    boundary_outputs: vec![],
};

let catalog = build_core_catalog();
let core = core_registries().unwrap();
let registries = Registries {
    sources: &core.sources,
    computes: &core.computes,
    triggers: &core.triggers,
    actions: &core.actions,
};
let ctx = ExecutionContext::default();
let report = run(&expanded, &catalog, &registries, &ctx)?;
```

## Golden Spike Tests

Two tests anchor the direct and orchestrated execution paths:

| Test | Location | Path |
|------|----------|------|
| `hello_world_graph_executes_with_core_catalog_and_registries` | `crates/kernel/runtime/src/runtime/tests.rs` | Direct `runtime::run()` path |
| `supervisor_with_real_runtime_executes_hello_world` | `crates/kernel/supervisor/tests/integration.rs` | Orchestrated supervisor path |

If either fails, the execution baseline is broken.
