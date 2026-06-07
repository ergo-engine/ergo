# ergo-supervisor

`ergo-supervisor` is the kernel supervisor crate for deterministic episode
scheduling, decision logging, capture, and replay over a `RuntimeInvoker`.

Most users should start with `ergo-sdk` for embedded Rust usage or
`ergo-cli` for command-line usage. Depend on this crate directly only when you
need the lower-level kernel capture/replay or supervisor APIs.

## What this crate owns

- `Supervisor`, episode scheduling, retry/deferral decisions, and the write-only
  `DecisionLog` trait.
- `CaptureBundle` and the kernel-owned capture serde surface.
- Capture session helpers and atomic capture writing.
- Strict replay validation and decision/effect comparison helpers.

## What this crate does not own

- Primitive semantics or graph execution; those are owned by `ergo-runtime` and
  reached through adapter runtime handles.
- Adapter manifest meaning and event binding; those are owned by `ergo-adapter`.
- Project loading, host process lifecycle, egress dispatch, and CLI/SDK replay UX.

## More information

- Supervisor contract: [`docs/orchestration/supervisor.md`](../../../docs/orchestration/supervisor.md)
- Kernel layer map: [`crates/kernel/CODE_MAP.md`](../CODE_MAP.md)
- Canonical system docs start at [`docs/INDEX.md`](../../../docs/INDEX.md)