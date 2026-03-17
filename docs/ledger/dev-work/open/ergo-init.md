---
Authority: PROJECT
Date: 2026-03-16
Author: Sebastian (Architect) + Codex (Build Orchestrator)
Status: OPEN
Branch: feat/ergo-init
Tier: 3 (Developer Experience — the gate)
Depends-On: >-
  feat/catalog-builder, feat/adapter-runtime, feat/egress-surface,
  feat/sdk-rust;
  docs/ledger/decisions/custom-implementation-loading.md
  (EI-8 uses in-process Rust crate registration through CatalogBuilder);
  docs/ledger/decisions/multi-ingress-host-direction.md
  (v1 remains one ingress channel per run profile)
---

# Ergo SDK Project Scaffolding and Workspace Conventions

## Scope

Define the v1 project convention for how a developer creates,
organizes, builds, and runs an Ergo application.

The product surface is the Rust SDK, not the CLI. A production Ergo
project is a Rust crate that:

- depends on `ergo-sdk-rust`
- registers custom primitives in-process
- runs named profiles from `ergo.toml`

After this branch, domain work happens inside an Ergo project rather
than inside `crates/kernel/` or `crates/prod/`. That project must give
users a clear home for:

- implementations
- graphs
- clusters
- adapters
- ingress channels
- egress channels
- fixtures
- captures
- `Cargo.toml`
- `ergo.toml`

This branch owns project convention, SDK-oriented scaffolding, and
shared project/profile resolution. It does not redefine runtime or host
semantics.

## Current State

The SDK-first application surface now exists, but the scaffolded
project surface does not.

Current reality:

- [sdk-rust](/Users/sebastian/Projects/ergo/crates/prod/clients/sdk-rust/src/lib.rs)
  now exposes:
  - `Ergo::builder()`
  - `Ergo::from_project(...)`
  - in-process primitive registration
  - `run_profile(...)`
  - `replay_profile(...)`
  - `validate_project()`
- Project/profile resolution now exists in the SDK, including implicit
  `clusters/` search path handling.
- The CLI operates on explicit paths and run flags:
  - `ergo run <graph.yaml> --fixture <fixture.jsonl> --adapter <adapter.yaml>`
  - `ergo run <graph.yaml> --driver-cmd <program> [--driver-arg
    <value> ...] --adapter <adapter.yaml> [--egress-config
    <egress.toml>]`
  - `ergo validate <graph.yaml>`
  - `ergo replay <capture.json>`
- Real production users need custom Rust `ActionPrimitive`,
  `SourcePrimitive`, and related implementations plus
  `CatalogBuilder` registration to do useful work.
- There is still no `ergo init` scaffolded Rust crate, no generated
  sample project, and no CLI convenience command layer consuming the
  shared loader-owned project-resolution surface yet.

So the repo now has a real SDK-first entrypoint, but it still lacks the
workspace and scaffolding layer that makes that product surface
habitable for users.

## V1 Stance

The v1 project convention should lock the following scope now:

- `ergo-sdk-rust` is the primary product surface.
- The CLI remains a development/support tool for validation, replay,
  fixture runs, and optional project conveniences.
- One ingress channel per run profile.
- If a user needs multiple live feeds, they must multiplex them
  upstream into one ingress channel; canonical host remains
  single-ingress by
  [multi-ingress-host-direction.md](../decisions/multi-ingress-host-direction.md).
- `Cargo.toml` owns Rust build configuration.
- `ergo.toml` owns Ergo project and profile resolution.
- `ergo.toml` references a standalone egress TOML file rather than
  embedding the route table in v1.
- Clusters are first-class scaffolded authoring artifacts and should be
  readily usable in the sample project.
- Shared project resolution belongs to SDK plus prod loader. CLI may
  consume it, but it is not the authority for the product surface.

## SDK-First Entry Surface

The scaffolded project should feel like a normal Rust application:

```rust
let ergo = Ergo::builder()
    .project_root(".")
    .add_source(MyPriceSource::new())
    .add_action(MyOrderAction::new())
    .build()?;

let outcome = ergo.run_profile("live")?;
```

Equivalent explicit-config mode should also exist for non-project use,
but project/profile execution is the v1 ergonomic path.

## Project Layout Convention

The v1 layout should be a Rust crate plus authored asset directories:

```text
my-project/
├── Cargo.toml
├── ergo.toml
├── src/
│   ├── main.rs
│   └── implementations/
│       ├── mod.rs
│       ├── sources.rs
│       └── actions.rs
├── graphs/
│   └── strategy.yaml
├── clusters/
│   └── shared_math.yaml
├── adapters/
│   └── strategy.yaml
├── channels/
│   ├── ingress/
│   │   └── live_feed.py
│   └── egress/
│       └── broker.py
├── egress/
│   └── live.toml
├── fixtures/
│   └── backtest.jsonl
└── captures/
    └── backtest.capture.json
```

