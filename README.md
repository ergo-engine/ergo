# Ergo

Ergo is a deterministic graph execution engine built in Rust.

It implements a four-primitive ontological model — **Source, Compute, Trigger, Action** — representing universal causal roles. Graphs are declarative YAML. Execution is deterministic: same inputs, same decisions, same outputs. Every run produces a capture artifact that can be replayed for audit, debugging, or verification.

Ergo is industry-agnostic by design. The primitives and execution model are domain-free. Verticals are built as user-space projects using the extension surface: custom implementations, graphs, adapters, and boundary channels.

## Quick Start

Scaffold a new project inside an Ergo checkout:

```text
cargo run -p ergo-cli -- init my-project
cd my-project
cargo run
```

This runs the `historical` profile from the scaffolded `ergo.toml`, producing a capture file in `captures/`. Validate, replay, and explore:

```text
cargo run -- validate
cargo run -- replay historical captures/historical.capture.json
```

See the full walkthrough in [Getting Started with Ergo SDK](docs/authoring/getting-started-sdk.md).

## What You Author

Every Ergo project is a Rust crate. You write five things:

1. **Implementations** — Source, Compute, Trigger, and Action primitives in Rust
2. **Graphs** — declarative YAML wiring nodes and edges
3. **Adapters** — declarative contracts defining event kinds, context keys, and accepted effects
4. **Ingress channels** — programs that bring external data into the engine
5. **Egress channels** — programs that realize external effects (e.g., placing orders)

The SDK (`ergo-sdk-rust`) is the primary product surface. The CLI is development tooling.

## Project Layout

A scaffolded Ergo project:

```text
my-project/
  Cargo.toml          # Rust build config
  ergo.toml           # Ergo profiles and asset wiring
  src/
    main.rs
    implementations/  # Custom primitives
  graphs/             # Graph YAML
  clusters/           # Reusable sub-graphs
  adapters/           # Adapter contracts
  channels/
    ingress/          # Data-in boundary programs
    egress/           # Effect-out boundary programs
  egress/             # Egress routing config
  fixtures/           # Deterministic test data
  captures/           # Run output artifacts
```

## Architecture

```text
┌─────────────────────────────────────────────────────────┐
│  User Project (Rust crate)                              │
│  implementations, graphs, adapters, channels            │
├─────────────────────────────────────────────────────────┤
│  SDK  (ergo-sdk-rust)                                   │
│  builder API, profile resolution, primitive registration│
├─────────────────────────────────────────────────────────┤
│  Host  (ergo-host + ergo-loader)                        │
│  run orchestration, driver protocol, egress dispatch,   │
│  project/graph/cluster loading                          │
├─────────────────────────────────────────────────────────┤
│  Kernel  (runtime + adapter + supervisor)    [frozen]   │
│  graph evaluation, adapter validation, capture/replay   │
└─────────────────────────────────────────────────────────┘
     ▲ ingress                              egress ▼
     │ (process channel)            (process channel) │
     └── external data in            external effects out ─┘
```

Control flows top-down: the user project calls the SDK, which delegates to the host, which drives the kernel. Data flows in through ingress channels and out through egress channels. The kernel is semantically frozen — new features are built in the host/SDK layer or in user-space projects.

## Crate Structure

```text
crates/
  kernel/
    runtime/          # Graph evaluation, topology, scheduling
    adapter/          # Adapter validation and event binding
    supervisor/       # Capture, replay, and decision logging
  prod/
    core/
      host/           # Run orchestration, driver protocol, egress dispatch
      loader/         # Project resolution, graph/cluster loading
    clients/
      cli/            # ergo CLI (init, run, validate, replay)
      sdk-rust/       # Rust SDK — primary product surface
      sdk-types/      # Shared type definitions
  shared/
    fixtures/         # Test fixture utilities
    test-support/     # Test infrastructure
```

## Documentation

Documentation lives in `docs/` with a single-source rule: every fact has exactly one authoritative location. Start with [docs/INDEX.md](docs/INDEX.md).

**Read in order if you're new:**

1. [Kernel](docs/system/kernel.md) — what "kernel" and "closed" mean
2. [Current Architecture](docs/system/current-architecture.md) — the v1 system
3. [Ontology](docs/system/ontology.md) — the four primitives
4. [Execution](docs/system/execution.md) — how graphs evaluate
5. [Getting Started](docs/authoring/getting-started-sdk.md) — scaffold, run, edit

**Key references:**

- [Project Convention](docs/authoring/project-convention.md) — project shape and `ergo.toml`
- [Ingress Channel Guide](docs/authoring/ingress-channel-guide.md) — writing data-in channels
- [Egress Channel Guide](docs/authoring/egress-channel-guide.md) — writing effect-out channels
- [Terminology](docs/system/terminology.md) — canonical terms

## Current State

Ergo v1 is shipped. The engine, SDK, CLI, and scaffold are all live. The first vertical (EUR/USD moving-average crossover trading system) is built and running as a user-space project against the extension surface.

**Current limits:**

- The SDK dependency is local-checkout based until the crate is published
- One ingress channel per profile
- The `Ergo` handle is one-shot (build a fresh value per operation)

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE).
