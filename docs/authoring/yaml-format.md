---
Authority: STABLE
Version: v1
Last Updated: 2026-03-27
Owner: Documentation
Scope: YAML graph authoring format and loader-facing decode contract
Change Rule: Additive only
---

# YAML Graph Format — Specification

**Status:** STABLE — implemented in `ergo-loader` decode/discovery; canonically consumed via `ergo-host` path APIs (`run_graph_from_paths`, `replay_graph_from_paths`, `validate_run_graph_from_paths`, `prepare_hosted_runner_from_paths`) with clients delegating
**Scope:** How `ClusterDefinition` maps to YAML for hand-authoring and tooling
**Litmus test:** Demo 1 graph (15 nodes, 16 edges, 4 boundary outputs)
**Current CLI contract:** `ergo run <graph.yaml> (-f|--fixture <events.jsonl> | --driver-cmd <program> [--driver-arg <value> ...]) [-a|--adapter <adapter.yaml>] [--egress-config <egress.toml>] [-o|--capture|--capture-output <path>] [-p|--pretty-capture] [--cluster-path|--search-path <path> ...]` and `ergo fixture run <events.jsonl> [-o|--capture|--capture-output <path>] [-p|--pretty-capture]` (current flag names retain legacy `driver` terminology)
**Related ingress guidance:** [ingress-channel-guide](ingress-channel-guide.md) documents `HostedEvent`, process-channel protocol, and fixture/process authoring expectations.
**Future work:** Workspace-level project discovery now exists through
`ergo-loader` + `ergo-sdk-rust` and `ergo init` now scaffolds the
project layout. Optional CLI project-mode convenience commands remain a
future extension.

---

## 1. Design Decision Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Node implementation reference | `impl: id@selector` packed string | Concise, readable, mirrors `ImplementationInstance`; selector may be strict semver or a semver constraint |
| Cluster node reference | `cluster: id@selector` | Discriminator field determines `NodeKind::Impl` vs `NodeKind::Cluster`; selector may be strict semver or a semver constraint |
| Parameter bindings | Inferred — scalar = literal, `{ exposed: name }` = exposed | Avoids wrapping every constant in `{ literal: ... }` |
| Edge representation | String shorthand preferred, structured objects supported | `node.port -> node.port` is dramatically more scannable; both deser to same `Edge` |
| Boundary outputs | Map style — `name: node.port` | Clean, reads like what it means |
| Boundary inputs | List-of-objects (for reusable clusters only) | Matches `InputPortSpec` struct; rarely hand-authored |
| Top-level kind | `kind: cluster` always | Every graph file is a `ClusterDefinition`; a top-level graph is a cluster with no parent |

---

## 2. Canonical Example (Demo 1)

```yaml
kind: cluster
id: demo_1
version: "0.1.0"

nodes:
  src_left_a:
    impl: number_source@0.1.0
    params:
      value: 4.0

  src_left_b:
    impl: number_source@0.1.0
    params:
      value: 2.0

  src_right_a:
    impl: number_source@0.1.0
    params:
      value: 1.0

  src_right_b:
    impl: number_source@0.1.0
    params:
      value: 1.0

  src_ctx_x:
    impl: context_number_source@0.1.0

  add_left:
    impl: add@0.1.0

  add_right:
    impl: add@0.1.0

  add_right_ctx:
    impl: add@0.1.0

  add_total:
    impl: add@0.1.0

  gt_a:
    impl: gt@0.1.0

  gt_b:
    impl: gt@0.1.0

  emit_a:
    impl: emit_if_true@0.1.0

  emit_b:
    impl: emit_if_true@0.1.0

  act_a:
    impl: ack_action@0.1.0
    params:
      accept: true

  act_b:
    impl: ack_action@0.1.0
    params:
      accept: true

edges:
  - src_left_a.value -> add_left.a
  - src_left_b.value -> add_left.b
  - src_right_a.value -> add_right.a
  - src_right_b.value -> add_right.b
  - add_left.result -> add_total.a
  - add_right.result -> add_total.b
  - add_right.result -> add_right_ctx.a
  - src_ctx_x.value -> add_right_ctx.b
  - add_left.result -> gt_a.a
  - add_right_ctx.result -> gt_a.b
  - add_right_ctx.result -> gt_b.a
  - add_left.result -> gt_b.b
  - gt_a.result -> emit_a.input
  - gt_b.result -> emit_b.input
  - emit_a.event -> act_a.event
  - emit_b.event -> act_b.event

outputs:
  sum_left: add_left.result
  sum_total: add_total.result
  action_a_outcome: act_a.outcome
  action_b_outcome: act_b.outcome
```

