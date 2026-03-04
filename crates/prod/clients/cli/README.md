# ergo-cli Placement Guide

This README defines the target CLI shape: thin adapter only.

If code is domain logic (validation rules, YAML parsing, composition policy, fixture semantics), it does **not** belong in `ergo-cli`.

## Target Structure

```text
ergo-cli/
  src/
    main.rs
    cli/
      args.rs
      dispatch.rs
    output/
      text.rs
      json.rs
      errors.rs
    exit_codes.rs
```

## What Goes Where

### `src/main.rs`
- Use for: app wiring/composition root only.
- Register here: startup flow (`parse -> dispatch -> render -> exit`).
- Do not put here: command/domain business logic.

### `src/cli/args.rs`
- Use for: CLI argument and flag definitions, parsing, `--help` text.
- Register here: new command names, subcommands, flags, aliases.
- Do not put here: runtime/adapter rules or filesystem/domain transforms.

### `src/cli/dispatch.rs`
- Use for: mapping parsed commands to application/use-case calls.
- Register here: which use-case function each command invokes.
- Do not put here: parsing manifests/graphs, fixture validation, policy checks.

### `src/output/text.rs`
- Use for: human-readable output formatting.
- Register here: command success summaries and text reports.
- Do not put here: domain decisions or validation branches.

### `src/output/json.rs`
- Use for: machine-readable output formatting (`--json`).
- Register here: JSON response envelopes/shapes.
- Do not put here: business logic or policy checks.

### `src/output/errors.rs`
- Use for: converting typed domain errors into CLI-facing messages.
- Register here: error code/message templates and display shape.
- Do not put here: new domain rules; only map existing domain errors.

### `src/exit_codes.rs`
- Use for: stable exit code definitions and error-to-exit-code mapping.
- Register here: any new exit code constants.
- Do not put here: message formatting or domain rule implementation.

## Quick Placement Checks

- "Does this parse command-line flags?" -> `src/cli/args.rs`
- "Does this decide which use-case to call?" -> `src/cli/dispatch.rs`
- "Does this call runtime/host/adapter APIs?" -> `src/cli/dispatch.rs`
- "Does this format text output?" -> `src/output/text.rs`
- "Does this format JSON output?" -> `src/output/json.rs`
- "Does this map domain errors to CLI errors?" -> `src/output/errors.rs`
- "Does this define process exit status?" -> `src/exit_codes.rs`
- "Does this enforce domain semantics?" -> move to `runtime`/`adapter`/`ergo-host`, not CLI

