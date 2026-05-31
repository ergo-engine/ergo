# ergo-sdk-types

`ergo-sdk-types` contains lightweight serializable types intended to be shared
across Ergo SDK clients and bindings.

Today the public surface is intentionally small: it exports `SdkVersion`.

Most Rust application users should depend on `ergo-sdk-rust` instead. Use this
crate directly only when you need the shared transport/version DTOs without the
Rust SDK engine.

## What this crate owns

- Small serde-compatible SDK-facing data types.
- Cross-binding shapes that should remain easy to serialize and mirror outside
  Rust.

## What this crate does not own

- Runtime semantics, graph loading, project validation, or host orchestration.
- CLI behavior or SDK engine ergonomics.
- Kernel/prod boundary rules.

## More information

- Prod layer map: [`crates/prod/CODE_MAP.md`](../../CODE_MAP.md)
