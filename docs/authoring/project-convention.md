---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-16
Owner: Sebastian (Architect)
Scope: SDK-first Ergo project convention, crate layout, and profile model
Change Rule: Tracks implementation
---

# Project Convention

This document explains the current v1 Ergo project model.

The primary product surface is the Rust SDK. A production Ergo
application is a Rust crate that depends on `ergo-sdk-rust`,
registers custom primitives in-process, and runs named profiles from
`ergo.toml`.

The CLI remains a support tool for validation, replay, fixture runs,
and other development conveniences. It is not the defining production
surface.

## 1. Two Manifests, Two Jobs

An Ergo project has two top-level manifests:

- `Cargo.toml`
  Rust build, dependencies, and binary/library configuration.
- `ergo.toml`
  Ergo project profiles, graph/adapter/channel wiring, and capture
  defaults.

These must stay separate.

`Cargo.toml` answers:

- how the Rust crate is built
- which crates it depends on
- which binary is executed

`ergo.toml` answers:

- which graph a profile runs
- which adapter a profile binds
- which ingress source a profile uses
- which egress config a profile references
- where capture output should go

## 2. What A Project Contains

The v1 project layout is:

```text
my-project/
├── Cargo.toml
├── ergo.toml
├── src/
│   ├── main.rs
│   └── implementations/
├── graphs/
├── clusters/
├── adapters/
├── channels/
│   ├── ingress/
│   └── egress/
├── egress/
├── fixtures/
└── captures/
```

Purpose of each area:

- `src/main.rs`
  User-owned application entrypoint that builds an Ergo engine through
  the SDK.
- `src/implementations/`
  Custom Source, Compute, Trigger, and Action implementations
  registered through `CatalogBuilder` / SDK builder.
- `graphs/`
  Graph YAML entrypoints.
- `clusters/`
  Reusable cluster definitions. These are first-class authored
  artifacts and are discovered automatically by project resolution.
- `adapters/`
  Adapter manifests defining accepted event/effect contracts.
- `channels/ingress/`
  User-authored ingress channel programs.
- `channels/egress/`
  User-authored egress channel programs.
- `egress/`
  Standalone `EgressConfig` TOML files referenced by profiles.
- `fixtures/`
  Deterministic input event streams.
- `captures/`
  Replay artifacts produced by runs.

## 3. SDK-First Entry

The intended ergonomic shape is:

```rust
let ergo = Ergo::builder()
    .project_root(".")
    .add_source(MySource::new())
    .add_action(MyAction::new())
    .build()?;

let outcome = ergo.run_profile("live")?;
```

That means:

- user code owns primitive registration
- project/profile resolution is shared loader infrastructure consumed
  by the SDK now and available for future CLI convenience paths
- canonical execution still delegates to host
- replay still delegates to host strict replay

The SDK should wrap host + loader ergonomically. It should not invent a
second execution model.

For the initial SDK surface, the built `Ergo` handle is one-shot:
`run`, `run_profile`, `replay`, and validation operations consume it.
Projects should build a fresh handle per operation for now. A reusable
engine handle is expected later as an ergonomics improvement, not a v1
prerequisite.

## 4. Profile Model

Profiles live in `ergo.toml`.

Each profile resolves:

- `graph`
- `adapter`
- exactly one ingress source:
  - `fixture`, or
  - `ingress` process command
- optional `egress` config path
- optional capture output override

The project `clusters/` directory is always added to cluster search
paths automatically. Users should not repeat it in every profile.

One ingress channel per profile is the v1 limit. If a project needs
multiple live feeds, it must multiplex them upstream into one ingress
channel.

## 5. Custom Implementations

Custom implementations use the same trait surface as stdlib
primitives.

The v1 loading mechanism is **in-process Rust crate registration**
through `CatalogBuilder`. That means:

- no dynamic library loading
- no WASM loading in v1
- no separate runtime plugin boundary

The project binary links the user’s primitives directly and registers
them before run/validation/replay surfaces are built.

See:

- [Custom Implementation Loading Decision](../ledger/decisions/custom-implementation-loading.md)
- [Action Primitive Manifest](../primitives/action.md)

## 6. Clusters Are Normal

Clusters are not an advanced or deferred feature.

They are already part of the live runtime/loader path:

- loader discovers cluster files from search paths
- host loads the cluster tree before expansion
- runtime expands clusters away before execution

So a scaffolded project should include:

- a sample cluster in `clusters/`
- a sample graph in `graphs/` that references that cluster

## 7. Relationship To CLI

The CLI still matters, but as supporting tooling:

- validate
- replay
- fixture runs
- optional project-mode convenience commands

The CLI may consume the same project-resolution surface as the SDK, but
it does not define the product model.

## 8. Companion Docs

- [Current Architecture](../system/current-architecture.md)
- [Loader Contract](loader.md)
- [Ingress Channel Guide](ingress-channel-guide.md)
- [Egress Channel Guide](egress-channel-guide.md)
- [Ergo Init Ledger](../ledger/dev-work/open/ergo-init.md)
