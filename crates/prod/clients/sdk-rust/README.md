# ergo-sdk-rust

Rust SDK over Ergo host + loader.

`ergo-sdk-rust` is the primary product surface for embedding Ergo into
an application crate. It wraps the canonical host run and replay
paths, supports in-process primitive registration, and resolves
project profiles from `ergo.toml`.

## What It Owns

- ergonomic builder API
- custom primitive registration
- project discovery from `ergo.toml` via shared loader resolution
- profile execution, replay, and validation

## What It Does Not Own

- execution semantics
- replay policy
- adapter composition rules
- host effect routing

Those remain owned by `ergo-host`, `ergo-loader`, `ergo-runtime`, and
the canonical docs under `/docs`.

## Example

```rust
use ergo_sdk_rust::{Ergo, IngressConfig, RunConfig};

let outcome = Ergo::builder()
    .project_root(".")
    .build()?
    .run_profile("backtest")?;

let explicit = Ergo::builder()
    .build()?
    .run(
        RunConfig::new(
            "graphs/strategy.yaml",
            IngressConfig::process(["python3", "channels/ingress/feed.py"]),
        )
        .egress_config("egress/live.toml"),
    )?;
```

## Current Handle Semantics

`Ergo` is currently a one-shot engine handle. `run`, `run_profile`,
`replay`, `replay_profile`, and `validate_project` consume the built
handle, so build a fresh `Ergo` value for each operation.

A reusable engine handle is planned as a future ergonomics improvement,
but it is not part of the initial SDK surface.

## Doctrine

- `SDK-CANON-1`: canonical run/replay delegate to `ergo-host`
- `SDK-CANON-2`: project resolution must not invent a second execution model
- `SDK-CANON-3`: custom primitives register in-process through the same
  runtime validation path as core primitives
