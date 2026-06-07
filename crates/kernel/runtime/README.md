# ergo-runtime

`ergo-runtime` is the kernel runtime crate. It defines Ergo's primitive
ontology, bundled deterministic standard library, graph expansion/validation
pipeline, and synchronous execution engine.

Most users should start with `ergo-sdk` for embedded Rust usage or
`ergo-cli` for command-line usage. Depend on this crate directly only when you
are building lower-level kernel integrations, custom primitive registration
surfaces, or runtime-focused tooling.

## What this crate owns

- The four primitive families: Source, Compute, Trigger, and Action.
- Primitive manifests, metadata, catalogs, registries, and the bundled core
  primitive inventory.
- Expansion from authoring clusters into executable graph form.
- Runtime validation and synchronous execution.
- Runtime-owned error types for validation and execution failures.

## What this crate does not own

- Adapter manifests, event binding, adapter composition, or runtime invoker
  handles; those are owned by `ergo-adapter`.
- Episode scheduling, capture, and replay; those are owned by
  `ergo-supervisor`.
- Project loading, host orchestration, CLI behavior, or SDK ergonomics.

## Bundled stdlib

The crate ships the current core inventory of Source, Compute, Trigger, and
Action primitive implementations. Callers that need lower-level access can use
`catalog::build_core()`, `catalog::build_core_catalog()`,
`catalog::core_registries()`, or `catalog::CatalogBuilder`.

Context-reading source primitives apply deterministic defaults when their
declared context key is missing or has the wrong type. The runtime
`ExecutionContext` itself does not invent defaults; the defaults belong to the
individual context source primitives.

## Runtime posture

The v1 runtime is synchronous and single-threaded by construction. It performs
validation before execution and keeps execution state local to each call. Do not
read this crate as the public product surface; the SDK and CLI wrap it through
adapter, supervisor, loader, and host layers.

## More information

- Kernel layer map: [`crates/kernel/CODE_MAP.md`](https://github.com/ergo-engine/ergo/blob/v0.1.0-alpha.1/crates/kernel/CODE_MAP.md)
- Kernel overview: [`docs/system/kernel.md`](https://github.com/ergo-engine/ergo/blob/v0.1.0-alpha.1/docs/system/kernel.md)
- Execution model: [`docs/system/execution.md`](https://github.com/ergo-engine/ergo/blob/v0.1.0-alpha.1/docs/system/execution.md)
