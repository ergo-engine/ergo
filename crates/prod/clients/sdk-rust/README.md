# ergo-sdk-rust

Rust SDK over Ergo host + loader.

`ergo-sdk-rust` is the primary product surface for embedding Ergo into
an application crate. It wraps the canonical host run and replay
paths, supports in-process primitive registration, and resolves
project profiles from either `ergo.toml` or an SDK-owned in-memory
project snapshot.

## What It Owns

- ergonomic builder API
- custom primitive registration
- filesystem project discovery from `ergo.toml` via shared loader resolution
- in-memory project/profile modeling through validated SDK builders
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
use ergo_sdk_rust::{
    Ergo, InMemoryProfileConfig, InMemoryProjectSnapshot, ProfileCapture,
};

let outcome = Ergo::builder()
    .project_root(".")
    .build()?
    .run_profile("historical")?;

let my_assets = /* PreparedGraphAssets loaded or built earlier */;
let in_memory = InMemoryProjectSnapshot::builder("demo", "0.1.0")
    .profile(
        "historical",
        InMemoryProfileConfig::process(
            my_assets,
            ["python3", "channels/ingress/feed.py"],
        )?
        .capture(ProfileCapture::in_memory()),
    )
    .build()?;

let explicit = Ergo::builder()
    .in_memory_project(in_memory)
    .build()?
    .run_profile("historical")?;
```

The public doctrine is:

- `.project_root(...)` is the canonical filesystem project lane.
- `.in_memory_project(...)` is the canonical SDK in-memory project lane.
- Both must delegate to the same host-owned orchestration model.
- The SDK may widen product transport, but it must not invent a second
  execution authority.

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
  resolved file-capture settings.

## Transport-Neutral Replay

`replay(config)` and `replay_profile(profile_name, capture_path)` stay
file-backed.

`replay_bundle(...)` and `replay_profile_bundle(...)` are the
transport-neutral replay entrypoints when the caller already owns a
`CaptureBundle`.

## Doctrine

- `SDK-CANON-1`: canonical run/replay delegate to `ergo-host`
- `SDK-CANON-2`: filesystem loader resolution and SDK in-memory project
  resolution must both translate into host-owned canonical requests,
  not invent a second execution model
- `SDK-CANON-3`: custom primitives register in-process through the same
  runtime validation path as core primitives
