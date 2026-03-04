# ergo-sdk-rust

Thin Rust client SDK over product core crates.

## Doctrine
- `SDK-CANON-1`: delegate canonical run/replay to `crates/prod/core/host`.
- `SDK-CANON-2`: perform no semantic validation/policy.
- `SDK-CANON-3`: replay goes through host strict replay path.
- `SDK-CANON-4`: effect application remains host-owned.

## Current Status
- Scaffolded crate with minimal API surface.
- Depends on `ergo-loader` + `ergo-host` for orchestration boundaries.
