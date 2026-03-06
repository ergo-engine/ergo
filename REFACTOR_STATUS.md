# Ergo Repository Refactor Analysis

**Analysis Date:** March 4, 2026
**Current Branch:** `refactor/boundaries` (up-to-date with origin)
**Most Recent Commit:** `e383d8f` - refactor(boundaries): restructure crates and docs into kernel/shared/prod layers

---

## 1. GIT STATUS & RECENT CHANGES

### Current State

- Branch: `refactor/boundaries` (tracking origin, no divergence)
- Untracked files: `.agents/.claude/settings.local.json`, `.claude/` (Claude Code session artifacts)
- No staged or unstaged changes to committed files

### Recent Commit History (Last 20)

The refactor spans multiple dimensions:

**Recent Semantic Work:**

- `e383d8f`: restructure crates and docs into kernel/shared/prod layers (MOST RECENT)
- `4d2aabe`: enforce duplicate event_id invariants across host and strict replay
- `1eb9724`: close D3 with host-owned strict replay and required effects schema
- `03d9174`: add ergo-host effect loop and restore termination-only supervisor boundary

**Earlier Feature Work:**

- `7a4b886`, `7a01d62`, `c4bf185`: context_bool_source and context_set_* action implementations
- `739122f`: context_number_source parameterization
- `3f453b1`, `4963943`: action payload ingress and tighter adapter typing
- `1b02ce5`, `427836a`: effect routing and $key parameter-bound manifest declarations

**Merge Points:**

- PR #45: canonical host effect loop
- PR #44: context_set_* implementations
- PR #43: context infrastructure

### File Statistics (Last 5 Commits)

**Summary:** 381 files changed, ~29,578 insertions(+), ~20,265 deletions(-)

**Major Deletions:**

- `crates/ergo-cli/` → `crates/prod/clients/cli/` (moved, not deleted)
- `crates/reference-client/` (entire directory removed, excluded from workspace)
- `tools/ergo-mcp/` (entire Python tool removed)
- `tools/ralph/` (removed)
- Composition tests moved from `crates/adapter/tests/` to `crates/kernel/adapter/tests/`

**Major Additions:**

- New crates: `crates/prod/core/loader/`, `crates/prod/core/host/`, `crates/shared/fixtures/`
- Comprehensive new docs tree: `docs/` (replacing `docs_legacy/`)
- Host usecases layer: `crates/prod/core/host/src/usecases.rs` (879 lines)
- Runner implementation: `crates/prod/core/host/src/runner.rs` (1234 lines)
- Replay infrastructure: `crates/prod/core/host/src/replay.rs` (833 lines)
- New action implementations: context_set_{bool,number,string} with manifests
- New source implementation: context_bool_source
- New trigger implementation: emit_if_event_and_true

**Moves (no code changes):**

- `crates/runtime/` → `crates/kernel/runtime/`
- `crates/adapter/` → `crates/kernel/adapter/`
- `crates/supervisor/` → `crates/kernel/supervisor/`

**Documentation Restructuring:**

- New docs structure: system/, orchestration/, authoring/, primitives/, contracts/, invariants/, ledger/
- Legacy docs preserved: `docs_legacy/` (33 .md files)
- New invariants tracking: 194+ tracked invariants across 16 phase files
- New ledger system: closure-register, gaps, escalations

---

## 2. WORKSPACE STRUCTURE

### Root Workspace (`Cargo.toml`)

```
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
exclude = [
  "crates/reference-client",
]
```

### Crate Hierarchy

#### KERNEL LAYER (Semantic enforcement, closed boundaries)

Three crates that form the core closed system:

1. **crates/kernel/runtime** (~500 lines)
   - Core primitives: sources, computes, triggers, actions
   - Execution engine
   - Manifest definitions
   - Tests: 4708 lines of runtime tests + 213 lines of action tests (5KB+ test suite)
   - Dependencies: semver, serde, serde_json, sha2

