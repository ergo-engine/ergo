---
Authority: STABLE
Version: v0
Last Updated: 2025-12-22
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

BoundaryKind is inferred from the full expanded graph, not from the declared or exposed boundary alone. Hiding outputs cannot coerce one kind into another.

The wiring matrix for clusters is identical to the wiring matrix for primitives.

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

Clusters are validated at three points:

### 6.1 Definition Time

When a cluster is saved:
- Internal graph is well-formed (DAG, wiring rules)
- Boundary signature is inferred
- Declared signature (if any) is compatible with inferred
- All ports reference valid internal nodes/outputs

### 6.2 Instantiation Time

When a cluster is placed in a parent context:
- Boundary kind is compatible with wiring context
- Parameter bindings are complete (or explicitly re-exposed)
- Version constraints are satisfied

### 6.3 Expansion Time

Before execution:
- All clusters are recursively expanded to primitives
- Full unified DAG is validated (cycles, types, all nodes present)
- Global "validate all nodes before any action executes" check

---

## 7. Versioning

Clusters are versioned artifacts.

### 7.1 Version Pinning

By default, cluster instances reference a specific version:
```
cluster_id@1.2.0
```

Floating references (`@latest`, `@^1`) are opt-in and require explicit user choice.

### 7.2 Breaking Change Detection

A **signature hash** is computed from:
- Input port names and types
- Output port names, types, and wireability
- Parameter interface (names, types, required/optional)
- Boundary flags (terminal, origin)

If the signature hash changes between versions, the change is breaking.

Breaking changes:
- Block automatic upgrades
- Require explicit user action
- Are surfaced by tooling with upgrade assistance

---

## 8. Expansion Algorithm

Before execution, all clusters must be expanded to primitives.

### 8.1 Expansion Process

```
expand(graph):
    for each node in graph:
        if node is ClusterInstance:
            subgraph = load_cluster_definition(node.cluster_id, node.version)
            subgraph = apply_parameter_bindings(subgraph, node.parameters)
            subgraph = expand(subgraph)  # recursive
            graph = inline_subgraph(graph, node, subgraph)
    return graph
```

### 8.2 Node Identity

After expansion, each primitive node has a unique identity derived from:
- Original cluster path (for debugging/tracing)
- Local node ID within the cluster

This allows error messages and traces to reference the authoring structure, even though it no longer exists at runtime.

---

## 9. Two-Phase Type Enforcement

Type enforcement operates at two levels:

### 9.1 IR Validation (UI/Build Time)

For user-authored clusters:
- Boundary kind compatibility checked against wiring matrix
- Port type compatibility checked at each connection
- Cycles, missing nodes, and invalid wiring rejected

This validation is fast and provides immediate feedback.

### 9.2 Rust DSL (Compile Time, Optional)

For power users writing strategies in Rust:
- Boundary kinds can be encoded as marker types
- Wiring legality enforced by trait bounds
- Invalid connections are compile errors

This provides "can't even write it" safety for those who want it.

Both phases enforce the same invariants. They differ only in when enforcement occurs.

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
- Cluster storage format (JSON, YAML, binary)
- Cluster registry or sharing infrastructure
- Collaboration workflows
- Specific parameter widget types

These may vary across implementations without affecting the authoring layer contract.

---

## Authority

This document specifies the authoring layer.

It is subordinate to `ontology.md`, `execution_model.md`, and `V0_FREEZE.md`.

AUTHORING_LAYER.md defines intent and constraints. CLUSTER_SPEC.md defines the executable interpretation. In case of ambiguity, CLUSTER_SPEC.md governs.

The authoring layer may evolve, but must always satisfy:
> All constructs compile away before execution.
