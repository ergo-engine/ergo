---
Authority: PROJECT
Date: 2026-03-22
Author: Sebastian (Architect) + Codex
Status: Historical
---

# In-memory transport — implementation closure plan

Historical design-loop artifact. Implemented closure is recorded in the
[closed delivery ledger](/Users/sebastian/Projects/ergo/docs/ledger/dev-work/closed/in-memory-loader-phase-1.md).
Deferred adjacent lanes remain tracked in the
[open defer ledger](/Users/sebastian/Projects/ergo/docs/ledger/gap-work/open/in-memory-loader-adjacent-defers.md).

This document closed the scope and boundary decisions for the implemented
in-memory transport work in Ergo.

It is not a product-vision essay and it is not a manifesto for “one world”
types. It records what this phase would do, what it would not do, and which
repo invariants it must preserve.

---

## North star

Ergo should eventually support both filesystem-backed and in-memory-backed
authoring flows as first-class experiences.

“First-class” does **not** mean every transport-facing public type becomes the
same type immediately. It means:

- the same semantic graph/cluster model
- the same host-owned orchestration and lifecycle truth
- no fake paths, hidden second execution model, or transport-specific hacks
- a product surface that treats in-memory as a real capability, not an escape hatch

The job of this phase is to move toward that destination **without lying
about the current boundaries**.

---

## Doctrine baseline

This phase must preserve the current architectural rules:

- clients do not own canonical orchestration
- host owns expansion, runtime setup, and execution lifecycle
- replay remains a distinct host lane
- loader owns transport/discovery concerns
- kernel/supervisor/runtime remain untouched

This plan is written to stay compatible with:

- `docs/invariants/00-cross-phase.md`
- `docs/invariants/07-orchestration.md`
- `docs/invariants/08-replay.md`
- `docs/system/kernel.md`
- `docs/system/kernel-prod-separation.md`

If a later phase wants to widen doctrine, that should be an explicit follow-on
decision, not an accidental consequence of this phase.

---

## Implementation Scope

### In scope

- loader support for in-memory graph/cluster transport
- host-owned live preparation work for:
  - canonical run preparation reuse behind existing `*_from_paths` entrypoints
  - lower-level canonical validation preparation
  - lower-level canonical manual-runner preparation
- live preparation only:
  - this phase does **not** add a new lower-level object-based live execution
    seam
  - existing lower-level `run_graph(...)` / `run_graph_with_control(...)` remain
    path-shaped and are **not** reinterpreted as supported in-memory execution
    entrypoints
- preserving the current canonical client surface:
  - existing host `*_from_paths` entrypoints stay canonical
  - existing public path-based host request structs remain unchanged
  - any new object-based host seam is additive and lower-level
  - canonical run preparation support in this phase may be internal/shared host
    prep reused by existing path entrypoints, not a new public prep API
- preserving current host lifecycle truth:
  - adapter-required stage separation
  - eager egress startup
  - finalization ordering
  - current public error buckets
  - validation remains non-session, non-egress-starting prep
- additive in-memory transport carriers and loader/host translation
- public-surface and docs updates needed to keep the transport boundary honest

### Explicitly deferred

- in-memory project/profile product surface
- new canonical SDK in-memory APIs
- new CLI in-memory product surfaces
- replay lower-level in-memory preparation
- DOT/rendering lower-level in-memory preparation
- manifest/composition lower-level in-memory preparation
- adapter object/string transport into live host prep
- new lower-level object-based live execution APIs
- render default output naming changes
- demo-fixture capture naming changes
- fixture-path ingress changes for live execution
- fixture inspect/validate reporting changes

These deferred adjacent lanes are recorded explicitly in
[/Users/sebastian/Projects/ergo/docs/ledger/gap-work/open/in-memory-loader-adjacent-defers.md](/Users/sebastian/Projects/ergo/docs/ledger/gap-work/open/in-memory-loader-adjacent-defers.md)
so the delivered implementation does not silently half-implement them.

### Why this split

This is the smallest scope that:

- adds real in-memory capability
- preserves current doctrine
- does not invent a second execution model
- does not force a project/profile redesign and host-lane campaign all at once

The destination can still include in-memory project/profile DX later. This
phase simply refuses to pretend that project/profile semantics are already
transport-neutral when they are not.

---

## Decision 1: Project/profile destination yes, project/profile deferred now

### Chosen

