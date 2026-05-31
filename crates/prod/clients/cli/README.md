# ergo-cli

`ergo-cli` provides the `ergo` command-line interface for project scaffolding,
graph runs, replay, manifest validation, fixture tooling, and graph
visualization.

Use this crate when you want the shipped binary. Rust applications that embed
Ergo directly usually want `ergo-sdk-rust` instead.

## Current command surface

The canonical help command is:

```sh
ergo help
```

Common v1 commands:

```sh
ergo init my-project
ergo run graph.yaml -f events.jsonl
ergo replay capture.json -g graph.yaml
ergo fixture inspect events.jsonl
ergo fixture validate events.jsonl
ergo validate adapter.yaml
ergo csv-to-fixture prices.csv events.jsonl
ergo graph-to-dot graph.yaml -o graph.dot
ergo render graph graph.yaml -o graph.svg
```

`ergo render` currently dispatches through the `graph` target, so the rendering
form is `ergo render graph <graph.yaml> ...`.

## What this crate owns

- The `ergo` binary entrypoint, command parsing, dispatch, exit behavior, and
  text/JSON output rendering.
- `ergo init` scaffold generation and its CLI-facing path checks.
- CLI wrappers for host run/replay/validation, graph visualization, fixture
  inspection/validation, CSV-to-fixture conversion, and generated-doc checks.

## What this crate does not own

- Runtime primitive semantics or graph execution rules.
- Loader decode/discovery behavior.
- Adapter manifests, adapter composition policy, or capture/replay semantics.
- Host orchestration truth; the CLI calls host use cases rather than
  reimplementing them.

Those responsibilities live in `ergo-runtime`, `ergo-loader`, `ergo-adapter`,
`ergo-supervisor`, and `ergo-host`.

## Fixture tooling

The shipped binary uses `ergo-fixtures` for:

- `ergo fixture inspect`
- `ergo fixture validate`
- `ergo csv-to-fixture`

`ergo-fixtures` is therefore a real publishable tooling crate, not hidden test
support.

## Notes

- Use `ergo help` or `ergo help <topic>`; bare top-level `--help` is not
  dispatched as the maintained help surface.
- `ergo init` generates a sample Rust app. Commands such as
  `cargo run -- profiles` and `cargo run -- doctor` belong to that generated
  app; they are not top-level `ergo` commands.
- SVG rendering uses Graphviz `dot` at runtime.

## More information

- Prod layer map: [`crates/prod/CODE_MAP.md`](../../CODE_MAP.md)
- SDK getting started guide: [`docs/authoring/getting-started-sdk.md`](../../../../docs/authoring/getting-started-sdk.md)
