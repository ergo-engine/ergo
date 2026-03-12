---
Authority: PROJECT
Date: 2026-03-04
Author: Claude Opus 4.5 (Structural Auditor)
Status: OPEN
Branch: feat/ergo-init
Tier: 3 (Developer Experience — the gate)
Depends-On: feat/catalog-builder, feat/adapter-runtime; docs/ledger/gap-work/open/custom-implementation-loading.md (for EI-8)
---

# Ergo Workspace Scaffolding and Project Conventions

## Scope

Define how a developer creates, organizes, and runs an Ergo project. After this branch, all domain-specific work (adapters, custom implementations, clusters, graphs) happens inside an ergo workspace — never in `crates/kernel/` or `crates/prod/`.

This branch is the gate between infrastructure work and domain work.

Entirely prod-layer. No kernel changes. No frozen doc changes.

## Current State

There is no project convention. The CLI operates on individual file paths and explicit ingress flags:

- `ergo run <graph.yaml> --fixture <fixture.jsonl> --adapter <adapter.yaml>`
- `ergo run <graph.yaml> --driver-cmd <program> [--driver-arg <value> ...] --adapter <adapter.yaml>`
- `ergo validate <graph.yaml>`

Users must know which flags to pass and where their files are. There is no discovery, no convention, no scaffolding.

## What's Needed

### Project Layout Convention

A standard directory structure that the CLI discovers automatically:

```
my-project/
├── ergo.toml              # project manifest (name, version, dependencies?)
├── graphs/                # graph YAML files
│   └── my_strategy.yaml
├── clusters/              # reusable cluster definitions
│   └── ema.yaml
├── adapters/              # adapter manifest YAML files
│   └── my_adapter.yaml
├── implementations/       # custom Rust implementations (compiled separately?)
│   └── src/
│       └── lib.rs
├── fixtures/              # test event sequences
│   └── backtest_data.jsonl
└── captures/              # replay capture bundles (output)
    └── my_strategy-capture.json
```

Layout is TBD. The above is a starting point, not a commitment.

### CLI Commands

| Command | Description |
|---------|-------------|
| `ergo init` | Scaffold a new project with the standard layout |
| `ergo run` (inside project) | Discover graph, adapter, and driver/fixture wiring from project layout. No path flags required. |
| `ergo validate` (inside project) | Validate all graphs + adapter compositions in the project. |
| `ergo replay` (inside project) | Replay from captures directory. |

Existing path-based commands continue to work for non-project usage.

### Discovery Logic

When the CLI runs inside a project directory (detected by `ergo.toml`):

1. Find graphs in `graphs/`
2. Find clusters in `clusters/` (added to loader search paths)
3. Find adapter manifests in `adapters/`
4. Find fixtures in `fixtures/`
5. Resolve driver configuration defaults from project manifest or convention using the host `DriverConfig` API from `feat/adapter-runtime`
6. Load custom implementations from `implementations/` (via `feat/catalog-builder`)

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| EI-1 | Define project layout convention | Layout documented. Reviewed by Sebastian. | Claude + Sebastian | OPEN |
| EI-2 | Define `ergo.toml` schema | Minimal project manifest: name, version. Extensible for future fields. | Codex | OPEN |
| EI-3 | Implement `ergo init` | Command creates directory structure with template files. | Codex | OPEN |
| EI-4 | Implement project discovery | CLI detects `ergo.toml`, resolves paths from project root. | Codex | OPEN |
| EI-5 | `ergo run` inside project | Discovers graph + adapter + driver/fixture wiring from layout. Runs through the host runner + adapter ingress path. | Codex | OPEN |
| EI-6 | `ergo validate` inside project | Discovers all graphs + adapter manifests. Validates each composition. Reports all errors, not just first. | Codex | OPEN |
| EI-7 | Cluster search path integration | `clusters/` directory automatically added to loader search paths during project runs. | Codex | OPEN |
| EI-8 | Custom implementation loading | Implementations from `implementations/` are loaded via `feat/catalog-builder` API using the mechanism approved in `GW-EI8-1`, with matching tests | Codex | OPEN |
| EI-9 | Test: `ergo init` + `ergo run` round-trip | Init a project, place a graph and fixture, run it, verify capture output in `captures/`. | Codex | OPEN |
| EI-10 | Test: project validation catches composition errors | Project with mismatched adapter/graph. `ergo validate` reports typed error with rule ID. | Codex | OPEN |
| EI-11 | Documentation | User-facing guide: "Getting Started with Ergo." Covers init, authoring a graph, running, replaying. | Claude | OPEN |

## Design Constraints

- The project layout is a convention, not a hard requirement. Path-based CLI usage continues to work.
- Custom implementation loading (EI-8) is gated by `GW-EI8-1`; implementation must follow the selected mechanism and documented non-goals.
- No domain-specific language in project scaffolding. Template files use generic examples (number_source, add, emit_if_true), not trading examples.
- After this branch, domain-specific vertical work is done by Sebastian inside a workspace using the extension surface. It does not appear in the ergo repo.

## What This Branch Enables

After `feat/ergo-init` merges, a developer can:

1. `ergo init my-project`
2. Write graphs and clusters in YAML
3. Write an adapter manifest in YAML
4. Optionally write custom implementations in Rust
5. `ergo validate` to check compositions
6. `ergo run` to execute against fixtures or live adapters
7. `ergo replay` to verify determinism

All domain-specific work lives in the workspace. The ergo repo provides the runtime, the contracts, and the tools.
