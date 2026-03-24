---
Authority: PROJECT
Date: 2026-03-22
Author: Sebastian (Architect) + Codex
Status: Historical
---

# In-memory loader blast radius map

Historical design-loop artifact. Implemented closure is recorded in the
[closed delivery ledger](/Users/sebastian/Projects/ergo/docs/ledger/dev-work/closed/in-memory-loader-phase-1.md).
Deferred adjacent lanes remain tracked in the
[open defer ledger](/Users/sebastian/Projects/ergo/docs/ledger/gap-work/open/in-memory-loader-adjacent-defers.md).

Revised with Codex-confirmed facts.

This memo remains the **broad surface-area and risk map**:

- it records the larger blast radius
- it preserves alternate paths and rejected pressures
- it should not be read as the authoritative closure record once it diverges
  from the closed ledger

This memo is the wider audit artifact.

---

## Four identity concerns

| Concern | Current mechanism | In-memory equivalent |
|---------|------------------|---------------------|
| Semantic cluster identity | `(id, version)` key in HashMap | Same — unchanged |
| Source identity | Best-effort canonicalized filesystem path (falls back to original path text when canonicalization fails) | `SourceRef` — see contract below |
| Human diagnostics | Path strings in error messages | Caller-provided label per asset |
| Provenance | Content-based hash of expanded graph | Same — already path-independent |

### SourceRef contract

The landed loader keeps `SourceRef` as an **internal** identity carrier.

From current usage patterns, the shipped internal type requires:

- `Clone` — stored in multiple collections
- `Eq + Hash` — used in `HashSet` (cycle detection) and `HashMap` (conflict detection)
- filesystem canonical identity plus lexical/opened path metadata for filesystem sources
- in-memory logical `source_id` plus human-facing `source_label` metadata for in-memory sources

It does **not** need `Display` or `Ord` directly in the landed shape because
opened-label and search-trace diagnostics are carried separately.

Current filesystem paths actually play three distinct roles today:

- **Canonical source identity** — canonicalized path used for dedupe/conflict detection and
  as the source-map/discovered-files identity
- **Lexical/opened source label** — the concrete path string that appears in parse/id-mismatch
  diagnostics ("opened '/path/to/file'")
- **Searched candidate trace** — the ordered list of candidate paths that missing-cluster
  errors enumerate

`SourceRef` only replaces the first role by itself. An in-memory design still needs a way
to preserve lexical/opened labels and ordered search trace for diagnostics; those are not
automatically recovered from an opaque canonical identity token.

This document already distinguishes those roles conceptually. The remaining gap is the
**concrete carrier shape** used to move them through discovery and any additive host seam.
If the transport payload only carries canonical `SourceRef` identity plus raw content, the
current lexical/opened diagnostic surface still cannot be reproduced faithfully.

Current loader behavior also depends on the **lexical referring path**, not just canonical
identity. Relative lookup uses `path.parent()` of the path spelling currently being
visited, while dedupe/conflict bookkeeping uses `canonicalize_or_self(path)`. A resolver
model that preserves only canonical `SourceRef` identity cannot fully reproduce current
alias/symlink behavior; filesystem parity requires either carrying both canonical identity
and lexical referrer metadata, or explicitly documenting that lexical-alias behavior
changes.

**Implementation requirement for FilesystemResolver:** Current lookup depends on
`path.parent()` of the referring source to derive `base_dir` for relative candidate
search. An opaque SourceRef with only the traits above is not enough for the
FilesystemResolver to reconstruct this behavior. The FilesystemResolver must either:

- Store a reverse mapping from SourceRef → PathBuf internally, or
- Use a SourceRef type that carries richer metadata (e.g. wraps a PathBuf for
  filesystem sources while remaining opaque to the builder), or
- Carry enough lexical referrer metadata to recover the current per-visit `base_dir`
  used for relative lookup

The builder and resolver trait see SourceRef as opaque. The FilesystemResolver
implementation knows its SourceRefs are backed by paths. This is an implementation
detail of the filesystem resolver, not a trait-level requirement.

---

## The loader crate: `crates/prod/core/loader/src/`

### decode/yaml_graph.rs — NEEDS LABEL-AWARE PUBLIC SEAM

| Function | Filesystem? | Status |
|----------|-------------|--------|
| `parse_graph_file(path)` | YES — `fs::read_to_string` | Stays for filesystem path |
| `decode_graph_yaml(input)` | NO — in-memory | **Insufficient** — hardcodes `"<memory>"` as the only source label |
| `parse_graph_str(input, source)` | NO — private, string-based | This is the real parser |

`decode_graph_yaml` hardcodes `Path::new("<memory>")`. If someone passes five graph
strings from five database rows, parse/decode diagnostics all collapse onto the same
generic `<memory>` label. Can't distinguish which failed.

**Needed at audit time:** A public label-aware function. Either expose
`parse_graph_str` publicly with a `&str` label parameter, or add
`decode_graph_yaml_labeled(input, label)`.

Status note: the implemented scope took the additive path and added
`decode_graph_yaml_labeled(...)`.

**Tests:** Existing decode tests use string literals → `decode_graph_yaml`. Stay as-is.
New tests verify label propagation in error messages.

---

### discovery.rs — THE HARD PART

Every filesystem touchpoint:

| Code | What it does | Why filesystem |
|------|-------------|----------------|
| `collect_candidate_search(base_dir, cluster_id, search_paths)` | Builds candidates relative to referring source's dir + search paths | Relative lookup from referring source |
| `candidate.exists()` | Checks disk existence | Filesystem presence check |
| `canonicalize_or_self(&candidate)` | Best-effort canonicalization; falls back to original path text on failure | Filesystem dedup identity |
| `visiting_paths: HashSet<PathBuf>` | Cycle detection by path | Path as visit identity |
| `visiting_keys: HashSet<(String, Version)>` | Cycle detection by semantic ID | Already transport-agnostic |
| `cluster_sources: HashMap<(String, Version), PathBuf>` | Maps semantic ID → canonical path for conflict detection | Path as source identity |
| `record_cluster_definition` | Same (id, version) at different canonical paths → error | Two-identity check |
| `visit()` → `parse_graph_file(&cluster_path)` | Reads nested cluster from disk | Filesystem read |
| Error messages | Reference paths throughout | Path in diagnostics |

#### Resolver trait — context model (confirmed by Codex)

Discovery is context-sensitive today. Lookup depends on the referring source's parent
directory plus ordered search paths. The resolver trait must support this context.
Any filesystem-parity resolver needs this context. Whether an in-memory resolver also
models referrer-sensitive scoping is a separate decision; see Decision 8.

```rust
// Resolver takes cluster_id only. Builder owns version arbitration.
fn resolve(
    &self,
    cluster_id: &str,
    referring_source: Option<&SourceRef>,
) -> Result<ResolverResult, LoaderError>
```

Where `ResolverResult` includes both found sources AND search trace:

