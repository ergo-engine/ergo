---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-19
Owner: Sebastian (Architect)
Scope: Practical testing notes for SDK-first Ergo projects
Change Rule: Tracks implementation
---

# Testing Notes

This document records practical testing notes for the current v1 Ergo
project flow.

Use it when you want to know what has been exercised end to end, what
is working in a real project, and which rough edges are current product
limitations rather than architecture gaps.

## 1. What Was Tested

The following path has been exercised in a scaffolded SDK-first project:

- `ergo init`
- `cargo run`
- `cargo run -- validate`
- `cargo run -- replay ...`
- real process ingress
- real process egress
- capture writing for completed and graceful interrupted runs
- strict replay of generated captures

The tested project shape was the standard v1 scaffold:

- Rust crate via `Cargo.toml`
- Ergo project manifest via `ergo.toml`
- custom primitives in `src/implementations/`
- graph YAML in `graphs/`
- cluster YAML in `clusters/`
- adapter manifest in `adapters/`
- ingress and egress channel programs in `channels/`

## 2. Verified Working

### Scaffolded project flow

The scaffolded SDK-first project is runnable as generated:

- project validates cleanly
- fixture-backed historical runs complete
- replay of generated capture succeeds

This proves the current v1 stack works across:

- SDK builder
- shared project/profile resolution
- cluster discovery/loading
- adapter-bound execution
- capture production
- replay verification

### Real ingress

A real OANDA `EUR_USD` ingress channel was exercised through a
generated project.

Verified:

- process ingress starts cleanly
- OANDA pricing stream events normalize into Ergo event payloads
- adapter-bound live profile can consume the stream

Expected environment variables:

- `OANDA_API_TOKEN` or `OANDA_API_KEY`
- `OANDA_ACCOUNT_ID`
- optional `OANDA_ENV=practice|live`
- optional `OANDA_INSTRUMENT=EUR_USD`

### Real strategy path

A first real paper-trading strategy was exercised in the scaffolded
project:

- spread-filtered fast/slow moving-average crossover
- persisted `mid_history`
- persisted `last_signal_code`
- emitted `paper_signal` intents
- durable egress acknowledgements

The tested fixture path produced real signal flips and replay-stable
captures.

### Live runtime stability

The live OANDA-backed paper strategy was run for five minutes against
the practice environment without:

- startup failure
- protocol failure
- egress failure
- runtime crash

That proves the current live streaming path is operational for a real
SDK-built project.

### Graceful stop path

The scaffolded SDK-first project now installs `Ctrl-C` handling through
`StopHandle` and runs profiles through `run_profile_with_stop(...)`.

That proves the current app/SDK/host path can:

- stop a long-running profile cleanly
- finalize the run instead of hard-killing it
- write a replayable capture when at least one event was committed

## 3. Current Product Limitations

### Sample live channels are still simple examples

The scaffolded live boundary programs are still starter examples:

- ingress and egress samples assume a local `python3`
- egress sample is a simple durable-ack outbox
- live order placement is not scaffolded by default

Projects are expected to replace those examples while keeping the same
manifest/layout model.

## 4. Practical Advice

- Treat the scaffold as a real starting point, not a toy.
- Use fixture profiles first, then replay, then live ingress.
- Start with paper-signal or paper-order strategies before wiring real
  broker execution.
- Rotate any live credentials used during manual testing.

## 5. Development Friction Encountered

The following friction points showed up while turning a scaffolded
project into a real OANDA-backed paper strategy.

### Cluster files resolve by filename

Cluster discovery currently resolves by filename such as
`clusters/<cluster_id>.yaml`, not only by the YAML `id` field.

That means a cluster with:

- `id: moving_average_signal`

must currently live at a matching path such as:

- `clusters/moving_average_signal.yaml`

even if the YAML body itself is correct.

### Adapter event schema gates persisted context merge

In adapter-bound runs, stored context keys are only merged back into the
next event payload when those keys are allowed by the event schema.

This matters for stateful strategies. Persisted keys such as:

- `mid_history`
- `last_signal_code`

must be present as optional properties in the event schema if later
evaluation should see them again.

### Egress ready handshake is strict about handled kinds

The host validates the egress `ready` frame against the effect kinds a
profile actually routes.

If a sample channel advertises the wrong handled kind, startup fails
before the run begins. That is correct behavior, but it is an easy
integration snag while evolving a sample strategy into a real one.

### Current SDK/project ergonomics still have a few rough edges

- Outside the Ergo checkout, `ergo init` still needs `--sdk-path`
  until `ergo-sdk-rust` is published.
- Scaffolded live sample channels assume `python3` is available.
- The built `Ergo` handle is currently one-shot.

## 6. Related Docs

- [Getting Started with Ergo SDK](getting-started-sdk.md)
- [Project Convention](project-convention.md)
- [Ingress Channel Guide](ingress-channel-guide.md)
- [Egress Channel Guide](egress-channel-guide.md)
- [Current Architecture](../system/current-architecture.md)
