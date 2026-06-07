# ergo-sdk

`ergo-sdk` is the primary Rust API for embedding Ergo in an application.
It builds an `Ergo` handle, resolves filesystem or in-memory project profiles,
and delegates run, replay, validation, and manual stepping to the production
host layer.

Most Rust application users should start here. Command-line users should use
the `ergo` binary from `ergo-cli`.

## Install

```toml
[dependencies]
ergo-sdk = "0.1.0-alpha.1"
```

## Minimal filesystem project example

This example assumes the current directory is an Ergo project with an
`ergo.toml` profile named `historical`. That is the fixture-backed profile
created by `ergo init`; in your own project, replace `historical` with a profile
that exists in your `ergo.toml`.

```rust,no_run
use ergo_sdk::{Ergo, RunOutcome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ergo = Ergo::from_project(".").build()?;

    let summary = ergo.validate_project()?;
    eprintln!("validated {} v{}", summary.name, summary.version);

    match ergo.run_profile("historical")? {
        RunOutcome::Completed(run) => println!("events={}", run.events),
        RunOutcome::Interrupted(run) => eprintln!("interrupted: {:?}", run.reason),
    }

    Ok(())
}
```

## What this crate owns

- `Ergo` and `ErgoBuilder`, the SDK-facing engine handle and builder.
- Profile-based run, replay, validation, and manual-step entrypoints.
- In-process custom primitive registration before the runtime surfaces are
  frozen for host execution.
- SDK-owned in-memory project snapshots for callers that do not want to load an
  `ergo.toml` from disk.
- SDK-branded error types that preserve host, loader, and kernel detail without
  making application code depend on every lower layer directly.

## What this crate does not own

- Kernel execution semantics, replay rules, primitive meaning, or adapter
  composition rules.
- Loader discovery and graph decode behavior.
- Host effect routing, egress dispatch, and capture finalization policy.

Those responsibilities live in `ergo-runtime`, `ergo-adapter`,
`ergo-supervisor`, `ergo-loader`, and `ergo-host`.

## Handle and threading model

`Ergo` is a reusable same-thread handle in v1. You can build one handle and run
multiple sequential operations on that same thread. Runs and replays are
synchronous and block the calling thread until the host loop completes.

`Ergo` is not `Send + Sync` in v1. If you need multiple threads today, build a
separate handle per thread or serialize access outside the SDK. `StopHandle` is
the narrow thread-mobile type intended for requesting graceful stop from another
thread while a run is blocked.

## Other entrypoints

- `run(config)` and `replay(config)` are explicit graph/capture paths for callers
  that already have paths instead of named profiles.
- `replay_bundle(ReplayBundleConfig::new(bundle, graph_path))` replays an
  already-loaded in-memory `CaptureBundle` against explicit graph paths.
- `replay_profile_bundle(profile_name, bundle)` replays an already-loaded
  in-memory `CaptureBundle` using a named project profile's graph and adapter
  settings.
- `runner_for_profile(...)` returns a `ProfileRunner` for manual event stepping;
  `finish()` returns a capture bundle and `finish_and_write_capture()` writes
  only when the profile resolved an explicit capture path.

## More information

- SDK getting started guide: [`docs/authoring/getting-started-sdk.md`](../../../../docs/authoring/getting-started-sdk.md)
- Project convention: [`docs/authoring/project-convention.md`](../../../../docs/authoring/project-convention.md)
- Prod layer map: [`crates/prod/CODE_MAP.md`](../../CODE_MAP.md)
