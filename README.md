# Ergo

Ergo is a deterministic graph execution engine built in Rust.

It implements a four-primitive ontological model — **Source, Compute, Trigger, Action** — representing universal causal roles. Graphs are authored declaratively in YAML or JSON. Execution is deterministic: same inputs, same decisions, same outputs. Ergo can produce replayable capture artifacts or bundles for audit, debugging, and verification.

Ergo is industry-agnostic by design. The primitives and execution model are domain-free. Verticals are built as user-space projects using the extension surface: custom implementations, graphs, adapters, and boundary channels.

## Quick Start

Install the published prerelease CLI and scaffold a project:

```text
cargo install ergo-cli --version 0.1.0-alpha.2
ergo init my-project
cd my-project
cargo run
```

The current crates.io releases are prereleases. Cargo does not select
prerelease versions for bare requirements, so `cargo install ergo-cli`
and `cargo add ergo-sdk` will not find the alpha line by default. Use
explicit versions until a non-prerelease `0.1.0` ships:

```text
cargo install ergo-cli --version 0.1.0-alpha.2
cargo add ergo-sdk@0.1.0-alpha.1
```

Or write the SDK dependency directly:

```toml
ergo-sdk = "0.1.0-alpha.1"
```

From a local Ergo checkout, the same scaffold flow can be run without
installing the published CLI:

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

Every Ergo project is a Rust crate. Depending on the profile, you may author up to five things:

1. **Implementations** — Source, Compute, Trigger, and Action primitives in Rust
2. **Graphs** — declarative YAML or JSON wiring nodes and edges
3. **Adapters** — optional declarative contracts defining event kinds, context keys, and accepted effects
4. **Ingress channels** — optional programs that bring external data into the engine
5. **Egress channels** — optional programs that realize external effects (e.g., placing orders)

The SDK (`ergo-sdk`) is the primary product surface. The CLI is development tooling.

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
│  SDK  (ergo-sdk)                                   │
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

The v1 architecture is implemented, and the nine public crates are live
on crates.io as alpha prereleases. The initial stack published at
`0.1.0-alpha.1`; `ergo-cli` is currently `0.1.0-alpha.2` with the same
published dependency stack underneath it.

This is an early alpha release, not a stable semver commitment. APIs may
change before the first non-prerelease `0.1.0`.

**Current limits:**

- Default `ergo init` writes `ergo-sdk = "0.1.0-alpha.1"`; use
  `ergo init --sdk-path <path-to-ergo-sdk>` only when developing against
  a local SDK checkout
- One ingress channel per profile
- The generated live sample ingress and egress channels require a local `python3`
- The built `Ergo` handle is same-thread reusable across run, replay, validation, and manual-runner operations

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE).
