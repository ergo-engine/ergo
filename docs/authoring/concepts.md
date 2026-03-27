---
Authority: STABLE
Version: v0
Last Updated: 2026-03-26
Scope: Cluster concepts, fractal composition, boundary kinds
Change Rule: Additive only
---

# Authoring Layer

This document defines the authoring layer of the system.
It specifies how users compose strategies from primitives without violating ontological constraints.

The authoring layer is explicitly **not frozen**. It may evolve independently of the runtime ontology.

---

## 1. Relationship to Runtime Ontology

The runtime ontology defines four primitives: Source, Compute, Trigger, Action.

The authoring layer provides tools for composing these primitives into reusable, nestable structures called **clusters**.

**The invariant:**

> All authoring constructs compile away before execution.
> The runtime sees only the four primitives and their wiring rules.

This includes all execution-relevant semantics. No authoring construct may influence runtime behavior except via the expanded DAG.

This invariant is frozen. The authoring layer itself is not.

---

## 2. Clusters

A **cluster** is a named, bounded subgraph that can be treated as a single node from the outside.

Clusters:

- Contain primitives and/or other clusters (arbitrary nesting)
- Expose boundary ports (inputs and outputs)
- May have configurable parameters
- Are saveable, reusable, shareable, and versionable
- Compile away before execution

Clusters are the primary abstraction for modularity and reuse.

---

## 3. Fractal Composition

The authoring layer supports **fractal composition**:

- At any zoom level, the user sees nodes and wires
- Zooming into a cluster reveals its internal structure
- Zooming out collapses internal structure into a single node
- This nesting is arbitrarily deep

At every level, the same visual language applies: nodes, ports, wires.

At execution time, all levels flatten into a single unified DAG.

---

## 4. Cluster Boundaries

Every cluster has a **boundary** that determines how it interacts with its environment.

A boundary consists of:

- **Input ports** — data/events the cluster consumes
- **Output ports** — data/events the cluster produces
- **Parameters** — static configuration values
- **Flags** — metadata about the cluster's behavior

### 4.1 Boundary Ports

Ports are the only way to "peek inside" a cluster.

- An input port maps to an internal graph input placeholder
- An output port maps to a specific (node_id, output_name) inside the cluster
- Ports must be explicitly declared; no implicit access to internals

### 4.2 Boundary Kind

Every cluster has a **BoundaryKind** that determines where it can be wired.

There are exactly four boundary kinds, mirroring the four primitives:

| BoundaryKind | Meaning |
|--------------|---------|
| SourceLike | No inputs, produces values |
| ComputeLike | Values in, values out |
| TriggerLike | Produces wireable events |
| ActionLike | Terminal, no wireable outputs |

BoundaryKind is **inferred** from the cluster's boundary signature, never declared independently.

BoundaryKind is inferred from the cluster's declared boundary ports, resolved against the expanded internal graph. Which outputs are exposed in `output_ports` directly affects the inferred kind — a cluster that hides all wireable outputs will infer as `ActionLike`. See cluster-spec.md §3 for the full algorithm.

The wiring contract for clusters mirrors the primitive wiring contract.
For `ActionLike` inputs, the primitive contract includes a per-port refinement:

- `event` inputs are gating inputs and must be wired from `TriggerLike`
- scalar inputs (`number | bool | string`) are payload inputs and may be wired from `SourceLike` or `ComputeLike`

This refines the coarse kind matrix; it does not introduce a fifth boundary kind.

### 4.3 Boundary Flags

Flags capture properties orthogonal to BoundaryKind:

- `has_side_effects` — true if the cluster contains any Action
- `is_origin` — true if the cluster has no inputs and all roots are Sources

These flags inform execution semantics and audit, but do not affect wiring legality.

---

## 5. Parameters

Clusters may expose **parameters** — static configuration values set at instantiation time.

Parameters:

- Are typed (int, number, bool, string, enum)
- May have defaults
- May be bound to a literal value
- May be re-exposed to the parent context (parameter threading)

Parameter threading allows arbitrary-depth exposure:

- Cluster A contains Cluster B
- Cluster B exposes `threshold`
- Cluster A may bind it: `threshold = 0.5`
- Or re-expose it: `threshold := parent.threshold`

All parameters must be bound to concrete values before expansion.