Directory roles:

- `Cargo.toml` defines the Rust crate and SDK dependency.
- `src/main.rs` is the user-owned application entrypoint.
- `src/implementations/` contains custom Source, Compute, Trigger, and
  Action implementations registered through the SDK/CatalogBuilder path.
- `graphs/` contains graph YAML entrypoints.
- `clusters/` contains reusable cluster definitions. Project resolution
  adds it to loader search paths automatically.
- `adapters/` contains adapter manifests.
- `channels/ingress/` contains user-authored ingress channel programs.
- `channels/egress/` contains user-authored egress channel programs.
- `egress/` contains standalone `EgressConfig` TOML files referenced by
  `ergo.toml`.
- `fixtures/` contains deterministic input event streams.
- `captures/` contains replay artifacts produced by runs.

The sample project should include:

- one sample cluster in `clusters/`
- one sample graph in `graphs/` that uses that cluster
- one sample custom Action implementation with external intent
- one sample adapter, ingress channel, egress channel, and fixture

## `ergo.toml` As Project Authority

`Cargo.toml` answers Rust build questions. `ergo.toml` answers Ergo
project questions.

The project manifest must define named run profiles that resolve the
authored artifacts into the inputs the current host already
understands.

Minimum project fields:

- `name`
- `version`
- `profiles.<name>`

Each profile should resolve:

- `graph`
- `adapter`
- implicit project `clusters/` search path
- exactly one ingress source:
  - `fixture`, or
  - `ingress` process command
- optional `egress` config path
- optional capture output override

Illustrative v1 shape:

```toml
name = "my-project"
version = "0.1.0"

[profiles.backtest]
graph = "graphs/strategy.yaml"
adapter = "adapters/strategy.yaml"
fixture = "fixtures/backtest.jsonl"
capture_output = "captures/backtest.capture.json"

[profiles.live]
graph = "graphs/strategy.yaml"
adapter = "adapters/strategy.yaml"
egress = "egress/live.toml"
capture_output = "captures/live.capture.json"

[profiles.live.ingress]
type = "process"
command = ["python3", "channels/ingress/live_feed.py"]
```

Profile rules:

- one profile resolves to one graph + one adapter + one ingress source
- `fixture` and `ingress` are mutually exclusive
- project `clusters/` is always searched automatically; users do not
  repeat it in every profile
- `egress` is optional, but when present it points to the existing
  standalone TOML surface chosen in
  `decisions/egress-routing-config.md`
- custom primitive registration is code-level in `src/main.rs` /
  `src/implementations/`, not a per-profile manifest field

## Product Entry Points

<!-- markdownlint-disable MD013 -->
| Surface | Role |
| ------- | ---- |
| `ergo-sdk-rust` | Primary product API for building an engine, registering primitives, resolving projects, running profiles, validating, and replaying |
| `ergo init` | Scaffold a Rust crate that depends on the SDK and includes sample authored assets |
| CLI project commands | Optional development convenience over the same shared project-resolution surface |
| Existing path-based CLI commands | Continue to work for explicit non-project usage |
<!-- markdownlint-restore -->

## Shared Project Resolution Logic

Project resolution should be shared between SDK and CLI:

1. Discover project root by locating `ergo.toml`.
2. Resolve all manifest-relative paths from that root.
3. Add `project_root/clusters/` to loader search paths automatically.
4. Resolve a named profile into current host inputs:
   - graph path
   - adapter path
   - one ingress source (`fixture` or process ingress command)
   - optional egress config path parsed into `EgressConfig`
5. Build runtime surfaces from:
   - core primitives
   - user-registered custom primitives from the Rust crate
6. Pass the resolved project profile into the existing host run/replay
   surfaces rather than inventing a second execution model.

In other words:

- SDK is the primary product entry surface
- prod loader resolves project files and cluster discovery
- host executes the resolved profile
- CLI may wrap the same resolution and host calls for convenience

## Closure Ledger

