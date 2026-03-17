---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-16
Owner: Sebastian (Architect)
Scope: Getting started workflow for SDK-first Ergo projects
Change Rule: Tracks implementation
---

# Getting Started with Ergo SDK

This guide walks through the current v1 Ergo project workflow.

Use this when you want to create a new Ergo application, run the sample
project, and understand where to put your code and authored assets.

## 1. Scaffold a Project

From inside an Ergo checkout, create a new Rust crate plus Ergo
project layout:

```text
ergo init my-project
```

`ergo init` generates:

- `Cargo.toml`
- `ergo.toml`
- `src/main.rs`
- `src/implementations/`
- `graphs/`
- `clusters/`
- `adapters/`
- `channels/ingress/`
- `channels/egress/`
- `egress/`
- `fixtures/`
- `captures/`

The generated `Cargo.toml` currently points at a local
`ergo-sdk-rust` checkout.

- Inside the Ergo repository, `ergo init` wires that dependency
  automatically with a relative path.
- Outside the repository, pass `--sdk-path <path-to-ergo-sdk-rust>`
  until the SDK is published outside the repo.

## 2. Run the Sample Project

The scaffolded crate is runnable immediately:

```text
cd my-project
cargo run
```

This default path runs the `backtest` profile from `ergo.toml`:

- graph: `graphs/strategy.yaml`
- cluster search path: implicit `clusters/`
- adapter: `adapters/sample.yaml`
- ingress source: `fixtures/backtest.jsonl`
- egress config: `egress/live.toml`
- capture output: `captures/backtest.capture.json`

The sample run proves the full SDK path:

- in-process custom primitive registration
- project/profile resolution
- cluster loading
- adapter-bound execution
- external intent dispatch
- capture output

Current sample boundary programs are POSIX examples:

- `channels/ingress/live_feed.sh`
- `channels/egress/sample_outbox.sh`

If you are not running on a POSIX shell environment, replace those
sample channel programs and the corresponding commands in
`ergo.toml` / `egress/live.toml` before using the live profile.

## 3. Validate and Replay

Validate every named profile in the project:

```text
cargo run -- validate
```

Replay the generated backtest capture:

```text
cargo run -- replay backtest captures/backtest.capture.json
```

The scaffolded `main.rs` is intentionally small. It shows how to:

- build `Ergo` from the project root
- register custom primitives
- run one named profile
- validate the project
- replay a capture

## 4. Where To Edit Things

### Custom primitives

Edit:

- `src/implementations/sources.rs`
- `src/implementations/actions.rs`

The sample project includes:

- a custom Source primitive that emits a string message
- a custom Action primitive that emits an external intent plus a mirror
  write

Add your own Source, Compute, Trigger, and Action implementations here,
then register them in `src/main.rs`.

### Graphs and clusters

Edit:

- `graphs/strategy.yaml`
- `clusters/sample_message.yaml`

The sample graph already references the sample cluster, so clusters are
part of the normal project flow from day one.

### Adapter contract

Edit:

- `adapters/sample.yaml`

This is where event kinds, context keys, accepted effect kinds, and
capture fields are declared.

### Boundary channels

Edit:

- `channels/ingress/live_feed.sh`
- `channels/egress/sample_outbox.sh`
- `egress/live.toml`

The sample project includes both ingress and egress channel programs.
The backtest profile uses fixture ingress. The live profile uses the
process ingress script plus the same egress routing config.

## 5. Manifest Split

Keep the two top-level manifests separate:

- `Cargo.toml`
  Rust build and dependency configuration
- `ergo.toml`
  Ergo project profiles and authored-asset wiring

`ergo.toml` is the project authority for:

- graph path
- adapter path
- exactly one ingress source per profile
- optional egress config path
- capture output

## 6. Current v1 Limits

- One ingress channel per profile.
- The SDK handle is currently one-shot; build a fresh `Ergo` value per
  run, validation, or replay operation.
- CLI remains supporting tooling. The production surface is the Rust
  crate you scaffold and run with Cargo.

## 7. Read Next

- [Project Convention](project-convention.md)
- [Loader Contract](loader.md)
- [Ingress Channel Guide](ingress-channel-guide.md)
- [Egress Channel Guide](egress-channel-guide.md)
- [Action Primitive Manifest](../primitives/action.md)