2. **crates/kernel/adapter** (~3KB)
   - Adapter composition validation
   - Event binding and fixture infrastructure
   - Schema materialization
   - Dependency scanning
   - Contains: composition tests, fixture stress tests, validation tests
   - Dependencies: ergo-runtime, jsonschema, regex, semver, serde, serde_json, sha2, hex

3. **crates/kernel/supervisor** (~450 lines)
   - Termination-only supervisor boundary (host-owned effects)
   - Replay infrastructure (411 new lines)
   - Capture enrichment (89 lines)
   - Integration tests + replay harness tests
   - Dependencies: ergo-adapter, ergo-runtime, hex, serde, serde_json, sha2
   - Features: `demo` (for testing)

#### PROD/CORE LAYER (Host runtime and loading)

1. **crates/prod/core/host** (6818 lines)
   - **New usecase layer:** usecases.rs (879 lines) with request/result types
   - **Runner:** runner.rs (1234 lines) - execute and fixture handling
   - **Replay:** replay.rs (833 lines) - replay execution with effects
   - **Error surface:** replay_error_surface.rs (246 lines)
   - **Manifests:** manifest_usecases.rs (314 lines, moved from CLI)
   - **Demo/Docs:** gen_docs_usecase.rs (1232 lines), demo_fixture_usecase.rs (77 lines)
   - **Graph operations:** graph_dot_usecase.rs (298 lines)
   - Dependencies: ergo-adapter, ergo-runtime, ergo-supervisor, ergo-loader, serde, serde_json, serde_yaml

2. **crates/prod/core/loader** (NEW)
   - Transport, decode, and discovery layer
   - YAML graph decoding: decode_graph_yaml, parse_graph_file, load_cluster_tree
   - Resolver: resolve_cluster_candidates
   - IO operations: load fixture files, source maps
   - Data structures: LoadedGraphBundle (root, discovered_files, source_map)
   - Dependencies: ergo-runtime, serde, serde_json, serde_yaml, semver

#### PROD/CLIENTS LAYER (Thin user-facing adapters)

1. **crates/prod/clients/cli** (2448 lines)
   - Main binary: main.rs (831 lines) composition root
   - CLI structure:
     - cli/args.rs (138 lines)
     - cli/dispatch.rs (329 lines)
     - cli/handlers.rs (89 lines)
   - Output formatting:
     - output/text.rs (124 lines)
     - output/json.rs (3 lines)
     - output/errors.rs (154 lines)
   - Fixture operations: fixture_ops.rs (235 lines)
   - Graph rendering: graph_yaml.rs (305 lines), graph_to_dot.rs (230 lines)
   - Validation: validate.rs (152 lines)
   - CSV fixtures: csv_fixture.rs (164 lines)
   - Tests: phase7_cli.rs (324 lines), fixture_graph_stress.rs (28 lines)
   - Dependencies: ergo-host, ergo-fixtures, serde, serde_json, serde_yaml

2. **crates/prod/clients/sdk-rust** (NEW)
   - SDK wrapper for Rust clients
   - Minimal crate (5 lines)

3. **crates/prod/clients/sdk-types** (NEW)
   - Type definitions for SDK
   - Minimal crate (6 lines)

#### SHARED LAYER (Disallowed in kernel [dependencies], allowed in [dev-dependencies])

1. **crates/shared/fixtures**
   - CSV fixture support
   - Report generation (354 lines)
   - Dependencies: ergo-adapter, csv, serde, serde_json

2. **crates/shared/test-support**
   - Minimal support crate

---

## 3. DEPENDENCY ANALYSIS

### Direct External Dependencies (Production)

