# ergo-prod-duration

`ergo-prod-duration` is a small production-layer helper crate for parsing Ergo's
authored duration literals.

Most users should not depend on this crate directly. It is published because
publishable prod crates use the same parser for current config surfaces.

## What this crate owns

- Parsing integer duration literals with `ms`, `s`, `m`, and `h` suffixes.
- The current user-visible parser error strings returned by that shared helper.

## What this crate does not own

- Runtime timing policy, scheduling, or run-limit semantics.
- Serde wrappers or file-specific diagnostic context for loader or host config.
- A general-purpose duration formatting/parsing API outside Ergo config needs.

## Used by

- `ergo-loader` for project/profile duration literals.
- `ergo-host` for egress configuration durations.

## More information

- Prod layer map: [`crates/prod/CODE_MAP.md`](https://github.com/ergo-engine/ergo/blob/v0.1.0-alpha.1/crates/prod/CODE_MAP.md)