```rust
struct ResolvedSourceCandidate {
    /// Canonical source identity used for cycle/conflict bookkeeping.
    source_ref: SourceRef,
    /// Concrete opened / lexical label used in diagnostics for this candidate.
    opened_label: String,
}

struct ResolverResult {
    /// Sources found in deterministic discovery order.
    found: Vec<ResolvedSourceCandidate>,
    /// What was searched but not found — for diagnostic error messages.
    search_trace: Vec<String>,
}

fn read(&self, source_ref: &SourceRef) -> Result<String, LoaderError>
```

This is necessary because current missing-cluster errors enumerate every searched
candidate path: "looked for 'shared_value.yaml' in: /path/a, /path/b — not found."
A resolver that only returns found results cannot produce this diagnostic. Likewise,
current parse / ID-mismatch errors report the exact label that was opened. The
landed resolver therefore reports searched scope plus per-found opened labels,
while content reads remain a separate step.

Current candidate-generation parity also includes a few smaller observable behaviors:

- searched paths are lexically deduped before formatting diagnostics
- a configured search root that already ends in `clusters/` does not get a redundant
  synthetic `clusters/clusters/{id}.yaml` candidate
- existing hits are deduped only after the existence check, using best-effort
  `canonicalize_or_self(...)`

**Ordering is part of parity.** `found` and `search_trace` must preserve the current
filesystem candidate order. Today discovery is order-sensitive: it searches in a fixed
sequence, surfaces the first parse/id/conflict error it encounters, and keeps the first
definition recorded for a semantic cluster key. A resolver that reorders candidates can
change observable error ordering and which file "wins."

**Discipline:** The resolver is source discovery, not version arbitration. The resolver
takes `cluster_id` only and returns ALL sources for that ID regardless of version.
The builder parses each returned source, checks the YAML `id` field, checks the version
selector, and decides which candidates match. This preserves parity with current behavior
where discovery records non-matching versions and only expansion fails later if no
version satisfies the selector.

FilesystemResolver uses `referring_source` to derive the current lexical `base_dir` for
relative lookup and populates `search_trace` with the lexically deduped candidate paths
it checked.
InMemoryResolver behavior depends on Decision 8. Even if it does not model relative
lookup, it still needs a deterministic precedence/search-trace order. Its `search_trace`
can summarize the checked in-memory scope, but it should still identify the checked scope
concretely, and the order of `found` must still be
defined by Decision 6 and the diagnostic output should not silently degrade.

#### Discovery semantics (confirmed by Codex)

Current discovery behavior that the resolver must preserve or explicitly diverge from:

1. Search is filename-by-cluster-id first (`{cluster_id}.yaml`)
2. Parse the file
3. Check YAML `id` field matches expected cluster_id — **mismatch is an immediate error**
4. Record the source definition for `(id, version)` and detect duplicate-definition conflicts
5. Check version selector against YAML version — **non-match is recorded and skipped, not an error**
6. Discovery does not fail on version mismatch alone; expansion fails later if no version satisfies

An in-memory resolver collapses steps 1-2 (no filename to match, content is already
provided). Steps 3-5 still apply — the builder validates content after the resolver
provides it.

**Second-order effect:** current loader public outputs can include sources that did not
actually satisfy the version selector. `record_cluster_definition(...)` runs before
version filtering, and `load_graph_sources(...)` later re-reads every `cluster_sources`
entry into `source_map` / `discovered_files`. The same pre-filter recording also affects
the public `discover_cluster_tree(...)` result and the public `load_cluster_tree(...)`
return value. An in-memory transport that only carries the matched tree would therefore
change observable loader API behavior, not just internal search.

