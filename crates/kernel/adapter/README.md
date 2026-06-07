# ergo-adapter

`ergo-adapter` is the kernel-owned adapter contract crate. It defines adapter
manifests, event binding, source/action composition checks, adapter provenance,
capture helpers, and runtime-facing external event types.

Most users should start with `ergo-sdk` for embedded Rust usage or
`ergo-cli` for command-line usage. Depend on this crate directly only when you
are building lower-level Ergo adapter tooling or integrating with kernel-facing
adapter surfaces.

## What this crate owns

- Adapter manifest data structures and validation entrypoints.
- Source/adapter and action/adapter composition checks.
- Event binding from external adapter events into runtime context and payloads.
- Adapter provenance fingerprinting via `adapter_fingerprint(...)`.
- Runtime invoker handles and event/context wrapper types used at the
  supervisor/runtime boundary.

## What this crate does not own

- Runtime primitive semantics, graph validation, or execution physics; those are
  owned by `ergo-runtime`.
- Episode scheduling, capture-bundle ownership, and replay policy; those are
  owned by `ergo-supervisor`.
- Host orchestration, process-channel lifecycle, and product-facing error text;
  those are prod-layer responsibilities.

## More information

- Adapter contract: [`docs/orchestration/adapter.md`](../../../docs/orchestration/adapter.md)
- Kernel layer map: [`crates/kernel/CODE_MAP.md`](../CODE_MAP.md)
- Canonical system docs start at [`docs/INDEX.md`](../../../docs/INDEX.md)