<!-- markdownlint-disable MD013 -->
| ID | Task | Closure Condition | Owner | Status |
| -- | ---- | ----------------- | ----- | ------ |
| EI-0 | Define SDK-first public surface | `ergo-sdk-rust` exposes the canonical builder/project API over host + loader. Scaffold consumption remains with `feat/ergo-init`. | Codex | CLOSED |
| EI-1 | Define Rust crate project layout | Layout documented as the concrete v1 project convention, including `Cargo.toml`, `src/main.rs`, `src/implementations/`, `graphs/`, `clusters/`, `adapters/`, `channels/`, `egress/`, `fixtures/`, `captures/`, and `ergo.toml`. Reviewed by Sebastian. | Claude + Sebastian | OPEN |
| EI-2 | Define `ergo.toml` schema | Project manifest includes named profiles that resolve graph, adapter, implicit cluster search path, exactly one ingress source, optional egress config path, and optional capture output. Delivered and implemented by `feat/sdk-rust`. | Codex | CLOSED |
| EI-3 | Implement `ergo init` scaffold | `ergo init` creates a Rust crate depending on the SDK, with sample primitives, graph, cluster, adapter, channels, fixture, capture directory, and `ergo.toml`. | Codex | OPEN |
| EI-4 | Implement shared project discovery/resolution | `ergo-loader` exposes one project-resolution surface for `ergo.toml`, relative paths, and cluster search paths. SDK consumes it now; optional CLI convenience paths may consume the same surface later. | Codex | CLOSED |
| EI-5 | Implement SDK profile execution path | `Ergo::from_project(...).run_profile(...)` (or equivalent) resolves one named profile into graph + adapter + ingress source + optional egress config and runs through the existing host path. Delivered by `feat/sdk-rust`. | Codex | CLOSED |
| EI-6 | Implement project validation surface | SDK validation resolves every named profile, including graph/adapter composition and referenced egress config parsing when present. CLI validation may wrap the same surface. Delivered by `feat/sdk-rust`. | Codex | CLOSED |
| EI-7 | Make clusters first-class in scaffold and resolution | Scaffold includes `clusters/` plus a sample cluster used by the sample graph. Project resolution automatically adds `project_root/clusters` to loader search paths. | Codex | OPEN |
| EI-8 | Implement in-process custom primitive registration | Scaffolded Rust crate registers user primitives through `CatalogBuilder` / SDK builder according to `custom-implementation-loading.md`, with matching tests. | Codex | OPEN |
| EI-9 | Test: scaffolded project builds and runs | Init a project, build it as a Rust crate, run a fixture-backed profile through the SDK path, and verify capture output in `captures/`. | Codex | OPEN |
| EI-10 | Test: project validation catches composition errors | Project with mismatched adapter/graph. Validation reports typed error with rule ID. | Codex | OPEN |
| EI-11 | Test: project profile resolves egress config | Profile that references an `egress/*.toml` file resolves and passes parsed `EgressConfig` into the host run path. SDK path is implemented; scaffolded-project proof remains open. | Codex | OPEN |
| EI-12 | Documentation | User-facing guide: "Getting Started with Ergo SDK." Covers init, custom primitive registration, graphs, clusters, adapters, channels, profiles, running, validation, and replay. | Claude | OPEN |
<!-- markdownlint-restore -->

## Design Constraints

- The project layout is a convention, not a hard requirement.
  Path-based CLI usage continues to work.
- EI-8 must follow the selected in-process mechanism from
  `decisions/custom-implementation-loading.md`.
- The v1 project model supports one ingress channel per run profile.
  Projects needing multiple live sources use a multiplexer ingress
  channel upstream of host.
- `ergo.toml` references standalone egress TOML files; it does not
  redefine `EgressConfig`.
- The primary production path is SDK-first. CLI project-mode commands,
  if added, are convenience wrappers rather than the defining product
  surface.
- `Cargo.toml` and `ergo.toml` serve different purposes and must remain
  separate.
- No domain-specific language in project scaffolding. Template files
  use generic examples (`number_source`, `add`, `emit_if_true`), not
  trading examples.
- Clusters are normal authored artifacts in v1, not an advanced or
  deferred feature.
- Project convention is a prod-layer concern shared by SDK plus loader.
  CLI may consume it, but does not define it.
- After this branch, domain-specific vertical work is done by Sebastian
  inside a workspace using the extension surface. It does not appear in
  the Ergo repo.

## What This Branch Enables

After `feat/ergo-init` merges, a developer can:

1. `ergo init my-project`
2. Open a real Rust crate with working SDK dependency and sample
   `main.rs`
3. Write custom primitives in `src/implementations/`
4. Write graphs and clusters in YAML
5. Write an adapter manifest in YAML
6. Write ingress and egress channel programs in the workspace
7. Declare named run profiles in `ergo.toml`
8. `cargo run` or equivalent SDK-driven binary execution to run a
   fixture-backed or live profile
9. Validate project profiles and replay captures through the same
   project model

All domain-specific work lives in the workspace. The Ergo repo provides
the runtime, the contracts, the SDK, and the tooling that scaffold,
load, and execute that workspace.