**Important:** Filename-based resolution is not just an internal search detail. It is
part of the current public user-facing contract — asserted in loader API tests and
spelled out in emitted diagnostics (e.g. "cluster resolution is filename-based: the
file must be named '{id}.yaml'"). An in-memory transport does not merely swap storage;
it makes filename-based resolution transport-specific. The FilesystemResolver preserves
this behavior. The InMemoryResolver explicitly does not use it. This difference must be
documented as an intentional transport-specific semantic, not silently dropped.

#### Source identity — cycle detection and conflict detection

`(id, version)` is semantic identity only. SourceRef handles two separate concerns:

1. **Cycle detection on the active recursion stack** — "am I currently inside a visit
   to this source?"
   - Current: `visiting_paths: HashSet<PathBuf>` — inserted on entry, **removed on unwind**
   - This is NOT global dedup. The same source can be revisited through a different
     branch after the first visit unwinds. It only prevents cycles in the current
     recursion path.
   - In-memory: `visiting_sources: HashSet<SourceRef>` — same insert-on-entry,
     remove-on-unwind semantics. Must not be changed to global dedup.

2. **"Two DIFFERENT sources define the same (id, version)"** → error (conflict)
   - Current: `cluster_sources.get(&key)` returns existing path, compare to new path
   - This IS persistent across the full discovery — `cluster_sources` is never unwound.
   - In-memory: same logic with SourceRef instead of PathBuf

**What changes in ClusterTreeBuilder:**
- `visiting_paths: HashSet<PathBuf>` → `visiting_sources: HashSet<SourceRef>`
- `cluster_sources: HashMap<(String, Version), PathBuf>` → `HashMap<(String, Version), SourceRef>`
- `visit(path, def)` → `visit(source_ref, def)`
- Candidate search → resolver call
- `parse_graph_file` inside visit → resolver provides content
- Error messages: `path.display()` → rendered source labels / lexical source metadata
  associated with `SourceRef`, not just an opaque identity token

**What stays the same:**
- `clusters: HashMap<(String, Version), DecodedAuthoringGraph>` — semantic identity
- `visiting_keys: HashSet<(String, Version)>` — semantic cycle detection
- All validation logic — ID matching, version matching, conflict detection
- The recursive visit structure

---

### io.rs — PUBLIC API BLAST RADIUS

| Struct/Field | Current type | Status |
|-------------|-------------|--------|
| `FilesystemGraphBundle.root` | `DecodedAuthoringGraph` | Unchanged |
| `FilesystemGraphBundle.discovered_files` | `Vec<PathBuf>` | **Public API** |
| `FilesystemGraphBundle.source_map` | `BTreeMap<PathBuf, String>` | **Public API** |

| Function | Status |
|----------|--------|
| `load_graph_sources(path, search_paths)` | Stays for filesystem path |
| `canonicalize_or_self(path)` | Stays, used by FilesystemResolver |

`FilesystemGraphBundle` is public. Changing its field types is a breaking API
change. `InMemoryGraphBundle` is now its additive in-memory peer rather than a
retrofit of the filesystem carrier.

**Ordering note:** resolver search/error order and public bundle output order are different
observable behaviors. Missing-cluster diagnostics and first-error precedence follow
candidate search order; `source_map` / `discovered_files` currently follow `BTreeMap` key
order. Any in-memory additive type that keeps ordered maps should document that ordering
explicitly; changing containers changes public behavior.

#### Snapshot semantics (confirmed by Codex)

Today the loader reads the same source more than once:
- Root text is read via `fs::read_to_string(...)`
- Root is also parsed from disk via `parse_graph_file(...)`
- Nested cluster files are parsed during discovery
- Then re-read into `source_map`

An in-memory path naturally collapses this into a single snapshot — the caller provides
the string once, and both parsing and source-map storage use that same string. This is a
small semantic tightening: the filesystem path could theoretically see a file change
between the multiple reads, and discovery also has an existence-check/open window before a
candidate is later parsed. The in-memory path cannot observe those TOCTOU windows. This is
almost certainly better behavior, but it is a documented behavior difference.

---

### Loader public export blast radius (confirmed by Codex)

The following are publicly exported from `ergo-loader` and are affected:

| Export | Current shape | Impact |
|--------|-------------|--------|
| `load_graph_sources(path, search_paths)` | Path-based | Stays; may get in-memory sibling |
| `resolve_cluster_candidates(base_dir, id, search_paths)` | Path-based | Stays; may get resolver-backed sibling |
| `load_cluster_tree(root_path, search_paths)` | Path-based | Stays; may get resolver-backed sibling |
| `FilesystemGraphBundle` | PathBuf fields | Truthful filesystem carrier now landed |
| `InMemoryGraphBundle` | String-keyed logical-path fields | Additive in-memory peer now landed |
| `parse_graph_file(path)` | Path-based | Stays |
| `decode_graph_yaml(input)` | In-memory (label-hardcoded) | Audit gap now closed by additive `decode_graph_yaml_labeled(...)` |
| `decode_graph_json(input)` | In-memory | **Audit-time broken public surface now fixed in the delivered implementation.** Programmatic graph generation (web UIs, APIs) produces JSON natively, so wiring the existing public entry point was explicit adjacent work, not optional polish. |

Under the chosen scope path, existing path-based exports stay and in-memory
equivalents are additive, with one deliberate pre-publication rename:
`LoadedGraphBundle` -> `FilesystemGraphBundle`.

This table is not the entire semver surface. `ergo-loader` also exposes public modules via
`pub mod`, so callers can reach `discover_cluster_tree(...)`, `ClusterDiscovery`, and
loader error types directly. Those path-typed surfaces are also in blast radius and need
either additive parallel types or explicit non-goals, not silent behavior drift.

`resolve_cluster_candidates(...)` also has a specific public lexical-path contract today:
it best-effort canonical-dedupes existing hits internally after the existence check, but
returns the first-seen candidate path spellings as raw `PathBuf`s. Any resolver-backed
sibling or migration path must preserve or explicitly replace that behavior.

---

### project.rs — DEFERRED OR IN BLAST RADIUS

`ProjectManifest` is already deserialized config data from `ergo.toml`, but it still
contains unresolved path-like strings. The resolved path policy appears when
`ResolvedProject` / `ResolvedProjectProfile` join those fields against the project root.

| Function/Type | Filesystem-bound? | Notes |
|--------------|-------------------|-------|
| `load_project(start)` | YES | Reads ergo.toml from disk |
| `discover_project_root(start)` | YES | Walks filesystem upward |
| `ProjectManifest` | Mixed | Unresolved config data, reusable, but still carries path-like string fields |
| `ProjectProfile` / `ProjectIngress` | Mixed | Public config-shape types that already encode current project/profile policy |
| `ResolvedProject` | YES | Holds root PathBuf |
| `ResolvedProjectProfile` / `ResolvedProjectIngress` | Mixed | Public resolved types carrying path fields plus ingress/config/bounds state |
| `ProjectError` | Mixed | Public error surface for current project/profile resolution semantics |

**Whether project.rs is in active blast radius depends on a scoping decision.**
See decisions section.

**Second-order effect:** `ResolvedProjectProfile` is not just a bag of paths. It also
encodes current project-resolution policy, including automatic `root/clusters`
search-root injection. Deferring this area therefore defers the main documented
SDK ergonomic path, not just a helper type.

**Tests:** 4 tests, all temp-dir-based. All stay regardless of decision.

---

## The host crate: `crates/prod/core/host/src/`

### Host public export blast radius

Public exports from `ergo-host` via `lib.rs` that are directly path-bound or adjacent to
the same blast radius:

| Export | Current shape | Impact |
|--------|-------------|--------|
| `run_graph_from_paths(request)` | Path-based | Stays; may get object-based sibling |
| `run_graph_from_paths_with_surfaces(request, surfaces)` | Path-based + injected surfaces | Stays |
| `run_graph_from_paths_with_surfaces_and_control(request, surfaces, control)` | Path-based | Stays |
| `run_graph_from_paths_with_control(request, control)` | Path-based | Stays |
| `run_graph(request)` | Mixed — carries `graph_path` plus `DriverConfig` (which may itself be path-bearing via fixture input) | Stays; still path-shaped |
| `run_graph_with_control(request, control)` | Mixed — carries `graph_path` plus `DriverConfig` | Stays |
| `validate_graph_from_paths(request)` | Path-based | Stays; may get object-based sibling |
| `validate_graph_from_paths_with_surfaces(request, surfaces)` | Path-based + injected surfaces | Stays |
| `prepare_hosted_runner_from_paths(request)` | Path-based | Stays; may get object-based sibling |
| `prepare_hosted_runner_from_paths_with_surfaces(request, surfaces)` | Path-based + injected surfaces | Stays |
| `replay_graph_from_paths(request)` | Path-based | Stays |
| `replay_graph_from_paths_with_surfaces(request, surfaces)` | Path-based + injected surfaces | Stays |
| `replay_graph(request)` | **Already object-based** | Stays — already transport-agnostic |
| `run_fixture(request)` | Path-based | Stays |
| `graph_to_dot_from_paths(request)` | Path-based | Stays; may get object-based sibling |
| `run_demo_fixture_from_path(request)` | Path-based | Stays; demo/fixture utility surface remains separate from graph/cluster transport |
| `finalize_hosted_runner_capture(runner, stop)` | Object-based | Already transport-agnostic |
| `scan_adapter_dependencies(expanded, catalog, registries)` | Object-based | Already transport-agnostic |
| `validate_adapter_composition(expanded, catalog, registries, provides)` | Object-based | Already transport-agnostic |
| `parse_egress_config_toml(input)` | String/object-based | Already transport-agnostic; host already exposes a public TOML parser |
| `validate_egress_config(...)` | Object-based | Public validation helper stays transport-agnostic |
| `validate_manifest_path(...)` / `check_compose_paths(...)` / `ManifestSummary` / `HostManifestError` / `HostRuleViolation` | Path-based | Public manifest/composition surface with its own private adapter-manifest parser; any adapter string/object seam must account for this lane too |
| `RunDemoFixtureRequest` / `GraphToDotFromPathsRequest` / `RunGraphRequest` / `RunFixtureRequest` / `RunFixtureResult` / `HostedAdapterConfig` / `HostedReplayError` | Mixed | Already public; remain in blast radius anywhere they carry paths or path-derived diagnostics |
| `decision_counts(...)` / `replay_bundle_strict(...)` | Object-based | Public replay helpers stay transport-agnostic but remain adjacent to replay boundary docs |
| `describe_adapter_required(...)` | Object-based | Public error-surface helper stays unchanged but is adjacent to the same validation/reporting area |
| `AdapterDependencySummary` | Object-based | Public canonical adapter-diagnostic summary stays transport-agnostic but remains adjacent to the same reporting surface |
| `write_capture_bundle` / `CaptureBundle` / `CaptureJsonStyle` | Mixed | Public capture surface stays; output paths remain caller-supplied |
| `describe_host_replay_error` / `describe_replay_error` / `HostErrorDescriptor` | Object-based | Replay error surface stays public; may need diagnostic-label parity if setup errors stop carrying paths |
| `HostedRunner` | Object-based | Already transport-agnostic |
| `HostedEvent`, `HostedStepOutcome` | Object-based | Already transport-agnostic |
| `RuntimeSurfaces` | Object-based | Already transport-agnostic |
| `PrepareHostedRunnerFromPathsRequest` | Mixed — path fields plus `Option<EgressConfig>` | Stays; any new lower-level carrier should preserve object-based egress config at the host boundary |
| `HostRunError` / `HostReplayError` / `RunGraphResponse` / `RunSummary` / `InterruptedRun` / `RunOutcome` | Mixed | Public canonical result/error surface remains in blast radius wherever paths or path-derived capture behavior appear |

All `*_from_paths` exports stay as-is. Any new lower-level object-based seam is additive.
Several host exports (`replay_graph`, `finalize_hosted_runner_capture`,
`scan_adapter_dependencies`, `validate_adapter_composition`, `HostedRunner`,
`RuntimeSurfaces`) are already transport-agnostic and need no changes.

### Host ingestion sites — seam and its asymmetry

| Function | Filesystem? | Impact |
|----------|-------------|--------|
| `prepare_graph_runtime(graph_path, cluster_paths, surfaces)` | YES — calls loader | **The seam.** Below: objects. Above: paths. |
| `graph_to_dot_from_paths(request)` | YES — loads graph/clusters from paths, expands, uses path-backed cluster source reporting | Public host API that also goes through path-backed loader |
| `summarize_expand_error(...)` in `usecases.rs` and `graph_dot_usecase.rs` | YES — formats available cluster versions using `PathBuf` values from loader `cluster_sources` | Host diagnostics currently depend on path-typed loader source metadata; an in-memory loader path needs either host-visible source labels or a host-visible diagnostic map, not just a loader-internal `SourceRef` |
| `parse_adapter_manifest(path)` | YES — `fs::read_to_string` + YAML | Needs in-memory sibling |
| `manifest_usecases.rs` adapter parse path | YES — separate private `parse_adapter_manifest(...)` under public manifest/composition APIs | Adapter-manifest loading is duplicated in host today; updating only `usecases.rs` leaves a second public path-backed lane behind |
| Host request fields `egress_config: Option<EgressConfig>` | NO — host already receives parsed config objects | Host is already transport-agnostic for egress config at the request boundary; path-backed egress config loading is SDK-owned today |
| `replay_graph_from_paths_internal` capture read | YES — `fs::read_to_string(capture_path)` + JSON parse | Replay setup reads capture from disk |
| Fixture reading | YES | Separate concern (ingress) |
| Capture writing | YES | Separate concern (output) |
| Default capture naming | `graph_path.file_stem()` | See decisions section |

#### Host orchestration export blast radius

Subset of `ergo-host` public exports most relevant to run/replay/validation/manual-runner
orchestration. Adjacent utility exports are listed in the broader table above:

| Export family | Path-based? | Impact |
|--------------|-------------|--------|
| `run_graph_from_paths` / `_with_surfaces` / `_with_control` / `_with_surfaces_and_control` | YES | Stays; may get object-based sibling |
| `run_graph` / `run_graph_with_control` | Mixed — `RunGraphRequest` carries `graph_path` plus `DriverConfig` (fixture paths remain a separate ingress concern) | Stays |
| `replay_graph_from_paths` / `_with_surfaces` | YES | Stays; `ReplayGraphRequest` is already object-based |
| `replay_graph` | NO — already object-based | Unchanged |
| `validate_graph_from_paths` / `_with_surfaces` | YES | Stays; may get object-based sibling |
| `prepare_hosted_runner_from_paths` / `_with_surfaces` | YES | Stays; may get object-based sibling |
| `finalize_hosted_runner_capture` | NO — operates on objects | Unchanged |
| `run_fixture` | YES — fixture is path-based | Stays; separate concern (ingress) |
| `graph_to_dot_from_paths` | YES | Stays; may get object-based sibling |
| `scan_adapter_dependencies` | NO — operates on `ExpandedGraph` | Unchanged |
| `validate_adapter_composition` | NO — operates on objects | Unchanged |
| `HostedRunner` / `HostedEvent` / `HostedStepOutcome` | NO — object-based | Unchanged |
| `RuntimeSurfaces` | NO — runtime injection | Unchanged |
| Types: `DriverConfig`, `RunOutcome`, `InterruptionReason`, `ReplayGraphResult`, `RunControl`, `HostStopHandle`, etc. | Mixed | Unchanged |
| Types: `PrepareHostedRunnerFromPathsRequest` | Mixed — path fields plus `Option<EgressConfig>` | Stays; any new lower-level carrier should preserve object-based egress config at the host boundary |
| Types: `RunGraphFromPathsRequest` | YES | Stays |
| Types: `ReplayGraphRequest` | NO — already object-based | Unchanged |

All existing path-based exports stay. New object-based entry points are additive.
The canonical client-facing seams are the `*_from_paths` families plus
`finalize_hosted_runner_capture(...)` for manual-runner finalization. `run_graph`,
`run_graph_with_control`, `replay_graph`, `scan_adapter_dependencies`, and
`validate_adapter_composition` remain public lower-level building blocks for advanced
embedded callers and tests; `run_fixture` remains a public utility API, not the canonical
orchestration surface for SDK/CLI.

#### Host API asymmetry (confirmed by Codex)

| Request type | Path-based? | Object-based? |
|-------------|-------------|---------------|
| `RunGraphFromPathsRequest` | YES | NO |
| `RunGraphRequest` | YES — carries `graph_path` | NO |
| `ReplayGraphRequest` | NO | **YES — already done** |
| `ReplayGraphFromPathsRequest` | YES | Has object sibling |
| `PrepareHostedRunnerFromPathsRequest` | YES | NO |

Replay is already object-based at the request level. Run and runner-preparation are only
partially path-bound. `run_graph(...)` already executes from a prebuilt `HostedRunner`
plus derived dependency state; the remaining path coupling is mainly `graph_path` for
default capture naming, the persisted capture-output/result contract that flows into
`RunSummary.capture_path`, and any fixture-path-bearing `DriverConfig` supplied by the
caller. The lower-level in-memory gap is therefore narrower than “run has no object
seam”: it can be closed either by a sibling carrier for capture/source metadata or by
further decoupling finalization/output naming from `graph_path`.

**What the in-memory loader adds at host level:**
- A **new lower-level host seam**, not a new canonical client entrypoint. CLI/SDK should
  continue routing canonical run/replay/validation/manual-runner preparation through
  the existing `*_from_paths` families and canonical manual-runner finalization through
  `finalize_hosted_runner_capture(...)` unless doctrine is explicitly changed.
- This lower-level seam may accept decoded/resolved authoring assets (parsed graphs,
  resolved cluster set, and diagnostic source metadata). Host still owns expansion,
  catalog materialization, adapter preflight/binding, runtime setup, and runner
  construction. In canonical client flows, SDK/CLI still hand off to host-owned
  preparation; advanced embedded callers may continue using lower-level host APIs, but
  they must not treat them as a replacement canonical path.
- For **live run / validate / manual-runner preparation**, this joins the existing
  pipeline where `prepare_graph_runtime` currently sits: host takes authoring assets in,
  does its own expansion and runtime setup, and then continues into the appropriate live
  flow. Validation does not produce a runner; run/manual-runner continue into
  hosted-runner construction. Replay remains an adjacent lane with its own lower-level
  preparation path. Host-owned expansion error reporting still depends on loader-supplied
  source metadata.
- Host error reporting is also in blast radius: current unsatisfied-cluster-version diagnostics in both run preparation and DOT rendering consume `cluster_sources: HashMap<(String, Version), PathBuf>` from loader discovery and format those paths into the user-facing error surface. If loader stops returning `PathBuf` here, host needs an equivalent diagnostic label surface or rendered-source map. This is a real host error-surface dependency, not just a loader-internal refactor.
- Any additive host object seam must preserve the current **public error-bucket contract**.
  For live flows, adapter-required failures currently preserve their own
  `HostRunError::AdapterRequired` bucket; graph/discovery/adapter-setup failures otherwise
  land in `HostRunError::InvalidInput`; hosted-runner configuration validation lands in
  `HostRunError::StepFailed`; eager egress startup/shutdown land in
  `HostRunError::DriverIo`; and capture writing stays in `HostRunError::Io`. Replay keeps
  its separate `HostReplayError::{Setup, GraphIdMismatch, ExternalKindsNotRepresentable}`
  staging. A transport refactor must not flatten these into a generic "setup failed"
  surface.
- Adapter configuration may need a string/object seam in host because adapter manifest
  parsing is currently private and path-backed.
- Egress config path-loading is SDK-owned today, not host-owned: host request types already take `Option<EgressConfig>`, while the SDK currently reads TOML from disk and parses it before constructing host requests. In-memory ergonomics may therefore want either an SDK-side string-accepting helper or direct object wiring, but this is not a host filesystem seam in the same way adapter manifests are.
- Advanced host seams also preserve injected `RuntimeSurfaces` today via explicit
  `_with_surfaces` variants. Any new lower-level object seam should either preserve a
  surfaces-aware counterpart or explicitly defer that advanced embedded-caller lane.
- Boundary rule for additive host/SDK work: whether SDK may compose loader-owned public
  decode/discovery directly or must stop at host-facing seams is an explicit open choice
  in Decision 7. Regardless of that choice, SDK must not own expansion, dependency
  scanning, adapter binding, capture finalization, or claim a new canonical execution lane.
- Startup/finalization lifecycle is also in blast radius: live preparation and lower-level
  run currently **eagerly start egress before work begins**, and finalization remains
  ordered `ensure_capture_finalizable -> ensure_no_pending_egress_acks ->
  stop_egress_channels -> into_capture_bundle`. A transport refactor around object seams
  must preserve those ordering guarantees even if request types stop carrying paths.
- `graph_to_dot_from_paths(...)` is its own host ingestion site with separate loader +
  expansion flow. A seam added only at `prepare_graph_runtime(...)` does not
  automatically cover DOT rendering.

#### Host lanes that remain separate

Even if a new lower-level host object seam is added for live preparation, the host surface
still splits into distinct lanes that must be **explicitly included or explicitly
deferred**:

- **Live run / validate / manual-runner preparation** — the main lower-level seam discussed
  above
- **Replay preparation** — adjacent, but still path-backed below the request layer today
- **DOT rendering** — separate host ingestion site with its own loader + expansion path
  and, today, a core-catalog-only / raw-string-error asymmetry
- **Manifest/composition validation** — separate public path-backed lane with its own
  adapter-manifest parsing path
- **Fixture input / capture output** — separate ingress/output concerns, not solved by
  graph/cluster transport alone

Completion claims for this work should name which of these lanes are covered. Blast-radius
acknowledgment alone is not enough.

Two adjacent user-facing naming lanes should also be named as either included or deferred
if scope expands beyond core graph/cluster transport:

- **CLI render default output naming** — today defaults to `graph_path.with_extension("svg")`
- **Demo-fixture default capture naming** — today defaults from the fixture file stem to
  `target/<fixture-stem>-capture.json`

---

## The SDK crate: `crates/prod/clients/sdk-rust/src/`

Any new SDK support should stay additive. SDK must NOT own orchestration for in-memory
paths per SDK-CANON-1 / LAYER-3.

Additional SDK-owned product/path assumptions that are in blast radius:

- Deferring `project.rs` defers the main documented SDK ergonomic path, not a side
  concern. The shipped product surface is SDK-first and profile-oriented:
  `Ergo::from_project(...)`, `run_profile(...)`, `run_profile_with_stop(...)`,
  `replay_profile(...)`, `validate_project()`, and `runner_for_profile(...)` all depend
  on loader-owned project discovery/resolution from `ergo.toml`.
- SDK currently owns egress config file loading across explicit run paths and
  profile-derived execution / validation / manual-runner preparation paths: it reads
  TOML from `egress_config_path` and parses it into `EgressConfig` before constructing
  host requests. This is SDK-owned path logic today, not host-owned.
- `runner_for_profile(...)` and `ProfileRunner` are already shipped manual-runner APIs,
  not hypothetical convenience helpers. Current guarantees that any in-memory
  project/profile story would need to preserve or re-document:
  `runner_for_profile(...)` still resolves a normal run profile first, so profile
  resolution still enforces exactly one ingress source; `finish()` finalizes to an
  in-memory `CaptureBundle`; `finish_and_write_capture()` is the explicit file-writing
  path; `capture_output_path()` exposes the resolved capture file path.
- Manual runner is a mixed-mode product surface: runner creation still performs normal
  profile resolution and host preflight, but stepping itself does **not** launch ingress
  and does **not** enforce profile `max_duration` / `max_events`. Creation can already
  fail on adapter-required validation or eager egress startup, so any in-memory
  project/profile story must preserve those creation-time failure surfaces.
- A reusable `Ergo` handle is **not** a pre-resolved project snapshot. The handle
  preserves runtime surfaces and primitive instances, but every profile-facing call
  re-enters `load_project()` / `resolve_run_profile()` from disk. Any in-memory source
  design must choose whether same-handle behavior stays "re-resolve each call" or
  becomes explicit snapshot semantics.
- Reusable-handle behavior includes **post-error reuse**: a failed project/profile lookup
  does not consume the `Ergo` handle. The same handle is still expected to validate, run,
  or replay successfully on later calls.
- Explicit `RunConfig` and `ReplayConfig` remain largely path-shaped public APIs, while
  `IngressConfig` is already mixed (`Fixture { path }` vs `Process { command }`). Even if
  profile-based in-memory support is later added, these explicit config surfaces remain a
  separate public API decision from project/profile resolution.
- `replay_profile(...)` is already mixed-mode: project assets come from profile
  resolution, but the capture artifact remains an explicit caller-supplied path. Any
  in-memory project/profile story that does not also change replay APIs remains partly
  file-backed by design.
- `ErgoBuilder::project_root(...)`, `Ergo::from_project(...)`, and project/profile
  resolution stay path-backed unless in-memory project support is explicitly brought
  into scope.
- `validate_project()` also returns `ProjectSummary.root: PathBuf`, so SDK path identity
  is present in outputs as well as inputs.

If SDK in-memory support is added at all, it should be additive — new methods or a new
builder variant that wrap lower-level loader/host seams without inventing a second
execution model. Under current doctrine, that is not automatically doctrine-preserving:
any such methods would need an explicit doctrine/docs/boundary-check allowance and should
be treated as advanced / non-canonical helpers unless doctrine is explicitly changed.

**Tests:** Existing filesystem-based SDK tests stay relevant. Additional SDK parity
coverage should exercise manual-runner behavior, handle reuse / re-resolution semantics,
and any new wrappers added around lower-level in-memory seams.

---

## Non-code blast radius (confirmed by Codex)

| Doc | Why affected |
|-----|-------------|
| `docs/authoring/loader.md` | Loader contract — must acknowledge non-path transport |
| `docs/authoring/testing-notes.md` | Currently teaches filename-based cluster resolution and related path-shaped testing expectations |
| `docs/authoring/project-convention.md` | Current v1 project/profile model. If Decision 1 stays deferred, this doc should explicitly say the documented project/profile path remains filesystem-backed; if Decision 1 expands later, project transport needs a separate follow-on doctrine update |
| `docs/authoring/yaml-format.md` | Authoring spec — if a lower-level non-path seam is added, this doc should distinguish canonical path-based client entrypoints from lower-level non-path ingestion instead of implying either “paths only forever” or “new seam is canonical” |
| `docs/invariants/00-cross-phase.md` | Cross-layer invariant source for `LAYER-3` / canonical orchestration boundaries |
| `docs/invariants/07-orchestration.md` | Host surface — needs to acknowledge new lower-level object-based entry point. Current doctrine distinguishes canonical client entrypoints from lower-level host APIs; the new seam belongs in the lower-level bucket unless doctrine is explicitly changed. |
| `docs/invariants/08-replay.md` | Replay/client-authoring boundary doc — relevant anywhere the memo distinguishes tolerated non-canonical graph construction from host-owned replay/runner lifecycle |
| `docs/system/kernel.md` | System boundary — new transport seam may affect boundary descriptions. Whether the concrete `SourceRef` type crosses the loader/host boundary or host receives rendered diagnostic labels instead is a design choice, not a settled fact. |
| `docs/system/kernel-prod-separation.md` | Layer separation — new seam between loader and host transport |
| `docs/system/current-architecture.md` | Architecture overview — should describe a loader/host transport seam expansion without implying a repo-wide transport layer |
| `tools/verify_layer_boundaries.sh` / `tools/verify_runtime_surface.sh` | Enforcement scripts — may need updates so boundary checks encode the chosen SDK/host seam rather than silently allowing drift |
| `docs/authoring/getting-started-sdk.md` and CLI init scaffolds | User-facing/scaffold surfaces — they currently encode the filesystem project/profile story and should stay aligned with Decision 1 |
| `crates/prod/clients/sdk-rust/README.md` | SDK docs — if public surface grows |
| `crates/prod/core/loader/README.md` | Understates current API surface, needs update regardless |
| `crates/shared/fixtures/src/report.rs` and `crates/prod/clients/cli/src/fixture_ops.rs` | Fixture inspect/validate reporting is user-facing, serialized, and path-derived today (`fixture_path`, parse errors, text/JSON output) |
| `README.md` | Top-level project/layout story is still strongly filesystem-shaped today and should stay aligned with any chosen scope |

---

## Tempting approaches that weaken boundaries

Do NOT do these:

- SDK/client-built `ExpandedGraph` becomes a replacement for host-owned canonical
  execution. Lower-level/client-side graph construction may exist for narrow or
  illustrative cases, but it must remain non-canonical.
- SDK/client-built `HostedRunner` becomes an informal execution lane. Runner setup and
  lifecycle remain a sharper host-owned boundary than non-canonical graph-authoring
  examples.
- Loader starts doing catalog-aware validation (bleeds into kernel's job)
- RuntimeSurfaces smuggles authoring assets (wrong seam)
- Synthetic `mem://` paths jammed through `*_from_paths` (dishonest transport hiding)
- Hot-swap mutable graph behind one live runner (violates supervisor identity)

---

## Summary: what changes, what doesn't

### CHANGES
1. **New: internal SourceRef type** in loader — Clone + Eq + Hash with separate
   opened-label/search-trace carriage for diagnostics
2. **New: label-aware decode function** — replaces `"<memory>"` hardcode
3. **Adjacent public-surface fix: `decode_graph_json`** — the implemented scope wired the former broken stub so programmatic graph generation works natively with JSON. This supports the in-memory use case but, more importantly, closes a public API that previously always errored.
4. **New: resolver trait** in loader — context-aware, supports referring-source context
5. **New: FilesystemResolver** — wraps current discovery.rs search logic
6. **New: InMemoryResolver** — backed by caller-provided ordered source data per Decision 6
7. **Refactor: ClusterTreeBuilder** — resolver + SourceRef instead of paths
8. **Delivered later in implementation: lower-level host object seam(s)** — live preparation gained an additive object seam that accepts loader-owned sealed `PreparedGraphAssets` plus host-owned prep options, while replay / DOT / manifest-composition remained separate lanes
9. **Deferred adjacent work: string-accepting adapter parsing in host; optional SDK-side egress-config string helper or direct object wiring** — host already accepts `EgressConfig` objects, but adapter object/string transport remains deferred
10. **Docs** — see non-code blast radius table above
11. **Explicit scope/defer callouts for adjacent user-facing naming lanes** — render SVG output naming and demo-fixture capture naming, if those lanes are touched

### STAYS AS-IS
- `parse_graph_file` — filesystem decode entry point
- `load_graph_sources` — filesystem-specific high-level loader
- `resolve_cluster_candidates` — filesystem-specific (may get resolver-backed sibling)
- `load_cluster_tree` — filesystem-specific (may get resolver-backed sibling)
- All existing SDK methods and types, **if Decision 1 stays deferred**
- All existing host `*_from_paths` APIs
- All existing `ReplayGraphRequest` object-based paths
- Kernel, supervisor, runtime — untouched

### NO DEAD CODE
Every existing function and test stays. FilesystemResolver wraps the current discovery
logic in a trait implementation, but is not a trivial wrapper — it must also produce
search-trace diagnostics for missing-cluster errors that match current error quality.

### TEST IMPACT
- **Under the landed parallel-carrier path, existing filesystem tests should mostly keep passing, but parity-sensitive lexical-path / wording assertions may still force targeted implementation work or targeted test updates where the truthful public rename or additive peers touch asserted surfaces**
- **0 tests become dead code**
- **More than ~12 new or expanded tests is likely** once parity coverage is counted across
  loader, host, DOT rendering, and SDK product surfaces
- **New/expanded coverage should explicitly include:** loader ordering + search-trace
  parity, non-matching-version inclusion in public loader bundle outputs **and**
  public `discover_cluster_tree(...)` / `load_cluster_tree(...)` outputs, host
  lower-level object-seam parity plus DOT rendering parity, SDK manual-runner behavior,
  handle reuse / re-resolution semantics, and public bundle/API compatibility
- **Public compatibility coverage should also explicitly include:** module-public loader
  surfaces (`ClusterDiscovery`, `discover_cluster_tree`, `load_cluster_tree`,
  `LoaderIoError.path`) and the public host manifest/composition surface, not just
  `FilesystemGraphBundle` / `InMemoryGraphBundle`
- **Current green filesystem tests are not enough to protect every exported surface.**
  This implementation now adds direct regression coverage for `decode_graph_json(...)`,
  `discover_cluster_tree(...)`, and `load_cluster_tree(...)`, but host DOT
  behavior and the broader adjacent surfaces still should not be treated as
  implicitly protected by the narrow legacy suite.
- **Some existing integration tests get in-memory siblings**
- **If public loader bundle types are genericized or migrated away from their current truthful filesystem / in-memory split, some loader API tests will need updates** — the current `FilesystemGraphBundle` and `InMemoryGraphBundle` shapes are directly asserted in tests

---

## Refactor order

1. Define internal SourceRef in loader (Clone + Eq + Hash plus lexical/opened metadata)
2. Define resolver trait in loader (with referring-source context)
3. Add label-aware public decode function
4. Wrap existing discovery.rs logic in FilesystemResolver (not trivial — must produce search-trace diagnostics)
5. Make ClusterTreeBuilder generic over resolver (conceptually direct, but parity-sensitive because it changes discovery bookkeeping, public outputs, and diagnostics)
6. Add InMemoryResolver
7. Add lower-level live-preparation host object seam and explicitly defer or separately
   plan replay / DOT / manifest-composition lanes
8. Add string-accepting adapter parse function in host; decide whether SDK also gets a string-accepting egress-config helper
9. Add SDK convenience methods (only if Decision 4/7 explicitly allow them)
10. Update docs

Completed in the delivered implementation: `decode_graph_json` now routes through the
real typed decode path using the same serde-backed raw structure as YAML.

Steps 1-5: core refactor. Step 4 wraps current discovery logic in FilesystemResolver —
not trivial because it must also produce search-trace diagnostics for missing-cluster
errors and preserve per-found opened-label behavior. Step 5 is the main
`PathBuf` → `SourceRef` migration inside discovery/builder state, but it is still
parity-sensitive because public outputs and diagnostics are downstream of that state.
Under the additive path, existing filesystem tests should mostly stay green while
parity-focused coverage is added.

Steps 6-9: new capability. Additive in semver terms, but they still require real repo
changes across loader/host/SDK code and docs.

Step 10: docs and doctrine. These can proceed in parallel with exploratory refactor work,
but canonical-complete claims should not land until doctrine/docs are updated in line
with `DOC-GATE-1`.

---

## Historical Decisions Considered Before Implementation

The implementation plan in
`docs/plans/in-memory-loader-decision-rationale.md`
has since closed the active implementation choices. The options below remain as
the broader audit record of pressures that were considered while shaping the
scope. Where the landed code diverged, that is called out inline.

### Decision 1: In-memory project/profile scope

Is in-memory project/profile support part of this work or explicitly deferred?

- If **deferred:** this document's implementation scope is graph/cluster transport only.
  `project.rs`, `ResolvedProject*`, and project/profile-facing SDK product surfaces
  remain filesystem-backed and unchanged. Title of this work becomes
  "in-memory graph and cluster transport."
- If **included:** project.rs is in blast radius. Need a `ProjectManifest`-based
  non-filesystem project transport / representation and a corresponding SDK/host-facing
  product story. The exact constructor/type shape remains a separate design question and
  should not be treated as pre-decided here.

Independent of this decision, replay lower-level preparation, DOT rendering, and the
public manifest/composition lane must each be named as either **in scope** or
**explicitly deferred**. They are adjacent ingestion lanes, not automatic beneficiaries of
the live run/validate seam.

Before implementation started, the scope closure needed to be written down
explicitly, for example:

- Live run / validate / manual-runner preparation: in scope or deferred
- Replay preparation: in scope or deferred
- DOT rendering: in scope or deferred
- Manifest/composition validation: in scope or deferred
- Render/demo-fixture naming lanes: in scope or deferred

### Decision 2: FilesystemGraphBundle public type strategy

This was originally the `LoadedGraphBundle` question. The shipped code has since
chosen and landed the truthful filesystem name `FilesystemGraphBundle`, which
still publicly exposes `Vec<PathBuf>` and `BTreeMap<PathBuf, String>`.
Three options:

- **Option A: Keep filesystem-specific, add parallel type.** `FilesystemGraphBundle` stays
  PathBuf-backed for filesystem callers. Add a separately named in-memory peer for
  in-memory callers. Simplest transport split, but in the landed code this still
  combined with one deliberate pre-publication rename:
  `LoadedGraphBundle` -> `FilesystemGraphBundle`.
- **Option B: Genericize.** `FilesystemGraphBundle<S>` where S is the source identity type.
  More elegant, but generic virality may spread to callers.
- **Option C: Migrate to SourceRef everywhere.** Breaking change to existing callers.
  PathBuf-backed callers would need to construct SourceRef from paths.

Historical recommendation during audit: Option A on the transport split. The landed
code kept that split and also took the truthful pre-publication rename noted above.

**Historical pressure:** even if Option A was chosen, the audit originally pushed for
a host-defined request/asset carrier derived from loader outputs rather than direct
host re-export of the loader bundle type. The shipped code instead took the
split-carrier compromise recorded in
[/Users/sebastian/Projects/ergo/docs/plans/phase4-decision-record.md](/Users/sebastian/Projects/ergo/docs/plans/phase4-decision-record.md):
loader-owned sealed `PreparedGraphAssets` plus host-owned `LivePrepOptions`, with
host re-exporting the loader asset type in its lower-level block.

### Decision 2A: Loader-owned asset type crossing into host APIs

If the loader adds an additive in-memory asset/result carrier, may that exact type cross
into public host APIs?

- **Option A: Yes, host accepts the loader-owned carrier directly.** Lowest short-term
  friction, but couples host public API to loader transport ownership and makes boundary
  evolution harder.
- **Option B: No, host defines its own carrier derived from loader outputs.** Clearer
  ownership split: loader owns transport/discovery artifacts, host owns orchestration
  request shapes.
- **Option C: Thin adapter layer.** Loader exports its own additive carrier, but host
  accepts a small host-defined view/adapter that is mechanically derived from it.

Codex's recommendation: Option B (or C if ergonomics really demand a thin translation
layer). Host should not re-export loader transport types as part of its public contract.
This should be treated as an architecture gate to close before code, not follow-up polish.

### Decision 3: Default capture naming

Currently derived from `graph_path.file_stem()` with fallback `"graph"`. In-memory
graphs have no file stem.

Options:

- **Caller-provided label:** The in-memory entry point requires a capture name parameter.
- **Graph ID:** Derive from the graph's declared `id` field (already available after parse).
- **Require explicit:** The in-memory path does not support default capture naming; caller
  must always specify `capture_output` explicitly.

If DOT rendering or demo-fixture lanes are brought into scope, their path-derived default
output naming contracts need separate treatment too; this decision only covers graph-run
capture naming.

### Decision 4: SDK status of any new in-memory APIs

If SDK gains additive in-memory helpers, what doctrinal status do they have?

- **Option A: Advanced/non-canonical only.** SDK may expose explicit in-memory helper
  methods, but they are documented as lower-level/advanced wrappers over loader +
  lower-level host seams, not as a second canonical run/replay/profile lane.
- **Option B: No SDK surface yet.** Keep in-memory support below SDK entirely until
  doctrine is updated to define a canonical product story.
- **Option C: New canonical SDK lane.** SDK gets first-class in-memory run/profile APIs.
  This would require explicit doctrine changes and should not be treated as additive
  surface polish.

Codex's recommendation: Option A only if doctrine/docs/boundary checks are updated to
allow it explicitly; otherwise Option B is the current-doctrine-safe baseline.

### Decision 5: Strategy for loader path-typed public surfaces beyond `FilesystemGraphBundle`

`FilesystemGraphBundle` is not the only public loader surface carrying `PathBuf`-typed
contracts. `ClusterDiscovery.cluster_sources`, `LoaderIoError.path`, and the lexical
path behavior of `resolve_cluster_candidates(...)` are also public/observable today.

Options:

- **Option A: Keep all existing path-typed public surfaces stable.** Add new in-memory
  siblings or parallel types where needed, and treat existing path-typed APIs as the
  filesystem-specific contract.
- **Option B: Selective migration.** Keep `FilesystemGraphBundle` additive, but migrate some
  other public surfaces (for example `ClusterDiscovery` or loader errors) to `SourceRef`
  or generic carriers. This reduces duplication but increases semver and compatibility risk.
- **Option C: Uniform migration.** Move all relevant public loader source/location
  surfaces toward `SourceRef`/generic forms together. Most coherent long-term shape, but
  clearly a breaking public-surface change.

Historical recommendation during audit: Option A on the public-surface split, applied
consistently across the rest of the loader public surface. In the landed code, this
means additive in-memory peers while keeping existing filesystem-specific surfaces honest,
plus the deliberate `LoadedGraphBundle` -> `FilesystemGraphBundle` rename.

### Decision 6: Deterministic ordering contract for `InMemoryResolver`

Resolver ordering is observable because it affects first parse/id/conflict error surfaced,
which definition is recorded first, and search-trace presentation.

Options:

- **Option A: Caller-ordered inputs.** The in-memory API accepts an ordered collection
  (or preserves insertion order) and defines resolver precedence/search-trace order from
  that order.
- **Option B: Explicit priority field.** In-memory sources carry an explicit precedence
  or search-rank value that defines evaluation order.
- **Option C: Unordered map.** Use a plain map and document that ordering is unspecified.
  This is the least faithful to current filesystem behavior.

Codex's recommendation: Option A (simplest deterministic rule and closest to current
filesystem candidate ordering semantics).

