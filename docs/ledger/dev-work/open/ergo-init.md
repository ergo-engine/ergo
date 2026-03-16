---
Authority: PROJECT
Date: 2026-03-16
Author: Sebastian (Architect) + Codex (Build Orchestrator)
Status: OPEN
Branch: feat/ergo-init
Tier: 3 (Developer Experience — the gate)
Depends-On: >-
  feat/catalog-builder, feat/adapter-runtime, feat/egress-surface;
  docs/ledger/gap-work/open/custom-implementation-loading.md (for EI-8);
  docs/ledger/gap-work/open/effect-realization-boundary.md
  (GW-EFX-2 constrains v1 to one ingress channel per run)
---

# Ergo Workspace Scaffolding and Project Conventions

## Scope

Define the v1 workspace convention for how a developer creates,
organizes, and runs an Ergo project.

After this branch, domain work happens inside an Ergo workspace rather
than inside `crates/kernel/` or `crates/prod/`. That workspace must
give users a clear home for:

- implementations
- graphs
- adapters
- ingress channels
- egress channels
- fixtures
- captures
- `ergo.toml`

This branch owns workspace convention and project-mode CLI ergonomics.
It does not redefine runtime or host semantics, and it does not decide
the mechanism for custom implementation loading. `GW-EI8-1` remains the
gate for that specific loading mechanism.

## Current State

There is still no project convention. The CLI operates on individual
file paths and explicit run flags:

- `ergo run <graph.yaml> --fixture <fixture.jsonl> --adapter <adapter.yaml>`
- `ergo run <graph.yaml> --driver-cmd <program> [--driver-arg
  <value> ...] --adapter <adapter.yaml> [--egress-config
  <egress.toml>]`
- `ergo validate <graph.yaml>`
- `ergo replay <capture.json>`

Users must know which flags to pass, where their files are, and how to
wire ingress and egress explicitly. There is no project discovery, no
workspace manifest, and no scaffolding.

Path-based CLI usage remains valid after this branch. `feat/ergo-init`
adds a project-mode surface; it does not delete the current explicit
path surface.

## V1 Stance

The v1 workspace convention should lock the following scope now:

- One ingress channel per run profile.
- If a user needs multiple live feeds, they must multiplex them
  upstream into one ingress channel until `GW-EFX-2` is decided.
- `ergo.toml` owns project and profile resolution.
- `ergo.toml` references a standalone egress TOML file rather than
  embedding the route table in v1.
- Workspace convention belongs to the CLI plus prod loader surface.
  The SDK may consume the same project model later, but it is not the
  authority for workspace layout.

## Project Layout Convention

The v1 layout should be concrete, not aspirational:

