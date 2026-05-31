# ergo-loader

`ergo-loader` is the production loader crate. It discovers projects, reads
filesystem or in-memory graph sources, decodes YAML/JSON authoring text, resolves
cluster trees, and hands sealed graph assets to the host.

Most users should start with `ergo-sdk-rust` for embedded Rust usage or
`ergo-cli` for command-line usage. Depend on this crate directly only when you
need lower-level project discovery, graph decode, or asset-loading surfaces.

## What this crate owns

- UTF-8 source loading from filesystem paths and caller-provided in-memory
  sources.
- YAML/JSON graph decode into runtime authoring structures.
- Filesystem and logical in-memory cluster discovery.
- `ergo.toml` project/profile loading and project-root discovery.
- Sealed `PreparedGraphAssets` construction for host preparation.
- Loader-shaped I/O, decode, discovery, and project errors.

## What this crate does not own

- Kernel ontology, runtime validation rules, or execution.
- Adapter registration, adapter composition policy, or event binding.
- Host run/replay orchestration, egress dispatch, or product-facing CLI/SDK UX.

## Main surfaces

- Decode graph text from strings or files.
- Load graph source bundles from paths or in-memory source lists.
- Load `PreparedGraphAssets` for host run/replay/validation preparation.
- Discover and load Ergo projects from `ergo.toml`.

`PreparedGraphAssets` is intentionally sealed: external callers obtain it from
loader functions and read through accessors, rather than constructing or mutating
it directly.

## In-memory source notes

In-memory graph loading uses loader-defined logical source IDs. Use relative,
path-like IDs such as `graphs/root.yaml`; logical paths are platform-independent
and use `/` separators. The human-facing `source_label` is for diagnostics and
is not the semantic lookup identity.

## More information

- Prod layer map: [`crates/prod/CODE_MAP.md`](../../CODE_MAP.md)
- Loader contract: [`docs/authoring/loader.md`](../../../../docs/authoring/loader.md)
- Project convention: [`docs/authoring/project-convention.md`](../../../../docs/authoring/project-convention.md)