---

## 3. Top-Level Structure

```yaml
kind: cluster              # Required. Always "cluster".
id: <string>               # Required. Cluster identifier.
version: "<semver>"         # Required. Quoting recommended (see §8.1).

nodes: { ... }             # Required. Map of node_id -> node definition.
edges: [ ... ]             # Required. List of edges (shorthand or structured).
outputs: { ... }           # Optional. Boundary output map. Default: empty.

# Optional — only used for reusable clusters with exposed interfaces:
inputs: [ ... ]            # Input port specs. Default: empty.
parameters: [ ... ]        # Cluster-level parameter specs. Default: empty.
declared_signature: { ... } # Optional declared signature. Omit to rely on inference only; current prod validation is wireability-only.
```

Every graph file is a `ClusterDefinition`. A top-level executable graph is simply a cluster with no parent, no input ports, and no exposed parameters. The `expand()` function handles both cases identically.

---

## 4. Node Definitions

### 4.1 Primitive Implementation Nodes

```yaml
nodes:
  <node_id>:
    impl: <impl_id>@<version_or_constraint>
    params:                    # Optional. Omit if no parameters.
      <param_name>: <value>    # Scalar = literal binding
      <param_name>: { exposed: <parent_param> }  # Exposed binding
```

The `impl` field uses `id@selector` packed format. The selector may be a strict semver (`1.2.3`) or a semver constraint (`^1.2`). Expansion resolves the highest satisfying available version and preserves both the requested selector and the resolved concrete version. Presence of `impl` determines `NodeKind::Impl`.

### 4.2 Cluster Reference Nodes

```yaml
nodes:
  <node_id>:
    cluster: <cluster_id>@<version_or_constraint>
    params:
      <param_name>: <value>
```

Presence of `cluster` determines `NodeKind::Cluster`. The selector may be a strict semver or a semver constraint; expansion resolves the highest satisfying discovered cluster version. A node must have exactly one of `impl` or `cluster`, never both.

### 4.3 Parameter Binding Inference

Parameter values are inferred by type:

| YAML value | Inferred binding |
|------------|-----------------|
| Scalar (number, bool, string) | `ParameterBinding::Literal { value }` |
| Object with `exposed` key | `ParameterBinding::Exposed { parent_param }` |

This means:

- `value: 4.0` → literal Number
- `enabled: true` → literal Bool
- `label: "hello"` → literal String
- `threshold: { exposed: parent_threshold }` → exposed binding

You never write `{ literal: 4.0 }`. Scalars are always literals.

### 4.4 String vs Enum Handling

`ParameterValue` has both `String` and `Enum` variants, but both are plain strings in YAML. For untyped node parameter bindings, the loader resolves YAML strings as `ParameterValue::String` because it has no catalog access. For typed cluster parameter defaults, the loader can use the locally declared `ParameterType`, so `type: Enum` defaults are decoded as `ParameterValue::Enum`.

This means decode is intentionally asymmetric:

- untyped graph bindings stay stringly typed until later catalog-backed phases
- typed cluster defaults can already materialize `Enum` values during decode

---

## 5. Edges

### 5.1 String Shorthand (preferred for hand-authoring)

```yaml
edges:
  - src_left_a.value -> add_left.a
  - add_left.result -> gt_a.a
```

Format: `<from_node>.<from_port> -> <to_node>.<to_port>`