- **Destination candidate:** in-memory project/profile support may become a
  future product direction
- **This phase:** project/profile support is **deferred**

### Why

Current project/profile behavior is not just a transport wrapper. It already
contains real filesystem-backed product semantics:

- project root discovery
- relative graph/cluster/adapter/egress path resolution
- exactly-one-ingress profile resolution
- path-shaped capture output semantics
- path identity surfaced in SDK outputs like `ProjectSummary.root`

Treating in-memory project/profile as “in scope right now” would turn this
phase into a much larger product redesign. That may become a legitimate
future project, but it is not honest to smuggle it into a graph/cluster transport
refactor.

### Consequence

This phase may add in-memory support below the project/profile layer, but it
does **not** change:

- `Ergo::from_project(...)`
- `run_profile(...)`
- `run_profile_with_stop(...)`
- `replay_profile(...)`
- `validate_project()`
- `runner_for_profile(...)`

Those remain filesystem-backed until a separate product decision defines an
honest in-memory project model.

---

## Decision 2: Option A — parallel transport carriers with honest names

### Chosen

- keep a filesystem-facing loader carrier
- add a parallel in-memory loader carrier
- rename the filesystem-facing carrier so the names tell the truth

### Why

`LoadedGraphBundle` is a loader transport artifact, not the semantic center of
Ergo. Its current `PathBuf`-typed fields honestly describe filesystem source
identity. Forcing that public transport artifact to become transport-neutral
too early would flatten a real difference at the wrong layer.

Parallel carriers let us:

- keep filesystem contracts truthful
- add in-memory capability without fake symmetry
- preserve a clean migration window before publication
- unify the real semantic layers underneath

### Naming rule

Do not leave the filesystem type with the neutral/default-sounding name while
giving in-memory a “variant” name. Use explicit peers, for example:

- `FilesystemGraphBundle`
- `InMemoryGraphBundle`

Neutral names should be reserved for genuinely transport-neutral concepts.

### Consequence

This is a real public API migration inside the repo. It is acceptable because
the crate is not yet published on crates.io, but it is **not** zero-cost. The
repo already has direct consumers, tests, and docs for the current public
loader surface.

---

## Decision 2A: Split-carrier lower-level seam

### Chosen

- loader owns the lower-level sealed asset carrier
- host owns lower-level runtime-prep options
- host reexports the loader-owned asset carrier in its lower-level block for
  this phase

### Why

This is the shipped split-carrier compromise in the delivered implementation:

- loader owns transport/discovery artifacts
- host owns runtime-prep configuration
- host owns the lower-level orchestration entrypoints
- SDK/CLI do not get to decide where that boundary moves

Decision 7 does not dissolve this question. “SDK calls host only” is an SDK
wiring rule. It does not erase the fact that the host lower-level API is now
semver-coupled to a loader-owned sealed asset payload.

### Consequence

The lower-level host seam takes a loader-owned `PreparedGraphAssets` plus
host-owned `LivePrepOptions`. Host reexports the loader asset type rather than
mirroring or reconstructing it.

---

## Decision 3: Prep-only phase; future object-based live run must stay explicit

### Chosen

- this phase adds **no** new lower-level object-based live execution seam
- if a later phase adds one, `capture_output` must be explicit if file writing
  is requested
- no implicit default capture filename for that future seam in its first phase

### Why

Current default capture naming is a **host-owned output contract** derived from
`graph_path.file_stem()`. In-memory graphs do not have a truthful file stem.

Using `graph.id` as an implicit replacement sounds ergonomic, but it silently
changes a user-visible output policy and pretends content identity is the same
thing as output-path policy. That is too much hidden behavior for a
correctness-first first implementation phase.

### Consequence

- this phase stays prep-only in the sense that it adds no new one-shot
  object-based live run API and avoids silently widening host execution scope
- bundle-first/manual-runner flows stay honest: lower-level in-memory
  manual-runner prep may still yield a live `HostedRunner` for caller-driven
  stepping plus canonical `finalize_hosted_runner_capture(...)`, but that is
  the existing manual-runner lane rather than a new direct-run seam
- file-writing from any later object-based live seam requires explicit output path
- if implicit naming is desired later, add an explicit host-owned naming field
  as a new contract

This decision only covers graph-run capture naming. DOT/render and demo-fixture
naming remain deferred with their own transport-specific output rules.

---