| Dependency | Version | Used By | Purpose |
|------------|---------|---------|---------|
| semver | 1.0 | kernel/runtime, kernel/adapter, prod/core/loader | Version parsing and comparison |
| serde | 1.0 | All non-SDK crates | Serialization framework |
| serde_json | 1.0 | All non-SDK crates except SDK | JSON serialization |
| serde_yaml | 0.9 | kernel/adapter, prod/core/host, prod/core/loader, prod/clients/cli | YAML parsing |
| sha2 | 0.10 | kernel/runtime, kernel/adapter, kernel/supervisor | Hashing (event signatures) |
| hex | 0.4 | kernel/adapter, kernel/supervisor | Hex encoding |
| regex | 1.10 | kernel/adapter | Pattern matching |
| jsonschema | 0.40 | kernel/adapter | JSON schema validation |
| csv | 1.3 | shared/fixtures, prod/clients/cli | CSV parsing |

### No Obvious Unused Dependencies Found

- All declared dependencies are used in the refactored codebase
- No duplicate dependency declarations across crates (each crate declares what it directly uses)
- Internal crate dependencies follow the boundary rules:
  - Kernel crates depend on other kernel crates, never on prod/shared
  - Prod/core crates depend on kernel + loader
  - Prod/clients depend on prod/core + shared
  - Shared never appears in kernel [dependencies]

### Dependency Authority Boundaries

**Locked Rules** (from crate_refactor.md):

- `crates/prod/core/loader`: transport + decode + discovery only
- `crates/kernel/*`: semantic enforcement and ontology/rule rejection only
- `RuleViolation` ownership: kernel only
- `crates/prod/clients/*`: thin adapters over loader + host
- `crates/shared/*`: disallowed in kernel `[dependencies]`; allowed in kernel `[dev-dependencies]` only

All dependencies appear to respect these boundaries.

---

## 4. BUILD & COMPILATION STATUS

### Cargo Status

- **Cargo not available** in current environment (would require Rust toolchain setup)
- Cannot run `cargo check` or `cargo clippy` directly
- Cannot run `cargo +nightly udeps` for unused dependency detection

### Static Analysis Performed

- No stale crate path references (e.g., `crates/runtime/` → all updated to `crates/kernel/runtime/`)
- No broken internal dependencies in Cargo.toml files
- No obvious unused public exports in lib.rs files

### Code Metrics

| Crate | Lines of Code | Test Lines | Test Ratio |
|-------|---------------|------------|-----------|
| kernel/runtime | ~500 | 4921 | ~10:1 |
| kernel/adapter | ~3000 | multiple test files | High |
| kernel/supervisor | ~450 | integration + replay harness | High |
| prod/core/host | 6818 | Embedded in usecases | High |
| prod/core/loader | ~700 | 98+ (loader_api.rs) | Moderate |
| prod/clients/cli | 2448 | 324+ (phase7_cli.rs) | Moderate |

---

## 5. DOCUMENTATION STATE

### Primary Docs Tree (`docs/`)

**Fully reorganized into phase-based authority structure:**

#### System Layer

- `system/kernel.md` - kernel definition and closure semantics
- `system/ontology.md` - four primitives and causal roles
- `system/execution.md` - graph evaluation model
- `system/freeze.md` - frozen vs patchable declaration
- `system/terminology.md` - canonical terms and usage

#### Authoring & Contracts

- `authoring/cluster-spec.md` - cluster composition data structures
- `authoring/loader.md` - loader contract
- `authoring/concepts.md` - composition concepts
- `authoring/yaml-format.md` - YAML format reference

#### Orchestration & Contracts

- `orchestration/supervisor.md` - supervisor spec
- `contracts/ui-runtime.md` - UI/Runtime interface contract
- `contracts/extension-roadmap.md` - extension planning

#### Primitives (All Five)

- `primitives/source.md` - source implementations
- `primitives/compute.md` - compute implementations
- `primitives/trigger.md` - trigger implementations
- `primitives/action.md` - action implementations
- `primitives/adapter.md` - adapter implementation contract

#### Invariants & Ledger

- `invariants/INDEX.md` - phase invariant overview
- `invariants/00-cross-phase.md` through `invariants/15-action-composition.md` (16 files)
- `invariants/rule-registry.md` - enforcement rule registry
- `ledger/closure-register.md` - semantic closure tracking
- `ledger/gaps/` - open doctrine gaps
- `ledger/escalations/` - escalation tracking