The parser splits on `->` with optional surrounding whitespace and `.` (first dot only, splitting into node and port).

#### External Input References

For reusable clusters with input ports, the `$` prefix denotes an external input:

```yaml
edges:
  - $threshold -> gt_a.b
```

Format: `$<input_name> -> <to_node>.<to_port>`

At decode time, this becomes an `Edge` whose source is stored as an `OutputRef { node_id: name, port_name: name }` plus external-input bookkeeping. Expansion may materialize `ExpandedEndpoint::ExternalInput { name }`, and runtime validation rejects any surviving external-input endpoints before execution.

**Validation rules for external input edges:**

1. External inputs can only appear as edge sources, never as targets.
2. Every `$name` in the edge list must match a declared `inputs[].name`. A `$` reference to an undeclared input is a decode error.
3. After expansion, no `ExpandedEndpoint::ExternalInput` may survive into runtime execution. Validation rejects any surviving external-input endpoints before the graph becomes executable.

### 5.2 Structured Format (for tooling / codegen)

```yaml
edges:
  - from: { node: src_left_a, port: value }
    to:   { node: add_left, port: a }

  # External input (structured):
  - from: { external: threshold }
    to:   { node: gt_a, port: b }
```

### 5.3 Dual Support

Both formats deserialize to the same `Edge` struct. The Raw layer uses a serde untagged enum — try string parse first, fall back to structured. Both can be mixed in the same file, though in practice you'd pick one style.

---

## 6. Boundary Ports

### 6.1 Outputs (map style)

```yaml
outputs:
  sum_left: add_left.result
  action_a_outcome: act_a.outcome
```

Format: `<output_name>: <node_id>.<port_name>`

Maps to `OutputPortSpec { name, maps_to: OutputRef { node_id, port_name } }`.

### 6.2 Inputs (for reusable clusters only)

```yaml
inputs:
  - name: threshold
    type: number
    required: true
```

Each input declaration maps to `InputPortSpec { name, maps_to: GraphInputPlaceholder { name, ty, required } }`, where `maps_to.name` equals the declared input `name`. The `GraphInputPlaceholder` carries the type and requirement constraint for the external input; the `name` field is the same string in both the port spec and the placeholder.

The wiring from the external input to internal nodes is expressed via edges using the `$` prefix (see §5.1):

```yaml
edges:
  - $threshold -> gt_a.b
```

Omit entirely for top-level graphs. Defaults to empty list.

### 6.3 Parameters (for reusable clusters only)

```yaml
parameters:
  - name: multiplier
    type: Number
    default: 1.0
    required: false
```

Maps to `ParameterSpec { name, ty, default, required }`.

Omit entirely for top-level graphs. Defaults to empty list.

---

## 7. Cluster File Resolution

When a node references `cluster: pricing_engine@0.2.0` or `cluster: pricing_engine@^0.2`, the loader/discovery path must resolve candidate files and expansion must resolve the selector against the discovered versions.

**Convention:** Cluster files live in the same directory as the parent graph file, or in a declared search path. Resolution order:

1. Same directory as the referencing file: `./pricing_engine.yaml`
2. Named subdirectory: `./clusters/pricing_engine.yaml`
3. Explicit search paths (if provided via CLI flag or config)

File naming convention: `<cluster_id>.yaml`. Each discovered file's `version` field must be strict semver. Exact selectors match that concrete version; constraint selectors resolve to the highest satisfying discovered version.

Path-backed failures can surface in more than one place:

- discovery may fail with `LoaderError::Discovery(...)` before expansion if the cluster tree cannot be assembled
- expansion fails with I.6 selector errors if the requested version selector is invalid or no discovered version satisfies it
- exact version loads still surface `ExpandError::MissingCluster` when a resolved nested cluster cannot be loaded

**Note:** This convention is sufficient for v0. A registry or package manager for clusters is a v1 concern.

---

## 8. Parser Constraints

### 8.1 Version Quoting