```text
my-project/
├── ergo.toml
├── graphs/
│   └── strategy.yaml
├── clusters/
│   └── math.yaml
├── adapters/
│   └── strategy.yaml
├── implementations/
│   └── src/
│       └── lib.rs
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

- `graphs/` contains graph YAML entrypoints.
- `clusters/` contains reusable cluster definitions and is added to the
  loader search path automatically.
- `adapters/` contains adapter manifests.
- `implementations/` contains user-owned custom implementation code.
  Loading mechanism remains gated by `GW-EI8-1`.
- `channels/ingress/` contains user-authored ingress channel programs.
- `channels/egress/` contains user-authored egress channel programs.
- `egress/` contains standalone `EgressConfig` TOML files referenced by
  `ergo.toml`.
- `fixtures/` contains deterministic input event streams.
- `captures/` contains replay artifacts produced by runs.

## `ergo.toml` As Project Authority

The project manifest should do more than name the project. In v1 it
must define named run profiles that resolve the authored artifacts into
the inputs the current host already understands.

Minimum project fields:

- `name`
- `version`
- `profiles.<name>`

Each profile should resolve:

- `graph`
- `adapter`
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
- `egress` is optional, but when present it points to the existing
  standalone TOML surface chosen in
  `decisions/egress-routing-config.md`
- custom implementation discovery remains conventional from
  `implementations/` once `GW-EI8-1` lands; it does not need a
  per-profile field in v1

## CLI Commands

<!-- markdownlint-disable MD013 -->
| Command | Description |
| ------- | ----------- |
| `ergo init` | Scaffold a new project with the standard layout |
| `ergo run <profile>` (inside project) | Resolve one named project profile into graph, adapter, one ingress source, and optional egress config. |
| `ergo validate` (inside project) | Validate all named profiles, including graph/adapter composition and referenced egress config parsing when present. |
| `ergo replay <capture>` (inside project) | Replay a capture artifact resolved relative to the project root or `captures/`. |
<!-- markdownlint-restore -->

Existing path-based commands continue to work for non-project usage.

## Discovery And Resolution Logic

When the CLI runs inside a project directory:

1. Discover project root by locating `ergo.toml`.
2. Resolve all manifest-relative paths from that root.
3. Add `clusters/` to loader search paths automatically.
4. Resolve a named profile into current host inputs:
   - graph path
   - adapter path
   - one ingress source (`fixture` or process ingress command)
   - optional egress config path parsed into `EgressConfig`
5. Load custom implementations from `implementations/` once the
   `GW-EI8-1` mechanism is approved and implemented.
6. Pass the resolved project profile into the existing host run/replay
   surfaces rather than inventing a second execution model.

In other words:

- CLI scaffolds and discovers the workspace
- prod loader resolves `ergo.toml`
- host executes the resolved profile
- SDK may consume the same resolved project model later, but it should
  not define the workspace convention

## Closure Ledger

<!-- markdownlint-disable MD013 -->
| ID | Task | Closure Condition | Owner | Status |
| -- | ---- | ----------------- | ----- | ------ |
| EI-1 | Define project layout convention | Layout documented as the concrete v1 workspace convention, including `channels/ingress`, `channels/egress`, `egress`, `fixtures`, and `captures`. Reviewed by Sebastian. | Claude + Sebastian | OPEN |
| EI-2 | Define `ergo.toml` schema | Project manifest includes named profiles that resolve graph, adapter, exactly one ingress source, optional egress config path, and optional capture output. | Codex | OPEN |
| EI-3 | Implement `ergo init` | Command creates directory structure with template files. | Codex | OPEN |
| EI-4 | Implement project discovery | CLI detects `ergo.toml`, resolves project root, and loads named profiles from the manifest. | Codex | OPEN |
| EI-5 | `ergo run` inside project | `ergo run <profile>` resolves one named profile into graph + adapter + fixture or ingress command + optional egress config, then runs through the existing host path. | Codex | OPEN |
| EI-6 | `ergo validate` inside project | Validates every named profile, including graph/adapter composition and referenced egress config parsing when present. Reports all errors, not just first. | Codex | OPEN |
| EI-7 | Cluster search path integration | `clusters/` directory automatically added to loader search paths during project runs. | Codex | OPEN |
| EI-8 | Custom implementation loading | Implementations from `implementations/` are loaded via `feat/catalog-builder` API using the mechanism approved in `GW-EI8-1`, with matching tests. | Codex | OPEN |
| EI-9 | Test: `ergo init` + `ergo run` round-trip | Init a project, place a graph and fixture, run it, verify capture output in `captures/`. | Codex | OPEN |
| EI-10 | Test: project validation catches composition errors | Project with mismatched adapter/graph. `ergo validate` reports typed error with rule ID. | Codex | OPEN |
| EI-11 | Test: project profile resolves egress config | Profile that references an `egress/*.toml` file resolves and passes parsed `EgressConfig` into the host run path. | Codex | OPEN |
| EI-12 | Documentation | User-facing guide: "Getting Started with Ergo." Covers init, authoring graphs/adapters/channels, running profiles, replaying, and current single-ingress v1 scope. | Claude | OPEN |
<!-- markdownlint-restore -->

## Design Constraints

- The project layout is a convention, not a hard requirement.
  Path-based CLI usage continues to work.
- Custom implementation loading (EI-8) is gated by `GW-EI8-1`;
  implementation must follow the selected mechanism and documented
  non-goals.
- The v1 project model supports one ingress channel per run profile.
  Multi-ingress remains outside scope until `GW-EFX-2` lands.
- `ergo.toml` references standalone egress TOML files; it does not
  redefine `EgressConfig`.
- No domain-specific language in project scaffolding. Template files
  use generic examples (`number_source`, `add`, `emit_if_true`), not
  trading examples.
- Project convention is a prod-layer concern. Workspace scaffolding and
  discovery belong to CLI plus loader, not the SDK.
- After this branch, domain-specific vertical work is done by Sebastian
  inside a workspace using the extension surface. It does not appear in
  the Ergo repo.

## What This Branch Enables

After `feat/ergo-init` merges, a developer can:

1. `ergo init my-project`
2. Write graphs and clusters in YAML
3. Write an adapter manifest in YAML
4. Write ingress and egress channel programs in the workspace
5. Optionally write custom implementations in Rust once `EI-8` lands
6. Declare named run profiles in `ergo.toml`
7. `ergo validate` to check profile compositions
8. `ergo run <profile>` to execute fixture-backed or live profiles
9. `ergo replay` to verify determinism from captures

All domain-specific work lives in the workspace. The Ergo repo provides
the runtime, the contracts, and the tooling that scaffold, load, and
execute that workspace.