Parameter exposure is an authoring-time construct only; expansion requires all parameters to be concretely bound. Partial expansion with unresolved parameters is not permitted.

---

## 6. Validation Timing

Current prod enforcement is split across multiple stages rather than a
single authoring-time proof pass.

### 6.1 Loader / Decode Time

When graph text is decoded:

- Graph text is parsed into `ClusterDefinition`
- Format-level shorthand/default coercions are applied
- Identifier constraints and declared external-input references are checked

### 6.2 Instantiation Time

When a cluster is placed in a parent context:

- Parameter bindings are complete (or explicitly re-exposed)
- Version constraints are satisfied
- Current prod does not directly validate parent/child wiring against
  inferred `BoundaryKind` here; wiring legality is enforced later on
  primitive edges after expansion

### 6.3 Expansion Time

Before execution:

- All clusters are recursively expanded to primitives
- Parameter bindings, defaults, `derive_key`, and version selectors are resolved
- Declared-signature wireability checks run when `declared_signature`
  is present
- Full unified DAG validation (cycles, wiring, types, primitive
  existence, required inputs) happens on the expanded graph via
  `runtime::validate()`
- Global "validate all nodes before any action executes" check

---

## 7. Versioning

Clusters are versioned artifacts.

### 7.1 Version Pinning

Cluster and implementation references use `id@selector` syntax, where
the selector is either strict semver or a semver constraint:

```
cluster_id@1.2.0
cluster_id@^1.2
```

Selectors resolve to the highest satisfying available version.
`@latest` is not a supported selector.

### 7.2 Breaking Change Detection

Breaking-change detection via signature hashing is specified in
[cluster-spec.md §8](cluster-spec.md#8-signature-hash). This document does not
redefine the hash algorithm or its field set. Current implementation
status should not be inferred from this section.

---

## 8. Expansion Algorithm

Before execution, all clusters must be expanded to primitives.

### 8.1 Expansion Process

```
expand(graph):
    for each node in graph:
        if node is ClusterInstance:
            resolved_version = resolve_cluster_version(node.cluster_id, node.version)
            subgraph = load_cluster_definition(node.cluster_id, resolved_version)
            validate_parameter_bindings(subgraph, node.parameters)
            resolved_params = build_resolved_params(subgraph, node.parameters)
            subgraph = expand(subgraph, resolved_params)  # recursive
            graph = inline_subgraph(graph, node, subgraph)
    return graph
```

### 8.2 Node Identity

After expansion, each primitive node carries two identities:

- A deterministic sequential `runtime_id` used by the runtime
- An `authoring_path` used for debugging/tracing

This allows error messages and traces to reference the authoring
structure, even though it no longer exists at runtime.

---

## 9. Current Type Enforcement

### 9.1 Current Prod Validation Path

Current prod does not ship a separate UI/build-time IR validator that
fully proves DAG, wiring, and type correctness for authored clusters.
Instead, enforcement is split across:

- loader decode
- `validate_cluster_definition()`
- expansion
- `runtime::validate()` on the expanded graph

That split is the real enforcement path today. A compile-time Rust DSL
encoding boundary kinds as marker types is not implemented.

---

## 10. Doctrine

Three rules define the authoring layer's relationship to the ontology:

1. **Expanded DAG is the only executable truth.**
   Everything else is UI/IR sugar.

2. **Ports are the only peek.**
   No implicit access to internals. If it's wireable externally, it must be a declared port.

3. **Declarations constrain, never redefine.**
   Authors can assert interfaces, but the graph must prove them.

---

## 11. What This Document Does Not Define

The following are product-level concerns, not authoring layer concerns:

- Specific UI/UX for the canvas
- Cluster storage format details (see `yaml-format.md` for the YAML specification and `loader.md` for the decode contract)
- Cluster registry or sharing infrastructure
- Collaboration workflows
- Specific parameter widget types

These may vary across implementations without affecting the authoring layer contract.

---

## Authority

This document specifies the authoring layer.

It is subordinate to `ontology.md`, `execution.md`, and `freeze.md`.

concepts.md defines intent and constraints. cluster-spec.md defines the executable interpretation. In case of ambiguity, cluster-spec.md governs.

The authoring layer may evolve, but must always satisfy:
> All constructs compile away before execution.
