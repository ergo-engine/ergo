# ergo-host

`ergo-host` is the production host orchestration layer. It composes loader,
adapter, supervisor, runtime, egress, and capture surfaces into the canonical
run, replay, validation, and manual-step use cases consumed by the SDK and CLI.

Most users should start with `ergo-sdk-rust` for embedded Rust usage or
`ergo-cli` for command-line usage. Depend on this crate directly only for
advanced host integration or tooling that needs lower-level orchestration
surfaces.

## What this crate owns

- Canonical host run, replay, validation, and hosted-runner preparation use
  cases.
- Driver protocol boundaries for fixture and process ingress.
- Host-side effect routing, handler coverage, egress config/runtime, and egress
  dispatch.
- Production adapter gating and host-shaped error descriptors.
- Capture finalization and host-side capture enrichment around the kernel
  `CaptureBundle`.

## What this crate does not own

- Kernel primitive semantics, runtime validation, supervisor replay equality, or
  capture format shape.
- Loader transport/decode/discovery rules and sealed graph asset construction.
- CLI command grammar or SDK handle ergonomics.

## Public surface shape

The crate exposes canonical client-facing use cases for SDK/CLI callers plus
lower-level building blocks for advanced embedders and tests. The tiering is
described in the prod CODE_MAP; this README is only a crates.io landing page.

## More information

- Host boundary: [`docs/system/host-boundary.md`](../../../../docs/system/host-boundary.md)
- Kernel/prod separation: [`docs/system/kernel-prod-separation.md`](../../../../docs/system/kernel-prod-separation.md)
- Prod layer map: [`crates/prod/CODE_MAP.md`](../../CODE_MAP.md)