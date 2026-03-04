# Crate Refactor Notes

This file tracks the crate-structure migration and doctrine boundary decisions for the kernel/core/clients cutover.

## Locked Boundaries

- `crates/prod/core/loader`: transport + decode + discovery only.
- `crates/kernel/*`: semantic enforcement and ontology/rule rejection only.
- `RuleViolation` ownership: kernel only.
- `crates/prod/clients/*`: thin adapters over loader + host.
- `crates/shared/*`: disallowed in kernel `[dependencies]`; allowed in kernel `[dev-dependencies]` only.

## Target Workspace Members

```toml
[workspace]
members = [
  "crates/kernel/runtime",
  "crates/kernel/adapter",
  "crates/kernel/supervisor",
  "crates/prod/core/host",
  "crates/prod/core/loader",
  "crates/prod/clients/cli",
  "crates/prod/clients/sdk-rust",
  "crates/prod/clients/sdk-types",
  "crates/shared/test-support",
  "crates/shared/fixtures",
]
```

## Implemented Moves

- `crates/runtime -> crates/kernel/runtime`
- `crates/adapter -> crates/kernel/adapter`
- `crates/supervisor -> crates/kernel/supervisor`
- `crates/ergo-host -> crates/prod/core/host`
- `crates/ergo-cli -> crates/prod/clients/cli`
- Added:
  - `crates/prod/core/loader`
  - `crates/prod/clients/sdk-rust`
  - `crates/prod/clients/sdk-types`
  - `crates/shared/test-support`
  - `crates/shared/fixtures`

## Loader / Kernel Split

- YAML graph decode moved into `ergo-loader`:
  - `decode_graph_yaml`
  - `parse_graph_file`
  - `load_cluster_tree`
  - `resolve_cluster_candidates`
- CLI graph runtime prep now calls loader APIs instead of owning decode/discovery internals.
- Loader README added at `crates/prod/core/loader/README.md`.
- Loader decode model uses `ClusterDefinition` directly (`DecodedAuthoringGraph` remains an alias, not a parallel IR).
- `LoadedGraphBundle` now exposes:
  - `root: ClusterDefinition`
  - `discovered_files: Vec<PathBuf>`
  - `source_map: BTreeMap<PathBuf, String>`

## Host / Client Split

- Host now owns adapter dependency scan + composition validation APIs:
  - `scan_adapter_dependencies`
  - `validate_adapter_composition`
- Added host usecase API surface:
  - `run_graph`
  - `replay_graph`
  - `run_fixture`
  - and request/result/error types.
- CLI replay path updated to call host adapter-composition enforcement.

## CLI Shape

CLI now has explicit thin-client structure in `crates/prod/clients/cli/src`:

- `main.rs` composition root
- `cli/args.rs`
- `cli/dispatch.rs`
- `output/text.rs`
- `output/json.rs`
- `output/errors.rs`
- `exit_codes.rs`

## CI / Verification

- Added `tools/verify_layer_boundaries.sh`:
  - kernel dependency direction checks (`prod/*` and `shared/*` restriction rules)
  - loader `RuleViolation` guard
  - client parser-internal import guard
- Integrated boundary guard into `tools/verify_runtime_surface.sh`.
- Updated replay naming guard paths in `tools/verify_runtime_surface.sh` to new crate locations.
- Updated verification scripts to support in-progress docs migration:
  - `tools/verify_doctrine_gate.sh` now resolves doctrine ledgers from `docs_legacy` first, with `docs` fallback.
  - `tools/verify_runtime_surface.sh` now resolves `PHASE_INVARIANTS.md` from `docs_legacy` first, with `docs` fallback.

## Loader Test Coverage Added

- Added integration tests in `crates/prod/core/loader/tests/loader_api.rs` for:
  - IO failure path in `load_graph_sources`
  - decode failure path in `decode_graph_yaml` (no rule-ID leakage)
  - discovery candidate resolution + deduping in `resolve_cluster_candidates`

## Migration Procedure (One-Run)

1. Baseline tests.
2. Path-only physical moves + workspace/path rewires.
3. Script/tool path rewires.
4. Loader extraction (transport/decode/discovery).
5. Host policy ownership shift.
6. CLI thin-shape wiring.
7. SDK scaffolding + doctrine notes.
8. Boundary guard integration.
9. Full verification gate.

## Current Note

- Per active doc-refactor workflow, all crate-refactor documentation updates are kept in this file rather than `/docs`.

## CLI Canonicalization Closure (Host-Owned Run/Replay)

- Added host path-based canonical APIs in `crates/prod/core/host/src/usecases.rs` and re-exported in `crates/prod/core/host/src/lib.rs`:
  - `run_graph_from_paths(RunGraphFromPathsRequest)`
  - `replay_graph_from_paths(ReplayGraphFromPathsRequest)`
- Canonical run/replay composition (loader decode/discovery, expansion, provenance, adapter validation/binder, runner setup) now lives in host.
- Existing lower-level host APIs remain available and are documented in rustdoc comments as lower-level surfaces:
  - `run_graph(RunGraphRequest)`
  - `replay_graph(ReplayGraphRequest)`
- CLI run path (`crates/prod/clients/cli/src/graph_yaml.rs`) now calls only `ergo_host::run_graph_from_paths` for canonical execution.
- CLI replay path (`crates/prod/clients/cli/src/cli/handlers.rs`) now calls only `ergo_host::replay_graph_from_paths`.
- `cli/handlers.rs` keeps `run_fixture` as explicit non-canonical demo utility path.
- `output/errors.rs` now maps `HostReplayError` variants (including host-owned graph-id mismatch/setup failures) into stable CLI error output.

## Guardrail Additions

- Extended `tools/verify_layer_boundaries.sh` with checks for:
  - `--direct` absence in CLI command-contract files (`main.rs`, `cli/args.rs`, `cli/dispatch.rs`, `output/text.rs`)
  - print macros only under `crates/prod/clients/cli/src/output/*`
  - no canonical run orchestration symbols in `graph_yaml.rs`
  - no canonical replay orchestration symbols in `cli/handlers.rs`
- `tools/verify_runtime_surface.sh` already invokes `verify_layer_boundaries.sh`; new checks are now part of the runtime verification gate.

## Tests Added/Updated

- Host:
  - `run_graph_from_paths_executes_simple_graph`
  - `replay_graph_from_paths_replays_capture`
- CLI dispatch:
  - `run_dispatch_returns_text_summary`
  - `replay_dispatch_returns_text_summary`
- CLI handlers:
  - `replay_handler_uses_host_path_api_error_surface`
- Graph run adapter-required test updated to verify real host-owned `RUN-CANON-2` path without CLI-local canonical orchestration.

## Doc Alignment Notes (Deferred To Docs Refactor)

- Intended canonical doc updates are tracked here (not applied under `/docs` in this cut):
  - YAML CLI contract: keep `ergo run <graph.yaml> --fixture ...`, keep `ergo fixture run ...` as utility.
  - Orchestration API table: list `run_graph_from_paths` / `replay_graph_from_paths` as canonical client entrypoints.
  - Keep RUN-CANON loci host-owned.
