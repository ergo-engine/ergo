# Authoring Overview

> **Navigation aid only.** Authoritative content lives in the linked documents.

---

## Cluster Concepts

A **cluster** is a named, bounded subgraph that can be treated as a single node externally.

### Key Properties

- Contains primitives and/or other clusters (arbitrary nesting)
- Exposes boundary ports (inputs, outputs, parameters)
- Compiles away before execution
- Saveable, reusable, shareable, versionable

**Source:** [AUTHORING_LAYER.md](../STABLE/AUTHORING_LAYER.md) §2

---

## Boundary Kinds

Every cluster has a BoundaryKind inferred from its signature:

| BoundaryKind | Meaning |
|--------------|---------|
| SourceLike | No inputs, produces values |
| ComputeLike | Values in, values out |
| TriggerLike | Produces wireable events |
| ActionLike | Terminal, no wireable outputs |

The wiring matrix applies to clusters exactly as it applies to primitives.

**Source:** [AUTHORING_LAYER.md](../STABLE/AUTHORING_LAYER.md) §4.2

---

## Expansion Algorithm

```
expand(graph):
    for each node in graph:
        if node is ClusterInstance:
            subgraph = load_cluster_definition(...)
            subgraph = apply_parameter_bindings(...)
            subgraph = expand(subgraph)  # recursive
            graph = inline_subgraph(...)
    return graph
```

**Source:** [CLUSTER_SPEC.md](../STABLE/CLUSTER_SPEC.md) §7

---

## Validation Timing

1. **Definition Time** — When cluster is saved
2. **Instantiation Time** — When placed in parent context
3. **Expansion Time** — Before execution

**Source:** [AUTHORING_LAYER.md](../STABLE/AUTHORING_LAYER.md) §6

---

## See Also

- [Semantics](semantics.md) — Execution rules
- [CLUSTER_SPEC.md](../STABLE/CLUSTER_SPEC.md) — Full data structure specification
