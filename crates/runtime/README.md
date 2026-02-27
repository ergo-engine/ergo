# Primitive Library

This crate is the ergo runtime plus its bundled primitive library (the standard deterministic compute/trigger/action/source primitives that run inside it). The runtime defines the ontology and execution physics; the primitive library is the content executed within it.

Primitives are deterministic, manifest-defined units of computation, triggering, and action. They are enforced by runtime contracts and are the single source of truth for execution.

Terminology:
- Runtime: ontology + execution physics (validation, execution engine, wiring rules)
- Primitive library: stdlib of primitives packaged inside this runtime crate

This repository is:
- ontology-first
- deterministic by construction
- intentionally boring

It is NOT:
- a product
- a UI
- a strategy builder
- an orchestrator

Downstream systems depend on this library.
This library depends on nothing downstream.

## Payload Contract (v0)

### Context Value Access

Sources may read values from `ExecutionContext` via the `value(key)` method. The context is populated from event payloads by the adapter layer.

### Behavior

| Condition | Behavior |
|-----------|----------|
| Key exists, correct type | Returns value |
| Key missing | Returns type-specific default (`0.0` for numbers, `false` for bools) |
| Key exists, wrong type | Returns type-specific default (`0.0` for numbers, `false` for bools) |

### Supported Types

| Type | Default | Access Pattern |
|------|---------|----------------|
| Number (f64) | `0.0` | `ctx.value(key).and_then(\|v\| v.as_number())` |
| Bool | `false` | `ctx.value(key).and_then(\|v\| v.as_bool())` |

### Payload Hydration Path

```
ExternalEvent::with_payload()
    → context_from_payload()
    → payload_values() [JSON → Value]
    → RuntimeExecutionContext::from_values()
    → Source::produce(&params, &ctx)
```

### Key Naming

Context keys are strings. Current implementations:
- `context_number_source`: reads key from parameter `key` (default `"x"`)
- `context_bool_source`: reads key from parameter `key` (default `"x"`)

### Determinism

All default behaviors are deterministic. Missing or malformed payload data produces consistent outputs across replay.

## Core stdlib wiring (34 implementations)

- **Sources (5):** `number_source`, `boolean_source`, `string_source`, `context_number_source`, `context_bool_source`
- **Computes (22):** `const_number`, `const_bool`, `add`, `subtract`, `multiply`, `divide`, `safe_divide`, `abs`, `negate`, `gt`, `gte`, `lt`, `lte`, `eq`, `neq`, `min`, `max`, `and`, `or`, `not`, `select`, `select_bool`
- **Trigger (2):** `emit_if_true`, `emit_if_event_and_true`
- **Actions (5):** `ack_action`, `annotate_action`, `context_set_number`, `context_set_bool`, `context_set_string`

Helpers:
- `catalog::build_core_catalog()` builds a `PrimitiveCatalog` for validation/inference
- `catalog::core_registries()` registers all stdlib implementations into runtime registries

## Hello world graph (reference)

This graph compares two static numbers, emits an event if `a > b`, and acknowledges it.

```
number_source(value=3.0) -> gt:a
number_source(value=1.0) -> gt:b
gt.result -> emit_if_true.input
emit_if_true.event -> ack_action.event
```

Sketch in Rust:

```rust
use primitive_library::catalog::{build_core_catalog, core_registries};
use primitive_library::cluster::{ExpandedEndpoint, ExpandedGraph, ExpandedNode, OutputPortSpec, OutputRef};
use primitive_library::runtime::{run, types::{ExecutionContext, Registries}};

let expanded = ExpandedGraph { /* nodes + edges per diagram above */ };
let catalog = build_core_catalog();
let regs = core_registries().unwrap();
let registries = Registries { sources: &regs.sources, computes: &regs.computes, triggers: &regs.triggers, actions: &regs.actions };
let ctx = ExecutionContext::default();
let report = run(&expanded, &catalog, &registries, &ctx)?;
```

The reference graph is also exercised in `runtime/tests.rs::hello_world_graph_executes_with_core_catalog_and_registries`.

## Golden Spike Tests

Two integration tests serve as canonical reference paths:

| Test | Location | Path |
|------|----------|------|
| `hello_world_graph_executes_with_core_catalog_and_registries` | `crates/runtime/src/runtime/tests.rs` | Direct: `runtime::run()` |
| `supervisor_with_real_runtime_executes_hello_world` | `crates/supervisor/tests/integration.rs` | Orchestrated: `Supervisor::new()` → `RuntimeHandle` → `runtime::run()` |

If either test fails, the execution path is broken.
