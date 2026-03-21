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
    .run_profile("historical")?;

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

`Ergo` is a same-thread reusable engine handle. `run`, `run_profile`,
`replay`, `replay_profile`, and `validate_project` borrow the built
handle, so one `Ergo` value can execute multiple operations.

Reusing one handle also reuses the same registered primitive instances
behind it under the current in-process trust model.

## Manual Stepping

`runner_for_profile(...)` resolves a normal run profile and returns a
low-level `ProfileRunner` for manual stepping.

- It honors `graph`, cluster search paths, `adapter`, and `egress`.
- Profile resolution still requires exactly one ingress source today,
  even though manual stepping does not launch that ingress.
- It ignores profile `ingress`, `max_duration`, and `max_events`
  during manual stepping.
- `finish()` returns a `CaptureBundle` and does not write
  `capture_output`.
- `finish_and_write_capture()` is the call that explicitly applies the
  resolved `capture_output` / `pretty_capture` settings.

## Doctrine

- `SDK-CANON-1`: canonical run/replay delegate to `ergo-host`
- `SDK-CANON-2`: project resolution must not invent a second execution model
- `SDK-CANON-3`: custom primitives register in-process through the same
  runtime validation path as core primitives