### Legacy Docs Tree (`docs_legacy/`)

**Preserved for authority migration:**

- 33 .md files
- Authority order during migration: FROZEN → STABLE → CANONICAL → PROJECT
- Contains original CANONICAL/, FROZEN/, STABLE/, TOPICS/ structure
- Includes closure_register.md, adapter.md, ontology.md (v0), etc.

### Crate-Local READMEs

- `crates/prod/core/loader/README.md` (30 lines) - loader contract summary
- `crates/prod/clients/cli/README.md` (70 lines) - CLI usage and structure
- `crates/kernel/runtime/README.md` (updated, 20 lines) - runtime overview

### Special Documentation

- `crate_refactor.md` - refactor notes and decisions (159 lines)
- `AGENTS.md` - multi-agent protocols and governance (23+ lines of rules)
- `.agents/.claude/` - Claude Code session artifacts (untracked)

---

## 6. KEY REFACTOR DECISIONS & BOUNDARIES

### Crate Restructuring Completed

✓ `crates/runtime/` → `crates/kernel/runtime/`
✓ `crates/adapter/` → `crates/kernel/adapter/`
✓ `crates/supervisor/` → `crates/kernel/supervisor/`
✓ `crates/ergo-host` → `crates/prod/core/host/`
✓ `crates/ergo-cli` → `crates/prod/clients/cli/`

### New Crates Created

✓ `crates/prod/core/loader/` - YAML decode + discovery (transport layer)
✓ `crates/prod/clients/sdk-rust/` - SDK wrapper
✓ `crates/prod/clients/sdk-types/` - Type definitions
✓ `crates/shared/fixtures/` - Fixture support (CSV, reporting)
✓ `crates/shared/test-support/` - Test utilities

### Semantic Work Completed

✓ Host effect loop with strict replay
✓ Duplicate event_id invariant enforcement
✓ Action payload ingress infrastructure
✓ Context_set_{bool,number,string} implementations (3 new actions)
✓ Context_bool_source implementation
✓ emit_if_event_and_true trigger implementation
✓ $key parameter-bound manifest declarations
✓ Effect routing infrastructure

### Documentation Authority Migration (In Progress)

✓ New `docs/` tree created with phase-based organization
✓ Authority levels declared: FROZEN, STABLE, CANONICAL, CONTRACTS
✓ Invariants tracked: 194+ rules across 16 phases + rule registry
✓ Legacy `docs_legacy/` preserved for parallel authority
⚠ Authority order during migration not fully resolved in all documents

### Client Refactoring

✓ CLI moved to thin client pattern
✓ Manifest usecases moved from CLI to host (manifest_usecases.rs)
✓ Runner abstraction created (runner.rs)
✓ Graph execution surface exposed through host usecases
✓ Replay harness created with separate test layer

### Removals

✓ `crates/reference-client/` - React UI removed from workspace
✓ `tools/ergo-mcp/` - Python MCP server removed
✓ `tools/ralph/` - Analysis tool removed
✓ Old composition tests (moved and expanded in kernel/adapter/tests/)

---

## 7. CRITICAL OBSERVATIONS & NEXT STEPS

### What's Working Well

1. **Boundary enforcement:** No kernel → prod/shared dependencies detected
2. **Consistent refactoring:** All crate moves completed without path inconsistencies
3. **Comprehensive test coverage:** 5000+ line test suite for runtime, expanded adapter tests
4. **Thorough documentation:** Authority levels explicitly declared, phase invariants tracked
5. **Semantic completeness:** All recent feature work (context_set_*, event_id invariants, effects) integrated

### Potential Concerns for Code Review

**1. Cargo Build Status Unknown**

- Cannot verify that workspace compiles without Rust toolchain
- Recommend running: `cargo check` and `cargo clippy --all-targets`
- Check for any unused dependency warnings from `cargo check`

**2. SDK Crates Are Minimal Stubs**