## Decision 4: No SDK in-memory surface in this phase

### Chosen

- **no new SDK in-memory API in this phase**

### Why

Under current doctrine, the SDK is thin over the existing host client
entrypoints for canonical run, replay, validation, and manual stepping.

Adding a first-class SDK in-memory lane is not a harmless consequence of host
changes. It would be a doctrine and product-surface expansion. If we want that
later, we should make that choice explicitly after the lower-level transport
shape is proven.

### Consequence

This phase adds lower-level loader/host capability only. SDK remains on the
current canonical client surface.

This keeps the plan doctrine-safe and avoids pretending the guardrails already
bless a new SDK lane.

---

## Decision 5: Option A — keep filesystem-facing public loader surfaces honest

### Chosen

- keep existing path-shaped public loader surfaces where they are genuinely
  filesystem contracts
- add in-memory siblings or parallel carriers where an in-memory public surface
  is actually needed

### Why

Not every public loader surface is the same kind of thing:

- `ClusterDiscovery.cluster_sources` is a discovery result
- `LoaderIoError.path` is a typed error payload
- `resolve_cluster_candidates(...)` is a filesystem-facing lookup API with
  lexical-path behavior

Flattening all of them to `SourceRef` would erase truthful filesystem-specific
contracts in the name of symmetry. That is the wrong kind of unification.

### Consequence

We unify internals and semantics where it helps correctness, but we do **not**
force every transport-facing public loader API into one abstract shape.

“Stable” here means stable in transport truth and compatibility obligations, not
frozen spellings forever. Decision 2 already makes one explicit public rename to
remove a misleading neutral/default name.

Any new in-memory public discovery/load sibling must also preserve the current
public compatibility facts that matter here:

- definition/conflict recording still happens before version filtering
- non-matching versions may still appear in public discovery/load outputs where
  they do today
- public output ordering is treated as its own compatibility surface, not
  assumed to fall out of resolver search order
- `discover_cluster_tree(...)` and `ClusterDiscovery` compatibility are part of
  the surface, not just `load_cluster_tree(...)`
- `load_cluster_tree(...)` remains part of the compatibility surface, not an
  ignored side path

---

## Decision 6: Option A — caller-ordered deterministic precedence

### Chosen

- in-memory inputs have explicit deterministic order at the API boundary
- resolver precedence follows that caller-provided order
- duplicate-definition conflicts remain discovery-time and root-reachable
- no global duplicate rejection at resolver construction time
- resolver/public candidate carriers preserve the metadata needed for parity:
  - referrer-scoped lookup context
  - per-found opened/lexical label surface
  - searched-scope trace suitable for current diagnostics

### Why

Current loader behavior is order-sensitive and publicly observable:

- candidate search order is deterministic
- the first parse/ID/conflict failure surfaced depends on that order
- the first definition recorded for a semantic key depends on that order
- diagnostics expose searched paths and opened-file context

That is not just “diagnostic quality.” It is compatibility surface.

The dependency graph matters **after** a candidate is resolved. It does not
replace candidate-selection order.

Canonical source identity alone is not sufficient to preserve current behavior.
Filesystem parity also depends on lexical/referrer metadata and opened-label
reporting.

### Consequence

The in-memory resolver must preserve the current sequencing discipline:

1. choose candidates in deterministic order
2. parse candidate
3. validate ID
4. record definition / detect conflict
5. apply version filter
6. recurse

If we ever want a looser model later, it should be an explicit non-parity
feature, not the default semantics of the canonical in-memory path.

---

## Decision 7: Deferred in this phase

### Chosen

- this phase does **not** add SDK wrappers over the new in-memory loader/host seams
- therefore the SDK loader-helper composition question is deferred

### Why

Current doctrine does not automatically bless:

- SDK use of new lower-level host seams
- new canonical SDK wrappers over new in-memory loader/host seams
- a positive SDK allowlist in the boundary scripts

Current doctrine **does** already allow clients to use public loader APIs rather
than loader internals for parser access. What remains deferred here is whether a
future SDK scope should add wrappers over the new in-memory seams, and if so
whether those wrappers are host-only or also admit an explicit public-loader
allowlist.

So the most conservative move in this phase is to avoid exercising that wrapper
question at all.

### Consequence

If a future scope wants SDK wrappers, it must first decide:

- whether SDK gets any in-memory helper surface at all
- whether those helpers are canonical or advanced
- whether SDK is host-only or may use an explicit loader allowlist
- how `tools/verify_layer_boundaries.sh` changes to enforce that choice

This means SDK deferral in this phase is partly policy and partly current
guardrail posture, not a fully mechanical prohibition. A future SDK scope
must tighten enforcement as well as document intent.

---

## Decision 8: Option B — referrer-sensitive scoping for parity

### Chosen

- in-memory discovery preserves referrer-sensitive lookup semantics

### Why

Current filesystem discovery is context-sensitive:

- the referring source influences candidate search scope and precedence
- different referrers can make a cluster resolvable or missing
- diagnostics expose that searched scope

That means the resolver contract must preserve enough metadata to answer:

- what scope was searched for this referrer
- which candidate was actually opened
- which human-facing label should appear in parse/ID-mismatch diagnostics

So a flat global namespace is not “basically the same thing.” It is a smaller,
different feature.

If the goal is correctness and parity, the in-memory resolver must preserve the
fact that `referring_source` is semantically meaningful.

### Consequence

If we ever want a flat global in-memory mode later, it should be introduced as
an explicit non-parity feature with reduced claims, not as the canonical
transport semantics.

---

## Host seam rules this phase must preserve

Any new lower-level host object seam in this phase must preserve:

- host-owned expansion, dependency scan, adapter preflight/binding, runner
  construction, and lifecycle truth
- validation prep remains a distinct rule within that host truth:
  - canonical validation does not construct a hosted session
  - canonical validation does not start egress channels
- existing `*_from_paths` client entrypoints remain canonical; the new seam is
  additive and lower-level
- existing public path-based host request structs remain stable; any new object
  seam introduces sibling request types instead of mutating the current ones
- existing public error buckets, including:
  - `AdapterRequired`
  - `InvalidInput`
  - `StepFailed`
  - `DriverIo`
  - `Io`
  - replay `Setup`, `GraphIdMismatch`, `ExternalKindsNotRepresentable`
- eager egress startup before work begins
- finalization ordering:
  - `ensure_capture_finalizable`
  - `ensure_no_pending_egress_acks`
  - `stop_egress_channels`
  - `into_capture_bundle`
- host-visible diagnostic label surface for expansion errors
- a `with_surfaces` counterpart, or an explicit deferral of that advanced lane
- current live-prep adapter boundary stays honest in this phase:
  - in-memory graph/cluster transport may still pair with filesystem-backed
    adapter inputs
  - adapter object/string transport is deferred, not half-included

These are not incidental details. They are part of the contract this phase
must not weaken.

---

## Explicit non-goals for this phase

This phase does **not** claim to solve:

- in-memory project root / manifest / profile resolution
- in-memory replay preparation
- in-memory DOT/render preparation
- in-memory manifest/composition preparation
- adapter object/string transport into host live prep
- new lower-level object-based live execution APIs
- supported in-memory use of the existing lower-level `run_graph(...)` /
  `run_graph_with_control(...)` APIs
- render default SVG naming changes
- demo-fixture capture naming changes
- new CLI in-memory product surfaces
- fixture-path ingress changes for live execution
- fixture inspect/validate reporting changes
- a new canonical SDK in-memory lane
- flat-global in-memory scoping semantics
- fake `mem://` paths through existing `*_from_paths` APIs

---

## Adjacent public-surface work

Adjacent issues that should be handled explicitly, not treated as vague cleanup:

- `decode_graph_json(...)` was already a broken public export at audit time and
  is included in this phase's decode fix rather than deferred
- label-aware YAML decode was a missing truthful public surface at audit time
  and is included in this phase via `decode_graph_yaml_labeled(...)`
- broader public loader compatibility coverage should name the actual surfaces
  in play, including `discover_cluster_tree(...)`, `ClusterDiscovery`, and
  `load_cluster_tree(...)`, not only the currently exercised subset of loader
  API tests

These are not the transport seam itself, but they are too real as public or
diagnostic surfaces to leave in “maybe later” limbo while touching the same
area of the codebase. Each should be either included in this phase or
explicitly deferred in the work ledger.

---

## Phase 0 freeze outputs

Phase 0 is complete only when the following are treated as frozen inputs to
implementation, not revisited mid-code.

### 0A. Public naming freeze

#### Rename

- `LoadedGraphBundle` -> `FilesystemGraphBundle`

#### Add