### Decision 7: Allowed loader/helper surface for SDK additive in-memory wrappers

This decision is only reached if Decision 4 allows SDK helpers at all. If SDK gains
advanced/non-canonical in-memory helpers, what underlying loader/host surfaces may those
helpers actually compose under current doctrine and boundary checks?

Options:

- **Option A: Explicit allowlist.** SDK may call an approved subset of loader-owned
  public in-memory decode/discovery helpers and/or the chosen lower-level host seam, but
  only through an explicitly named allowlist. This keeps parser/public-loader access as a
  separate auditable question from canonical host orchestration ownership.
- **Option B: Host seam only.** SDK does not call loader decode/discovery directly for
  in-memory support; host must own all translation from raw authoring inputs downward.

Codex's recommendation: Option A only if the allowlist is named explicitly in
doctrine/docs and mirrored in boundary checks. If Decision 4 does **not** explicitly allow
SDK helpers, the current-doctrine-safe fallback remains "no SDK surface yet." If SDK
helpers are allowed, then Option B is the cleaner boundary among the Decision 7 choices.
Under current doctrine, SDK use of a new lower-level host seam is **not automatically
allowed** just because the helper is labeled advanced/non-canonical. If SDK is permitted
to call that seam, that is itself a doctrine/boundary change that must be made explicit
and mirrored in `tools/verify_layer_boundaries.sh`. Decision 7 Option A therefore implies
new positive allowlist logic in boundary enforcement, not just a small denylist tweak.

### Decision 8: In-memory scoping semantics

How should in-memory cluster lookup handle resolver context and namespace scoping?

Options:

- **Option A: Flat global namespace.** All supplied in-memory clusters are globally
  visible; `referring_source` does not affect lookup.
- **Option B: Referrer-sensitive scoping.** In-memory lookup models the current
  filesystem-relative behavior as closely as practical, using `referring_source` to
  influence visible search scope / precedence.
- **Option C: No implicit relative semantics.** In-memory lookup is deterministic and
  explicitly documented as not supporting filesystem-style relative search; callers must
  provide a single resolved namespace and not rely on referrer-specific visibility.

Codex's recommendation: Option B if parity with filesystem behavior is a goal; Option C
if the intent is a deliberately smaller, explicitly non-parity in-memory feature.