- `crates/prod/clients/sdk-rust/` and `crates/prod/clients/sdk-types/` contain only 5-6 lines
- These may be intentional placeholders, but verify they're not incomplete

**3. Authority Migration In Progress**

- Both `docs/` and `docs_legacy/` exist simultaneously
- Authority order rules must be enforced during transition
- AGENTS.md indicates multi-agent review flow; verify all PRs follow escalation rules

**4. Test Coverage Asymmetry**

- Runtime has extensive test suite (4700+ lines)
- Host/CLI test coverage is more moderate (300-350 lines each)
- Verify host usecases and runner are adequately tested (recommend running test suite)

**5. Removed Tools & UI**

- `crates/reference-client/` UI removed
- `tools/ergo-mcp/` and `tools/ralph/` removed
- Verify these are intentional exclusions and not needed downstream

### Recommended Next Steps

1. **Build Verification**

   ```bash
   cargo check
   cargo clippy --all-targets
   cargo test --all
   cargo +nightly udeps
   ```

2. **Boundary Verification**
   - Run `tools/verify_layer_boundaries.sh` (exists in repo)
   - Verify no unexpected imports across layers

3. **Documentation Consistency**
   - Audit authority level declarations
   - Ensure all new semantic work is reflected in invariants/
   - Verify migration from docs_legacy to docs complete

4. **Test Coverage Analysis**
   - Run coverage tool on host and CLI crates
   - Compare against runtime coverage baseline

5. **Semantic Verification**
   - Confirm event_id uniqueness enforcement is working
   - Test effect routing with latest implementations
   - Validate action payload ingress in composition tests

---

## Summary Statistics

| Metric | Value |
|--------|-------|
| Total Crates in Workspace | 10 |
| Kernel Crates | 3 |
| Prod/Core Crates | 2 |
| Prod/Clients Crates | 3 |
| Shared Crates | 2 |
| Total External Dependencies | 9 unique packages |
| Documentation Files (Primary) | docs/ with 8 directories |
| Documentation Files (Legacy) | 33 files in docs_legacy/ |
| Tracked Invariants | 194+ rules |
| Test Files | 10+ separate test suites |
| Code Moved/Refactored | 381 files |
| Net Changes | +29,578 insertions, -20,265 deletions |
| Current Branch Status | Up-to-date with origin |

---

## File Structure Overview

```
/sessions/modest-affectionate-dijkstra/mnt/ergo/
├── Cargo.toml (workspace root, 10 members)
├── crate_refactor.md (refactor decision log)
├── AGENTS.md (governance & protocols)
├── docs/ (current primary docs)
│   ├── INDEX.md
│   ├── system/ (kernel, ontology, execution, freeze, terminology)
│   ├── authoring/ (cluster-spec, concepts, yaml-format, loader)
│   ├── orchestration/ (supervisor)
│   ├── contracts/ (ui-runtime, extension-roadmap)
│   ├── primitives/ (source, compute, trigger, action, adapter)
│   ├── invariants/ (16 phase files + INDEX + rule-registry)
│   └── ledger/ (closure-register, gaps, escalations)
├── docs_legacy/ (legacy authority tree, 33 files)
├── tools/
│   ├── verify_doctrine_gate.sh
│   ├── verify_layer_boundaries.sh
│   └── verify_runtime_surface.sh
├── crates/
│   ├── kernel/
│   │   ├── runtime/ (500 LOC + 5KB tests)
│   │   ├── adapter/ (3KB + tests)
│   │   └── supervisor/ (450 LOC + integration tests)
│   ├── prod/
│   │   ├── core/
│   │   │   ├── host/ (6818 LOC)
│   │   │   └── loader/ (700 LOC + tests)
│   │   └── clients/
│   │       ├── cli/ (2448 LOC + tests)
│   │       ├── sdk-rust/
│   │       └── sdk-types/
│   └── shared/
│       ├── fixtures/ (CSV + reporting)
│       └── test-support/
└── target/ (generated build artifacts)
```