- `InMemoryGraphBundle`
- `InMemoryClusterDiscovery`
- `decode_graph_yaml_labeled(...)`
- `discover_in_memory_cluster_tree(...)`
- `load_in_memory_graph_sources(...)`

#### Preserve

- `load_graph_sources(...)`
- `decode_graph_yaml(...)`
- `decode_graph_json(...)`
  - Phase 1 fixes or explicitly defers the implementation/export story; no new
    public JSON decode name is frozen in Phase 0
- `parse_graph_file(...)`
- `resolve_cluster_candidates(...)`
- `discover_cluster_tree(...)`
- `load_cluster_tree(...)`
- `LoaderError`, `LoaderIoError`, `LoaderDecodeError`, `LoaderDiscoveryError`

The naming rule is:

- path-backed public artifacts keep truthful filesystem names
- in-memory public artifacts get peer names, not “variant of the real one”
- neutral/default names are reserved for genuinely transport-neutral concepts

The filesystem bundle rename is an intentional public migration inside the repo,
not a compatibility trick. It is acceptable in this phase because the crate
is not yet published, but it should still be treated as a deliberate rename
with explicit docs/tests updates.

### 0A.1 In-memory public loader carrier freeze

The additive public in-memory loader inputs/results for this phase are:

```rust
pub struct InMemorySourceInput {
    pub source_id: String,
    pub source_label: String,
    pub content: String,
}
```

```rust
pub struct InMemoryClusterDiscovery {
    pub root: DecodedAuthoringGraph,
    pub clusters: HashMap<(String, Version), DecodedAuthoringGraph>,
    pub cluster_source_ids: HashMap<(String, Version), String>,
    pub cluster_source_labels: HashMap<(String, Version), String>,
    pub cluster_diagnostic_labels: HashMap<(String, Version), String>,
}
```

```rust
pub struct InMemoryGraphBundle {
    pub root: DecodedAuthoringGraph,
    pub discovered_source_ids: Vec<String>,
    pub source_map: BTreeMap<String, String>,
    pub source_labels: BTreeMap<String, String>,
}
```

Contract:

- `source_id` is the public logical source-identity and lookup-path surface
- callers should use path-like `source_id` values such as `graphs/root.yaml`
  because referrer-sensitive lookup and search-trace reporting are derived from
  logical path structure
- logical paths are loader-defined, platform-independent, and use `/` separators
- `source_id` and `search_roots` must be relative logical paths; rooted paths,
  backslashes, `:`, empty path segments, and `.` / `..` segments are rejected
- `source_label` is the public human-facing diagnostic/display surface
- `source_label` is **not** semantic identity
- unique `source_label` in the current in-memory API is a deliberate diagnostic requirement so
  user-facing errors remain unambiguous, not a statement that labels define
  source identity
- cluster lookup still follows the existing filename-style contract on
  `source_id`; a resolvable logical source id must end in `<cluster_id>.yaml`
- `discover_in_memory_cluster_tree(...)` and `load_in_memory_graph_sources(...)`
  take ordered inputs such as `&[InMemorySourceInput]`
- both functions also take explicit logical `search_roots: &[String]`
  representing the referrer-sensitive in-memory lookup scope for the call
- `discover_in_memory_cluster_tree(...)` parses the root internally from
  `root_source_id` and returns it in `InMemoryClusterDiscovery.root`
- candidate precedence follows caller order
- `InMemoryGraphBundle.discovered_source_ids` order is explicit:
  lexicographic `source_id` order matching `source_map.keys()` order, not
  resolver precedence order

The public in-memory carrier does **not** expose loader-internal `SourceRef`.
That remains an internal discovery implementation detail.

### 0B. Split-carrier freeze

The lower-level live-prep seam uses two distinct carriers.

Loader owns the sealed asset carrier:

```rust
pub struct PreparedGraphAssets {
    root: DecodedAuthoringGraph,
    clusters: HashMap<(String, Version), DecodedAuthoringGraph>,
    cluster_diagnostic_labels: HashMap<(String, Version), String>,
    pub(crate) _sealed: (),
}
```

Host owns the runtime-prep options:

```rust
pub struct LivePrepOptions {
    pub adapter_path: Option<PathBuf>,
    pub egress_config: Option<EgressConfig>,
}
```

Interpretation:

- `PreparedGraphAssets` is pure loader discovery output
- only loader code constructs `PreparedGraphAssets`
- host reexports `PreparedGraphAssets`, but host does not mirror or reconstruct it
- `PreparedGraphAssets` is externally immutable; callers read it through
  accessor methods rather than mutating the loader-produced payload in place
- `cluster_diagnostic_labels` is the host-visible diagnostic label surface used
  for expansion/configuration error reporting
- loader owns and normalizes that label surface for both transports:
  filesystem asset loading emits path/opened-path labels, and in-memory asset
  loading emits caller-provided `source_label`
- `LivePrepOptions` is pure host runtime-prep configuration
- `adapter_path` stays path-shaped in this phase because adapter
  object/string transport is deferred
- `egress_config` stays object-shaped exactly as it already is on
  `PrepareHostedRunnerFromPathsRequest`

This keeps loader transport truth and host runtime-prep truth separate.

### 0C. Split-carrier fields explicitly out of scope

`PreparedGraphAssets` does **not** carry:

- source text maps
- discovered-file lists
- capture output policy
- driver/ingress configuration
- adapter manifest contents or adapter object transport
- replay bundles or replay integrity metadata
- runtime-produced objects such as `HostedRunner`, `RuntimeHandle`,
  `dependency_summary`, or provenance values

`LivePrepOptions` does **not** carry:

- graph text or discovered-source artifacts
- runtime surfaces
- loader-owned transport identity

`RuntimeSurfaces` remains a parallel advanced lane via `_with_surfaces` APIs.

### 0D. Public API delta freeze

#### Existing path-shaped host APIs that remain unchanged

- `RunGraphFromPathsRequest`
- `ReplayGraphFromPathsRequest`
- `PrepareHostedRunnerFromPathsRequest`
- `run_graph_from_paths(...)`
- `run_graph_from_paths_with_control(...)`
- `run_graph_from_paths_with_surfaces(...)`
- `run_graph_from_paths_with_surfaces_and_control(...)`
- `validate_graph_from_paths(...)`
- `validate_graph_from_paths_with_surfaces(...)`
- `prepare_hosted_runner_from_paths(...)`
- `prepare_hosted_runner_from_paths_with_surfaces(...)`
- `replay_graph_from_paths(...)`
- `replay_graph_from_paths_with_surfaces(...)`
- lower-level `run_graph(...)` / `run_graph_with_control(...)`

#### New lower-level host APIs this phase may add

- `load_graph_assets_from_paths(...)`
- `load_graph_assets_from_memory(...)`
- `validate_graph(...)` over `&PreparedGraphAssets` + `&LivePrepOptions`
- `validate_graph_with_surfaces(...)` over `&PreparedGraphAssets` + `&LivePrepOptions`
- `prepare_hosted_runner(...)`
- `prepare_hosted_runner_with_surfaces(...)`

These are the only public lower-level object-based host additions allowed by the
scope. Canonical run support is internal/shared prep reused behind existing
`*_from_paths` entrypoints, not a new public prep API.

### 0E. Phase 1 start gate

Phase 1 may begin only after the following are accepted as frozen:

- the filesystem/in-memory bundle names
- the labeled decode entrypoint names
- the split `PreparedGraphAssets` / `LivePrepOptions` carrier names and fields
- the exact no-change list for existing path-based host/client APIs
- the exact allowed new lower-level host API names for validation/manual-runner
- the explicit Phase 1 disposition rule for `decode_graph_json(...)`

If any of these are still in motion, the implementation is not done with Phase 0.

---

## Execution phases for this implementation

The implementation order should be:

### Phase 0: Freeze the public naming and boundary decisions

- rename the current path-backed loader bundle to an honest filesystem name
- choose the in-memory peer name
- define the lower-level split-carrier shape
- confirm that current path-based host request types remain unchanged and that
  new object-based seams, if any, are sibling APIs

This phase exists to stop later code work from dragging boundary decisions back
into implementation.

### Phase 1: Fix the adjacent public decode seams first

- add a truthful label-aware public YAML decode entrypoint
- resolve the already-broken `decode_graph_json(...)` public export path
- keep existing filesystem-facing decode helpers stable where they are still
  honest filesystem contracts

This is the smallest public-surface cleanup that unblocks truthful in-memory
diagnostics before discovery work begins.

### Phase 2: Introduce resolver/carrier internals without changing canonical host APIs

