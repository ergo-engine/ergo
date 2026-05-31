# ergo-fixtures

`ergo-fixtures` provides fixture artifact tooling used by the shipped Ergo CLI:
CSV-to-fixture conversion, fixture inspection, fixture validation, and
report-rendering DTOs.

This is a real publishable tooling crate, not hidden test support.
`ergo-cli` depends on it as a normal dependency for `ergo fixture inspect`,
`ergo fixture validate`, and `ergo csv-to-fixture`.

Most application users should start with `ergo-sdk-rust` or `ergo-cli` rather
than depending on this crate directly.

## What this crate owns

- CSV-to-fixture conversion helpers.
- Fixture inspection and validation reports.
- Text/JSON render helpers and fixture-report DTOs used by CLI tooling.

## What this crate does not own

- Canonical fixture parsing grammar or external event payload semantics; those
  come from `ergo-adapter`.
- Runtime replay semantics, capture equality, or host run behavior.
- CLI command parsing or output routing.

## More information

- CLI README: [`crates/prod/clients/cli/README.md`](../../prod/clients/cli/README.md)
- Prod layer map: [`crates/prod/CODE_MAP.md`](../../prod/CODE_MAP.md)