YAML interprets bare `1.0` as a float, not a string. Versions like `0.1.0` survive because YAML can't parse three-dot numbers, but two-segment versions will silently become floats.

**Rule:** The parser must accept both string and numeric YAML values for version fields and coerce to string. The spec recommends quoting versions (`version: "0.1.0"`) but does not require it.

Implementation: The Raw struct's version field should use a custom deserializer or `serde_yaml::Value` that handles both types.

### 8.2 Identifier Constraints

The `node.port` and `id@selector` shorthand formats impose constraints on identifiers:

| Character | Forbidden in |
|-----------|-------------|
| `.` (dot) | General identifiers and port names |
| `@` (at) | General identifiers |
| `$` (dollar) | General identifiers that participate in graph identity or parameter/input naming; reserved for external-input edge shorthand |
| `/` (slash) | General identifiers |
| `\` (backslash) | General identifiers |
| `:` (colon) | General identifiers |
| ` ` (space) | All identifiers |

**Rule:** The loader validates node IDs, cluster IDs, implementation IDs, parameter/input names, and port names against these constraints and produces clear decode errors on violation. Two related name classes matter here:

- internal `node.port` endpoint-reference port segments use the narrower parser rule and reject only empty strings, `.` and spaces
- declared signature boundary port names, plus the broader graph-identifier surfaces, use the general identifier rule and additionally reject `@`, `/`, `\`, `:`, and, for most graph identifiers, `$`

### 8.3 Parsing Strategy

The parser operates without catalog access. It produces a `ClusterDefinition` from YAML alone. Signature inference and semantic validation happen later during cluster expansion and runtime validation.

This means the parser cannot:

- Validate that an `impl_id` exists in the catalog
- Distinguish String vs Enum for untyped node parameter bindings
- Check port names against primitive manifests

Loader-facing failures surface as `LoaderError::{Io, Decode, Discovery}`. Those errors do not implement `ErrorInfo` or carry invariant IDs; rule-tagged semantic failures begin after the loader boundary.

---

## 9. Mapping to ClusterDefinition

| YAML field | ClusterDefinition field |
|------------|------------------------|
| `id` | `id` |
| `version` | `version` |
| `nodes` (map) | `nodes: HashMap<NodeId, NodeInstance>` |
| `nodes.<id>.impl` | `NodeKind::Impl { impl_id, version }` |
| `nodes.<id>.cluster` | `NodeKind::Cluster { cluster_id, version }` |
| `nodes.<id>.params` | `parameter_bindings: HashMap<String, ParameterBinding>` |
| `edges` | `edges: Vec<Edge>` |
| `outputs` | `output_ports: Vec<OutputPortSpec>` |
| `inputs` | `input_ports: Vec<InputPortSpec>` (wiring via `$` edges) |
| `parameters` | `parameters: Vec<ParameterSpec>` |
| `declared_signature` | `declared_signature: Option<Signature>` |

---

## 10. Existing Conventions (from primitive manifests)

The CLI already parses YAML manifests with these patterns, and the graph parser should maintain consistency:

- Top-level `kind` discriminator field
- Lowercase type names in port specs: `number`, `event`, `bool`, `series`, `string`
- Uppercase type names in parameter specs: `Number`, `Int`, `Bool`, `String`, `Enum`
- `Raw*` bridge structs handle deserialization → domain conversion
- Loader decode/discovery errors stay on the `LoaderError::{Io, Decode, Discovery}` surface; later host/runtime semantic errors may implement `ErrorInfo`

---

## 11. Full Pipeline

```
graph.yaml
    ↓  (parse)
ClusterDefinition
    ↓  (expand, with ClusterLoader + PrimitiveCatalog)
ExpandedGraph          ← flat DAG, X.9 enforced
    ↓  (validate, with PrimitiveCatalog)
ValidatedGraph         ← wiring matrix, types, action gating checked
    ↓  (execute, with Registries + ExecutionContext)
ExecutionReport        ← boundary outputs + emitted effects
    ↓  (format)
CLI output
```

This is the complete path from authored file to visible results.
