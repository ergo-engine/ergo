# ergo-cli

`ergo-cli` is the workspace CLI surface over host, loader, fixtures, and
project scaffolding.

It owns:

- command parsing and dispatch
- scaffold/template generation for `ergo init`
- CLI-only fixture, render, and conversion helpers
- text and JSON output formatting

It does not own:

- runtime semantics
- loader decode/discovery rules
- adapter registration or composition policy
- host run/replay orchestration truth

Those remain owned by `ergo-runtime`, `ergo-loader`, `ergo-adapter`, and
`ergo-host`.

## Current Structure

```text
ergo-cli/
  src/
    main.rs                 # binary entrypoint
    lib.rs                  # shared CLI test surface
    cli/
      args.rs               # option parsing helpers
      dispatch.rs           # top-level command routing
      handlers.rs           # CLI use-case glue
    output/
      text.rs               # human-readable output and help
      json.rs               # machine-readable output
      errors.rs             # stderr + exit mapping
    init_project.rs         # scaffold generation/templates for `ergo init`
    graph_yaml.rs           # explicit path-based graph run helpers
    graph_to_dot.rs         # DOT rendering
    render.rs               # SVG rendering via Graphviz
    validate.rs             # manifest validation helpers
    fixture_ops.rs          # fixture inspect/validate helpers
    csv_fixture.rs          # CSV -> fixture conversion
    gen_docs.rs             # generated docs wrapper
    error_format.rs         # typed CLI error rendering
    exit_codes.rs           # stable exit codes
```

## Placement Rules

- `main.rs` stays wiring-only: parse, dispatch, render, exit.
- `cli/` owns command grammar and top-level routing, not semantic rules.
- `init_project.rs` owns scaffold templates and init-time path checks.
- `graph_yaml.rs`, `validate.rs`, `fixture_ops.rs`, and `render.rs` may call
  host, loader, or fixtures use-cases, but they should not redefine product
  policy.
- `output/` and `error_format.rs` own presentation only.

## Current CLI Notes

- The workspace binary uses `ergo help`; top-level `--help` is not a canonical
  command.
- `ergo init` generates Python 3 sample ingress/egress channels, not POSIX `sh`
  scripts.
- The generated sample app exposes `cargo run -- profiles` and
  `cargo run -- doctor`; those are scaffolded app commands, not workspace
  `ergo-cli` subcommands.