- add the internal resolver/candidate carrier shape needed for:
  - caller-ordered precedence
  - referrer-sensitive scoping
  - opened-label diagnostics
  - searched-scope trace
- preserve current sequencing:
  - choose candidate
  - parse
  - validate ID
  - record/detect conflict
  - apply version filter
  - recurse
- preserve current filesystem resolver behavior via lexical/referrer metadata,
  not just canonical identity

At the end of this phase, the loader internals should be transport-ready even
if the public in-memory carrier is not exposed yet.

### Phase 3: Add parallel loader transport carriers

- rename the filesystem-facing bundle/type(s)
- add the in-memory carrier peer
- keep filesystem-facing public loader APIs honest
- tighten the existing filesystem discovery/load helper pair so the root is
  parsed internally from `root_path` and returned as part of `ClusterDiscovery`
  rather than accepted as a separately paired parsed input
- add only the frozen in-memory public siblings this phase actually needs:
  - `discover_in_memory_cluster_tree(...)`
  - `load_in_memory_graph_sources(...)`
- both frozen in-memory public functions take:
  - ordered `&[InMemorySourceInput]`
  - explicit logical `search_roots: &[String]`
- preserve public compatibility facts for discovery/load outputs:
  - pre-filter conflict recording
  - non-matching version retention where it exists today
  - ordering as an explicit public surface
  - `discover_cluster_tree(...)` / `ClusterDiscovery` compatibility, including
    the internally parsed `root`
  - `load_cluster_tree(...)` compatibility

Phase 3 does **not** add:

- `load_in_memory_cluster_tree(...)`
- any in-memory sibling for `resolve_cluster_candidates(...)`
- any public alias preserving `LoadedGraphBundle`

This is where the loader becomes explicitly dual-transport without pretending
both transports are the same artifact.

### Phase 4: Add the host live-preparation support needed by this phase

- add loader-owned sealed `PreparedGraphAssets`
- add host-owned `LivePrepOptions`
- host wraps loader asset loading so lower-level callers can stay on the host
  crate for this lane, even though the loader lower-level surface remains public
- add host preparation support for:
  - lower-level `load_graph_assets_from_paths(...)`
  - lower-level `load_graph_assets_from_memory(...)`
  - lower-level canonical validation prep over `&PreparedGraphAssets`
  - lower-level canonical manual-runner prep over `PreparedGraphAssets` +
    `&LivePrepOptions`
  - canonical run prep reuse behind existing `*_from_paths` entrypoints
- preserve:
  - adapter-required preflight
  - dependency scan / adapter setup
  - eager egress startup behavior for live/manual-runner prep only
  - validation remains non-session and non-egress-starting
  - finalization ordering
  - current public error buckets
  - `with_surfaces` parity or explicit deferral
  - the existing lower-level `run_graph(...)` / `run_graph_with_control(...)`
    APIs stay execution-only and path-independent; they are not reinterpreted
    as an object-based run bridge

This phase is prep-only. It does **not** add a new object-based live execution
API.

### Phase 5: Prove parity and keep defers explicit

- add parity tests for filesystem lookup ordering, referrer-sensitive scope, and
  diagnostics
- add in-memory tests that prove the chosen parity claims
- add compatibility coverage for affected public loader surfaces, not just the
  existing narrow test subset
- update docs and any boundary/guardrail checks required by the delivered scope
- explicitly record any deferred adjacent items in the work ledger

This phase is where the work earns the claim that it is correctness-first,
not just architecture-first.

---

## Completion bar for the implementation

This implementation is not complete until all of the following are true:

- the chosen loader/host carrier boundary is implemented
- existing filesystem behavior remains truthful and test-covered
- compatibility coverage explicitly includes the full public surface affected by
  the scope, not only the currently narrow loader API test subset
- host live-preparation support is delivered for every in-scope lane:
  - canonical run prep reuse behind existing `*_from_paths` entrypoints
  - lower-level canonical validation prep
  - lower-level canonical manual-runner prep
- no new supported bridge exists from the scope work into path-shaped
  `run_graph(...)` / `run_graph_with_control(...)` execution
- validation remains non-session and non-egress-starting
- host lifecycle/error contracts are preserved, including `with_surfaces`
  parity or an explicit recorded deferral
- docs and doctrine are updated to match the delivered scope
- deferred lanes remain explicitly deferred, not silently half-implemented

That is the bulletproof version of the plan: correctness first, DX preserved,
and no boundary lies.
