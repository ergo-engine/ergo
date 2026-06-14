---
Authority: PROJECT
Date: 2026-06-14
Author: Codex (Implementation)
Status: CLOSED
Branch: main
Depends-On: >-
  docs/ledger/decisions/crates-io-publish-set.md;
  docs/ledger/decisions/zero-one-stack-release-policy.md
---

# crates.io Alpha Publish

## Scope

Publish the first public Ergo crate stack to crates.io and record the
post-publish smoke evidence.

This record is operational evidence, not a new package-identity or
release-policy decision. The publish set and `0.1.x` compatibility policy
remain owned by the decision records listed in `Depends-On`.

## Published Versions

On 2026-06-14, the following nine crates were published to crates.io at
`0.1.0-alpha.1` in dependency order:

1. `ergo-runtime`
2. `ergo-prod-duration`
3. `ergo-adapter`
4. `ergo-loader`
5. `ergo-fixtures`
6. `ergo-supervisor`
7. `ergo-host`
8. `ergo-sdk`
9. `ergo-cli`

The new-crate publish sequence hit crates.io's new-crate rate limit once.
The publish resumed after the registry-provided retry window elapsed, and
each tier was confirmed resolvable from the index before its dependents
were published.

`ergo-cli` was then patched to `0.1.0-alpha.2` after the first publish to
fix top-level `--version`, `-V`, `--help`, and `-h` handling in the
hand-written dispatcher. The eight non-CLI crates remain at
`0.1.0-alpha.1`.

## Clean-Room Evidence

The first smoke test used a temporary Cargo home and install root, then
installed the published alpha `ergo-cli` from crates.io, scaffolded a
project, and exercised the generated project without depending on the
Ergo checkout:

1. `cargo install ergo-cli --version 0.1.0-alpha.1 --root <tempdir>`
2. `ergo init`
3. Generated `Cargo.toml` used `ergo-sdk = "0.1.0-alpha.1"` with no local
   path dependency.
4. `cargo build` succeeded.
5. `cargo run` completed, wrote a capture, and reported `Completed`.
6. Strict replay succeeded with `graph_id=sample_flow`, `events=1`,
   `invoked=1`, `deferred=0`, and `skipped=0`.

After the CLI patch release, a second clean-room install used an explicit
temporary install root and the exact published prerelease:

```text
cargo install ergo-cli --version 0.1.0-alpha.2 --root <tempdir>
```

That install downloaded and built `ergo-cli v0.1.0-alpha.2` from
crates.io. The installed binary reported:

```text
ergo --version
ergo 0.1.0-alpha.2

ergo -V
ergo 0.1.0-alpha.2
```

The same binary's `ergo --help` and `ergo -h` output matched
`ergo help`.

## Operational Notes

Only prerelease versions exist for the published Ergo crates right now.
Cargo's default selector does not choose prereleases for a bare `*`
requirement, so these commands do not select the alpha line:

```text
cargo install ergo-cli
cargo add ergo-sdk
```

Users must request the explicit prerelease until a non-prerelease
`0.1.0` ships:

```text
cargo install ergo-cli --version 0.1.0-alpha.2
cargo add ergo-sdk@0.1.0-alpha.1
```

Or they can write the SDK dependency directly:

```toml
ergo-sdk = "0.1.0-alpha.1"
```

docs.rs renders for `ergo-sdk` with 100% documented public items and for
`ergo-cli`.

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| PUB7-1 | Publish first crate stack | All nine crates are published to crates.io at `0.1.0-alpha.1` in dependency order and confirmed resolvable. | Codex | CLOSED |
| PUB7-2 | Verify scaffold from registry | Clean-room install/scaffold/build/run/replay proves the default scaffold uses published `ergo-sdk`, not a local checkout path. | Codex | CLOSED |
| PUB7-3 | Patch CLI flag papercut | `ergo-cli 0.1.0-alpha.2` is published and a fresh install proves `--version`, `-V`, `--help`, and `-h` route correctly. | Codex | CLOSED |
| PUB7-4 | Record prerelease install caveat | Public docs state explicit prerelease versions are required until a non-prerelease `0.1.0` exists. | Codex | CLOSED |
