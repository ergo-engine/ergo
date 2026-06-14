# Pre-publish Review Findings

Read-only assessment of the Ergo publish gate, organized by issue type and
ordered by priority. Each item records the cause and what is gained by
rectifying it.

Status note: this is a living findings backlog. Entries that describe the
current source tree may still name `ergo-sdk-rust` or `0.1.0` when that is the
state being diagnosed. Recommended publish-target wording should follow the
decision record: SDK package `ergo-sdk` and first publish version
`0.1.0-alpha.1` unless a later decision changes it.

Disposition labels used below:

- **Resolved pre-publish** — fixed or verified before `v0.1.0-alpha.1`.
- **PUB-7 procedure** — not a code/doc change; closes only during real publish.
- **Accepted for alpha** — known surface/risk is intentionally allowed in
  `0.1.0-alpha.1`, with rationale recorded.
- **Post-alpha follow-up** — real work, but not a first-publish blocker.
- **Informational** — no corrective action required.

## Resolved since this review

- SDK package name/version mismatch: resolved by `5c84cd4`.
- SDK opaque-source replacement gate and reachable lower-crate public error enum
  stability inventory: resolved by `4bb584c`.
- `ergo-supervisor` demo feature / self dev-dependency publish blocker: resolved
  by `84e277c`.
- CLI scaffold external-user default: resolved by `7dd6a80`.
- CLI accidental library surface: resolved by `3284794`.
- Demo-source-context residue: resolved by `144fab4`.
- SDK-adjacent DTO reservation: removed by `e69b852`.
- SDK catalog-helper re-export overreach: resolved by `e224fac`.
- Crate README cross-repo links: resolved by tag-pinned GitHub links for
  `v0.1.0-alpha.1`.
- `ergo-host` intra-doc rustdoc links: resolved; workspace rustdoc passes with
  broken intra-doc links denied.
- Package inclusion sanity: verified for the previous publish candidate, but
  superseded by later post-tag publish-set and SDK-facade changes. The nine-crate
  package check must be refreshed in the retag/sweep stage.
- Exact release tag: `v0.1.0-alpha.1` still points at historical commit
  `7d70ce8`; current `main` is past that tag. The release-candidate tag must be
  moved or recreated after the current reconciliation lands.
- Final publish dry-run: the previous sweep passed from the old tagged state,
  but that evidence is now stale. The final nine-crate sweep is pending the
  retag stage.

## Final disposition summary

| Item | Disposition | Blocks PUB-7? |
|---|---|---|
| Critical scaffold issue | Resolved pre-publish | No |
| Critical SDK name/version issue | Resolved pre-publish | No |
| 1. Crate README links | Resolved pre-publish | No |
| 2. Tagged release commit | Retag required after latest source changes | Yes |
| 3. Propagation / registry checks | PUB-7 procedure after fresh nine-crate sweep | Yes |
| 4. Future `0.1.x` dependency policy | Resolved pre-publish / policy recorded | No |
| 5. Scaffold switch breadth | Resolved pre-publish | No |
| 6. Scaffold SDK version stamping | Resolved for first publish | No |
| 7. `jsonschema` dependency weight | Accepted for alpha | No |
| 7A. Host rustdoc links | Resolved pre-publish | No |
| 8. CLI library surface | Resolved pre-publish | No |
| 8A. SDK-adjacent DTO reservation | Removed pre-publish | No |
| 8B. Demo/test-shaped adapter names | Demo residue resolved; fault harness accepted | No |
| 8C. SDK non-error re-exports | Accepted for alpha / classified | No |
| 9. Scaffold-used SDK entrypoints | Accepted for alpha / scaffold-stable | No |
| 10. CLI help text | Resolved pre-publish | No |
| 11. Init summary | Resolved pre-publish | No |
| 12. Scaffold tests | Resolved pre-publish | No |
| 13. TOML/path escaping | Post-alpha follow-up | No |
| 14. Generated Cargo.toml comment | Resolved pre-publish | No |
| 15. Runtime compatibility stamping | Resolved for runtime compatibility | No |
| 16. docs.rs layer guidance | Post-publish spot-check | No |
| 17. Package inclusion | Retag/sweep refresh required | Yes |
| 18. Metadata polish | Resolved pre-publish | No |
| 19. `ergo-fixtures` publishability | Informational / accepted | No |
| 19A. Name availability | PUB-7 procedure for nine names | Yes, procedural |
| 20. Path + version deps | Informational / confirmed | No |

After the latest source/manifest/doc changes, PUB-7 is not yet open. The next
gate is to retag the release candidate, reconcile the release evidence to the
nine-crate set, and rerun the full publish dry-run sweep from that tagged state.
Only then do the remaining blockers become procedural: publish in the tested
order, verify registry propagation between tiers, and stop if crates.io rejects
any name during the real transaction.

## Critical — CLI scaffold ships broken for external users

### Disposition

**Resolved pre-publish.** Commit `7dd6a80` changed default `ergo init` output to
`ergo-sdk = "0.1.0-alpha.1"`, kept `--sdk-path` as an explicit local-development
override, updated CLI help/summary/docs, and added default-mode scaffold tests.
`cargo test -p ergo-cli` and the full CI gate passed after the change.

### Cause

`ergo init` currently generates a `Cargo.toml` with a local filesystem
`path = "..."` dependency on `ergo-sdk-rust`, and the help text admits the
default only works inside the Ergo checkout. The current publish sequence under
discussion publishes all nine crates first, then switches the scaffold to a
version dependency afterward.

That means the first published `ergo-cli` would knowingly generate a project
that does not build for a normal crates.io user running
`cargo install ergo-cli --version 0.1.0-alpha.2 && ergo init myproject`
outside the author checkout.

### What is gained by rectifying it

- The first external user gets a working scaffold instead of a compile failure.
- Avoids shipping a known-bad first CLI experience that immediately needs a
  patch release.
- Makes the context-free crates.io/docs test exercise the intended public UX,
  not a transitional local-checkout UX.
- Prevents copied first-publish scaffold examples from preserving the wrong
  dependency shape.

## Critical — SDK package name/version disagreement with decision docs

### Disposition

**Resolved pre-publish.** Commit `5c84cd4` renamed the package to `ergo-sdk`,
updated the public import examples to `ergo_sdk`, and moved the publishable
crates plus internal dependency requirements to `0.1.0-alpha.1`.

### Cause

The current manifests and source use `ergo-sdk-rust` at `0.1.0`, but the repo's
publish decision record, `docs/ledger/decisions/crates-io-publish-set.md`, says
the first publish targets the SDK package name `ergo-sdk` and all nine published
crates at `0.1.0-alpha.1`.

The decision record explicitly says:

- Q-NAMING resolves the SDK package name as `ergo-sdk`.
- Q-VERSION resolves the first publish version as `0.1.0-alpha.1`.
- `ergo-sdk-rust` is rejected as residue from the in-repo placeholder period.
- The publish order lists `ergo-sdk` for `crates/prod/clients/sdk-rust`.

This affects the crates.io package identity, the dependency name generated by
`ergo init`, the package name users copy from README examples, and the docs.rs
URL slug.

### What is gained by rectifying it

- Permanently claims the intended crates.io SDK identity the first time.
- Avoids teaching users, lockfiles, README snippets, and agents the wrong SDK
  package name.
- Avoids a later migration from `ergo-sdk-rust` to `ergo-sdk`, which would be
  much harder after a public first publish.
- Aligns manifests, docs, scaffold output, and PUB decision records before
  crates.io makes the package identity irreversible.

## 1. Crates.io/docs.rs README links may break

### Disposition

**Resolved pre-publish.** Commit `7d70ce8` replaced crate README links to
top-level docs/CODE_MAPs/sibling crate READMEs with
`https://github.com/ergo-engine/ergo/blob/v0.1.0-alpha.1/...` links. A link
check after pushing the tag confirmed each unique tag-pinned URL returned HTTP
200.

### Cause

The nine published crate READMEs use repo-relative links to files outside each crate
package, for example:

- `../CODE_MAP.md`
- `../../../docs/system/kernel.md`
- `../../../../docs/authoring/getting-started-sdk.md`
- `../../prod/CODE_MAP.md`

These resolve inside the repository, but crates.io/docs.rs render each crate
package in isolation. The sibling `CODE_MAP.md` files and top-level `docs/`
tree are not necessarily present relative to the packaged README render
context.

Affected areas:

- `ergo-runtime`
- `ergo-adapter`
- `ergo-supervisor`
- `ergo-loader`
- `ergo-host`
- `ergo-prod-duration`
- `ergo-cli`
- `ergo-fixtures`
- `ergo-sdk` / source crate `ergo-sdk-rust`

### What is gained by rectifying it

- Prevents broken links on the first public crates.io landing pages.
- Makes the READMEs actually serve their intended purpose for strangers.
- Preserves the "point to CODE_MAPs, don't duplicate architecture" strategy
  while making the links usable outside the repo.
- Lets a context-free user or agent navigate from crates.io to the right
  architecture docs.
- Avoids publishing a polished-but-fragile documentation surface.

Likely fix shape: use absolute GitHub URLs, ideally pinned to the release tag,
for links to `docs/...`, `crates/kernel/CODE_MAP.md`,
`crates/prod/CODE_MAP.md`, and sibling crate READMEs.

## 2. Publish should happen from an exact tagged release commit

### Disposition

**Retag required after latest source changes.** The release tag
`v0.1.0-alpha.1` still peels to historical commit `7d70ce8`, and CI/dry-run
evidence for that commit was valid when recorded. That evidence is now
superseded by later release-control, CLI, demo-residue, SDK-adjacent DTO, and
SDK-facade changes. The release candidate must be retagged from the current
nine-crate state, and the full dry-run sweep must be rerun from that retagged
state before PUB-7 opens.

### Cause

The current plan says push branch, confirm CI green, then publish. But the
repo's own publish decision doc says the first real publish should run from a
tagged commit after PUB-6 is clean.

The stale evidence to preserve is historical, not current:

- old release tag: `v0.1.0-alpha.1`
- old release-candidate commit: `7d70ce8`
- old full dry-run sweep: passed from that tagged state

Current release evidence is pending retag plus fresh nine-crate sweep.

### What is gained by rectifying it

- Creates a stable source anchor for the exact code published to crates.io.
- Gives README/doc links a release tag target instead of drifting `main`.
- Makes crates.io artifacts, GitHub source, docs, and CI evidence traceable to
  one immutable commit.
- Reduces ambiguity if a publish issue appears later.
- Aligns the actual release process with the repo's recorded release doctrine.

Likely fix shape: after CI is green, merge or otherwise settle the exact release
commit, tag it, run/confirm PUB-6 against that exact tagged state, then publish
from that state.

## 3. Nine-crate publish set needs explicit propagation and registry-resolution gates

### Disposition

**PUB-7 procedure after the fresh nine-crate sweep.** No additional code change
is expected for propagation itself. This closes only during the real publish by
publishing in dependency order, waiting for crates.io propagation, and verifying
registry resolution from a fresh external crate before publishing each dependent
tier.

Use this dependency order for the retagged nine-crate dry-run and publish:

1. `ergo-runtime`
2. `ergo-prod-duration`
3. `ergo-adapter`
4. `ergo-loader`
5. `ergo-fixtures`
6. `ergo-supervisor`
7. `ergo-host`
8. `ergo-sdk`
9. `ergo-cli`

### Cause

The publish order is correct, but a nine-crate interdependent stack amplifies
any low-tier mistake.

Publish order to re-test after retag:

1. `ergo-runtime`
2. `ergo-prod-duration`
3. `ergo-adapter`
4. `ergo-loader`
5. `ergo-fixtures`
6. `ergo-supervisor`
7. `ergo-host`
8. `ergo-sdk`
9. `ergo-cli`

A low-tier crate problem propagates upward. If `ergo-runtime` or
`ergo-adapter` is bad, every dependent crate can inherit the issue.

### What is gained by rectifying it

- Prevents publishing a broken ladder of dependent crates.
- Gives each tier a stop point before the damage compounds.
- Confirms crates.io propagation before dependents rely on newly published
  versions.
- Reduces yank/re-publish fallout.
- Makes failure recovery simpler and localized.

Likely fix shape: after publishing each tier, wait for crates.io propagation,
verify clean registry resolution from outside the workspace, then publish the
next tier. Prefer a resolver check from a temporary external crate, such as a
fresh `cargo new` that depends on the just-published crate, over relying on a
crates.io page reload.

## 4. Future `0.1.x` dependency-range policy is load-bearing

### Disposition

**Resolved pre-publish.** The release policy is recorded in
`docs/ledger/decisions/zero-one-stack-release-policy.md`: `0.1.x` patch releases
must preserve APIs used by already-published `0.1.x` dependents. If a lower
crate needs a breaking internal-stack change, the whole affected stack moves to
`0.2.0` instead of publishing a compatible-looking `0.1.x` release that breaks
existing dependents. Exact internal pins are not the first-alpha policy.

### Cause

Internal workspace dependencies now use `version = "0.1.0-alpha.1"` alongside
local `path` for the first alpha, for example:

- `ergo-adapter` -> `ergo-runtime = { path = "../runtime", version = "0.1.0-alpha.1" }`
- `ergo-host` -> `ergo-supervisor = { path = "../../../kernel/supervisor", version = "0.1.0-alpha.1" }`
- `ergo-sdk` -> `ergo-host = { path = "../../core/host", version = "0.1.0-alpha.1" }`

The same policy question applies to future stack releases: decide whether
compatible-looking lower-crate updates may break already-published dependents,
or whether the stack moves in lockstep / bumps minor for breaking changes.

This is publish-compatible. The issue is semver behavior after publish. Normal
`0.1.x` caret requirements allow compatible `0.1.y` versions.

If a future `ergo-runtime 0.1.1` breaks APIs used by `ergo-host 0.1.0`, a fresh
build of already-published `ergo-host 0.1.0` may resolve the newer runtime and
break.

### What is gained by rectifying it

- Prevents accidental breakage of already-published crates.
- Clarifies the release rule for the whole stack.
- Makes future patch releases safer.
- Reduces pressure for emergency yanks.
- Gives maintainers a simple policy for whether a change is `0.1.x` or `0.2.0`.

Recorded policy:

1. Patch compatibility across the stack: `0.1.x` must preserve APIs used by
   already-published `0.1.x` dependents.
2. Breaking changes bump the affected stack: any breaking internal-stack change
   goes to `0.2.0`, not `0.1.y`.
3. Exact internal pins remain a future option only if the project later chooses
   strict lockstep stack releases.

## 5. Post-publish scaffold switch is broader than `cargo_toml_contents()`

### Disposition

**Resolved pre-publish.** Commit `7dd6a80` introduced an explicit scaffold SDK
dependency mode, made published `ergo-sdk = "0.1.0-alpha.1"` the default,
retained `--sdk-path` as a local override, updated help/summary/docs, and
covered published-mode content plus local-path build/run behavior in CLI tests.

### Cause

The scaffold switch should make `cargo_toml_contents()` emit the decided
published SDK dependency (`ergo-sdk = "0.1.0-alpha.1"` unless the release
decision changes) and remove the default `--sdk-path` / "must be inside
checkout" requirement. But current local-checkout coupling appears in multiple
places:

- `InitOptions { sdk_dependency_path: String }`
- `InitSummary { sdk_dependency_path: String }`
- `parse_init_options()` accepts `--sdk-path` and calls
  `default_sdk_dependency_path()` when omitted
- `scaffold_files(names, sdk_dependency_path)`
- `cargo_toml_contents(names, sdk_dependency_path)`
- `default_sdk_dependency_path(target_dir)` derives workspace root from
  `env!("CARGO_MANIFEST_DIR")` and requires target dir to be inside the checkout
- `resolve_explicit_sdk_dependency_path(...)`
- `render_init_summary(...)` prints `sdk dependency: <path>`
- `crates/prod/clients/cli/src/output/text.rs` documents `--sdk-path`, says
  default works inside checkout, and says use `--sdk-path` outside checkout
  until SDK publish
- `docs/authoring/getting-started-sdk.md`, `docs/authoring/testing-notes.md`,
  the root `README.md`, and crate READMEs also reference current scaffold or SDK
  dependency behavior and need a drift pass when the scaffold switches
- CLI help says generated sample channels target POSIX `sh`, while the scaffold,
  generated project README, generated scripts, and init summary use Python 3
- tests assert the old path-based behavior and help text

### What is gained by rectifying it

- Makes `ergo init` genuinely usable from any directory on any machine.
- Prevents shipped CLI help from describing pre-publish behavior.
- Avoids generated projects that only build on the author's machine.
- Keeps repo-local development support if `--sdk-path` remains as an override.
- Lets the context-free "agent from crates.io" test exercise the actual
  intended UX.

Likely fix shape: introduce an explicit scaffold dependency mode with published
crates.io dependency by default and optional local path dependency via
`--sdk-path`; update generated `Cargo.toml`, comments, init summary, CLI help,
error messages, docs, tests, and an outside-checkout scaffold smoke test.

## 6. Scaffold SDK version stamping can drift if tied blindly to CLI version

### Disposition

**Resolved pre-publish for the first publish; future release policy is tracked
by item 4.** `SCAFFOLD_SDK_VERSION` is a dedicated constant and is not derived
from `env!("CARGO_PKG_VERSION")`. This prevents the immediate scaffold from
blindly coupling the CLI crate version to the SDK dependency version. Before a
future CLI-only or SDK-only release, item 4 must still decide the broader
versioning policy.

### Cause

There is discussion of version-stamping the scaffold from the CLI's own build
version. That is only safe if `ergo-cli` and the published SDK package
(`ergo-sdk`) are guaranteed to release in lockstep.

Workspace version unification is deliberately deferred. Therefore, if
`ergo-cli` later releases without a matching SDK release, using
`env!("CARGO_PKG_VERSION")` from the CLI would generate an SDK dependency
version that could be nonexistent or unintended.

### What is gained by rectifying it

- Prevents `ergo init` from generating non-resolving dependencies after future
  CLI-only releases.
- Makes SDK scaffold dependency version a deliberate release artifact.
- Avoids coupling two crates more tightly than the workspace policy currently
  promises.
- Preserves freedom to release CLI patches independently.

Safer options: use a dedicated `SCAFFOLD_SDK_VERSION` constant, generate the
value at build time from a checked source, keep the decided first-publish SDK
version hardcoded for the immediate switch with a test, or adopt explicit
lockstep release policy before using the CLI crate version.

## 7. `jsonschema` pulls in `reqwest`/`rustls`/`aws-lc-sys`

### Disposition

**Accepted for alpha; post-alpha follow-up.** Re-checking the dependency tree
confirmed that `jsonschema` default features still pull `reqwest`, `rustls`, and
`aws-lc-sys`. Current Ergo adapter use remains local schema compilation, and
the previous publish-candidate dry-run plus workspace rustdoc pass did not
surface a blocker. The fresh retag/sweep remains required for the current state.
This is therefore a build-weight/docs.rs risk to trim after alpha, not a
first-publish blocker. Changing feature flags now would create fresh validation
risk late in release preparation.

### Cause

`ergo-adapter` depends on `jsonschema = "0.40.0"` with default features enabled.
The resolved feature set includes `default`, `reqwest`, `resolve-file`, and
`resolve-http`, and the dependency tree includes `jsonschema`, `reqwest`,
`rustls`, `aws-lc-rs`, and `aws-lc-sys`.

Current Ergo usage appears local:

- `event_binding.rs` uses `jsonschema::draft202012::new(schema)`
- `validate.rs` uses `jsonschema::draft202012::new(schema)`

No HTTP schema retrieval usage was observed in the adapter code. The TLS/native
crypto stack appears to be default-feature baggage, not obviously required by
current Ergo semantics.

### What is gained by rectifying it

- Reduces docs.rs build risk.
- Reduces compile time and dependency weight.
- Avoids exposing a foundational kernel crate to native crypto build friction.
- Simplifies downstream builds for every crate depending on `ergo-adapter`.
- Shrinks the transitive tree inherited by `ergo-fixtures`, `ergo-supervisor`,
  `ergo-host`, `ergo-sdk`, and `ergo-cli`.

Risk level: medium. This is not proven to block publish, especially if PUB-6
dry-runs are clean. But docs.rs has its own build environment, and native
dependencies are exactly the kind of thing that can surprise there.

Likely fix shape: investigate whether `jsonschema` can be used with reduced or
default-disabled features, specifically without `resolve-http`, while preserving
draft 2020-12 local validation. If not fixing before publish, explicitly accept
this as a docs.rs/build-weight risk.

## 7A. `ergo-host` has unresolved intra-doc rustdoc links

### Disposition

**Resolved pre-publish.** Commit `7d70ce8` changed the links to crate-root
qualified intra-doc links. `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links"
cargo doc --workspace --no-deps` passes.

### Cause

The host runner docs contain intra-doc links that are likely unresolved by
rustdoc:

- `prepare_hosted_runner_from_paths`
- `prepare_hosted_runner`
- `LivePrepOptions::for_production`
- `PrepareHostedRunnerFromPathsRequest::for_production`

The links appear in `crates/prod/core/host/src/runner.rs` around the preferred
API path note for `HostedRunner::new`. This is distinct from README link
rendering: it affects rustdoc/docs.rs directly.

### What is gained by rectifying it

- Produces cleaner docs.rs output for `ergo-host`.
- Prevents public API documentation from pointing at unresolved symbols.
- Keeps the host's low-level constructor guidance trustworthy.
- Makes `cargo doc --workspace --no-deps` a cleaner pre-publish signal.

## 8. `ergo-cli` publishes a library surface in addition to the binary

### Disposition

**Resolved pre-publish.** `ergo-cli` is published as the package that installs
the `ergo` binary, not as a Rust embedding API. The accidental library root was
removed before first publish, and the phase-7 checks that previously imported
`ergo_cli::{validate, gen_docs}` now run through the actual `ergo` binary.
Rust applications should depend on `ergo-sdk`.

### Cause

`ergo-cli` was described as the shipped binary, but it had a public library
root:

- `src/lib.rs`
  - `pub mod error_format;`
  - `pub mod gen_docs;`
  - `pub mod validate;`

That meant consumers could depend on `ergo-cli` as a library and import those
modules, even though the README positions the crate as a binary front door.

### What is gained by rectifying it

- Avoids accidentally freezing internal CLI helper modules as public API.
- Clarifies whether `ergo-cli` is binary-only or library+binary.
- Reduces future semver risk when refactoring CLI internals.
- Prevents users from depending on surfaces not meant for stable consumption.

Resolution: remove the library target before first publish and keep validation
coverage by exercising the binary command forms.

## 8A. SDK-adjacent DTO reservation has no downstream workspace consumers today

### Disposition

**Removed pre-publish.** The SDK-adjacent DTO crate was removed from the
workspace and first-alpha publish set. This reverses the earlier
accepted-for-alpha reservation disposition: the crate was an intentional forward
reservation for future cross-language bindings, but it had zero workspace
consumers and only one DTO (`SdkVersion { value: String }`). The reservation can
be reintroduced later as an additive crate when a real binding or cross-client
consumer exists.

### Cause

The SDK-adjacent DTO crate was in the publish set and exposed the small shared
DTO surface centered on `SdkVersion { value: String }`, but no other workspace
crate depended on it.

That does not make the crate wrong: it may be an intentional reservation for a
cross-language SDK/bindings surface. But publishing it now creates a public
crate to maintain through the `0.1.x` cycle before any workspace consumer proves
the dependency shape.

### What is gained by rectifying it

- Keeps the first-alpha publish set limited to crates with real consumers.
- Avoids maintaining an unused public crate through `0.1.x` while the type
  surface is still speculative.
- Preserves the reservation as a documented future option rather than a shipped
  idle package.

Resolution: remove the idle reservation before first publish and record the
decision in the publish-set decision ledger.

## 8B. Public adapter/kernel surfaces expose demo or test-shaped names

### Disposition

**Split disposition.** The demo-source-context pair was removed pre-publish;
`FaultRuntimeHandle` is kept for alpha unchanged.

- `DemoSourceContextError` and
  `ensure_demo_sources_have_no_required_context(...)` were orphaned demo/fixture
  residue, not live host setup. Workspace grep found zero callers for the
  helper, and the host's `HostAdapterSetupError::DemoSourceContext` variant had
  no construction site. The pair was removed from `ergo-adapter`, and the dead
  host variant/import/trait arms were removed from `ergo-host`.
- `FaultRuntimeHandle` is separate and explicitly out of scope for that
  removal. It remains live supervisor replay/integration test-harness machinery
  and is documented in invariants around deterministic replay behavior.

The corrected decision is recorded in
`docs/ledger/decisions/remove-demo-source-context-residue.md`. No
PHASE_INVARIANTS ID is involved: this was dead-code cleanup, not an invariant
fix. The duplicated safety rule was not removed from the live run path; the
no-adapter context-requirement check remains enforced by
`scan_adapter_dependencies` plus `ensure_adapter_requirement_satisfied`.

### Cause

`ergo-adapter` publicly exposed names that read as demo/test scaffolding:

- `DemoSourceContextError`
- `ensure_demo_sources_have_no_required_context(...)`
- `FaultRuntimeHandle`

The first two were residue from prior demo/fixture cleanups. History shows the
helper was introduced with callers, later gained another caller, and then all
callers were removed across `e383d8f3`, `d32a60f7`, and `84e277c1`; the
kernel-side definition survived because it lived outside each deleted caller's
file. Findings commit `c8abc84` incorrectly asserted that they were still used
through host setup and were not accidental dead code.

`FaultRuntimeHandle` remains a different case. `crates/kernel/CODE_MAP.md`
explicitly describes it as a test-only injector, and supervisor tests actively
use it. Once the crate is published, that name is part of the public API surface
unless visibility or naming changes before first publish.

### What is gained by rectifying it

- Removes dead demo/fixture residue before the first public `ergo-adapter` and
  `ergo-host` releases.
- Prevents a kernel crate from looking like it shipped unused demo helpers as
  product API.
- Records the disposition-drift correction visibly instead of silently
  overwriting the stale rationale.
- Keeps the live `FaultRuntimeHandle` test harness separate from the removed
  demo pair so it can be judged on its own merits.

## 8C. SDK transparent re-exports increase semver coupling to lower layers

### Disposition

**Partially resolved before alpha; remaining re-exports accepted with explicit
classification.** The error-surface portion is already resolved by the opaque
`ErgoErrorSource` model. This finding previously merged two different custom
primitive concerns: surface a user needs to write custom primitives, and
core-only catalog constructors that no SDK facade consumes. Those are now split.

The SDK root re-export of `build_core`, `build_core_catalog`, and
`core_registries` is removed before first publish. Those functions remain public
in `ergo-runtime`, but they are core-only constructors. The SDK registration
mechanism is `ErgoBuilder::add_*`, which feeds the SDK's private
`CatalogBuilder` and then calls `CatalogBuilder::build()`. No SDK API accepts
an externally built `CoreRegistries` or `CorePrimitiveCatalog`, so the old SDK
doc comment calling these helpers "advanced primitive registration" was too
broad and was removed with the re-export.

The remaining non-error SDK re-exports are intentional because they appear in
SDK-authored configuration, custom primitive authoring, manual-runner event
construction, or returned run/replay outcomes:

- `FixtureItem` supports SDK-authored in-memory fixture ingress.
- `EventTime`, `ExternalEventKind`, `HostedEvent`, `HostedStepOutcome`, and
  `RunTermination` support manual runner input/output and hosted event
  inspection.
- `AdapterInput`, `CaptureBundle`, `CaptureJsonStyle`, `Egress*`,
  `InterruptedRun`, `InterruptionReason`, `RunOutcome`, and `RunSummary` are
  SDK-facing config/outcome vocabulary. The payload-name inconsistency for
  `RunSummary`, `InterruptedRun`, and `InterruptionReason` is a known
  post-alpha item tracked in GitHub issue #81.
- `ExecutionContext` and the primitive modules (`action`, `common`, `compute`,
  `source`, `trigger`) are retained as genuine custom-primitive authoring
  surface: user implementations name `ExecutionContext` and `common::Value`,
  the `ErgoBuilder::add_*` bounds name the primitive traits from those modules,
  and the `ergo init` scaffold uses the modules directly.

These re-exports are semver-sensitive SDK alpha surface. They should be treated
as intentional until a later SDK facade narrowing pass removes or wraps them.

Status update: the SDK error-surface replacement gate is no longer an open
item in this finding. The typed-accessor gate was reconciled to the opaque
`ErgoErrorSource` model, the stale accessor wording was removed from the
publish plan, and the direct-SDK-reachable runtime
`compute::ErrorType` vocabulary was marked `#[non_exhaustive]`. The remaining
concern here is the broader non-error transparent re-export posture, if any,
not the lower-crate public error-enum stability gate.

### Cause

`ergo-sdk` transparently re-exports lower-layer types, including adapter event
types and `RunTermination`, and also exposes host/config/outcome-style types
through its public API surface.

The catalog helper trio was an over-broad classification inside that bucket:
`build_core`, `build_core_catalog`, and `core_registries` build the default
runtime core, not the SDK facade's custom-registration path. They remain
available from `ergo-runtime`; they are no longer SDK-root surface.

The publish decision record already warns that transparent root re-exports need
PUB-1 classification. If any re-export remains unclassified, the SDK semver
surface becomes coupled to lower-layer type shape: fields, variants, and method
signatures in lower crates can become SDK semver events.

The lower-crate public error-enum stability portion of that classification has
been handled separately by the replacement gate. Do not carry `ErrorType` or
stale typed-accessor wording forward as unresolved action items from this plan.

### What is gained by rectifying it

- Confirms that each lower-layer re-export is intentionally part of the SDK
  authoring/config/outcome vocabulary.
- Removes three core-only catalog constructors from the SDK root before they
  become a published facade promise.
- Keeps `ExecutionContext` and the primitive modules available for real custom
  primitive authoring.
- Reduces accidental SDK semver pressure from internal kernel/prod type changes.
- Gives special attention to `RunTermination`, which is visible through SDK
  tests and public re-export paths.
- Aligns implementation with the publish decision record's Q-SURFACE rules.

## 9. Generated scaffold code freezes SDK entrypoints as de facto stable

### Disposition

**Accepted for alpha and classified as scaffold-stable.** The generated
scaffold uses `Ergo`, `ProjectSummary`, `StopHandle`,
`Ergo::from_project(".")`, `.add_source(...)`, `.add_action(...)`, `.build()`,
`.run_profile_with_stop(...)`, `.validate_project()`, and
`.replay_profile(...)`. Those entrypoints are now treated as scaffold-stable for
the `0.1.x` alpha line unless a later release intentionally migrates generated
projects. The CLI scaffold tests compile and run the generated project through
the local `--sdk-path` mode and check the published-mode generated contents.

### Cause

The generated sample app imports and uses public SDK symbols directly,
including:

- `Ergo`
- `ProjectSummary`
- `StopHandle`
- `Ergo::from_project(".")`
- `Ergo::builder()`
- `StopHandle::new()`
- `StopHandle::stop()`
- `.add_source()`
- `.add_action()`
- `.build()`
- `.run_profile_with_stop()`
- `.validate_project()`
- `.replay_profile()`

That generated code becomes a compatibility promise because user projects will
copy it at scaffold time.

### What is gained by rectifying it

- Makes the scaffolded API contract explicit.
- Prevents future SDK refactors from accidentally breaking generated projects.
- Helps distinguish SDK public API from scaffold-stable API.
- Reduces support burden after users generate projects from `ergo init`.

Likely fix shape: classify the scaffold-used SDK entrypoints as stable enough
for generated projects or intentionally temporary with migration expectations;
add tests around the generated code path if not already sufficient.

## 10. CLI help text still describes pre-publish scaffold behavior

### Disposition

**Resolved pre-publish.** CLI usage/help now describes the published
`ergo-sdk` default, keeps `--sdk-path` as a local-development override, and names
Python 3 as the sample channel runtime.

### Cause

Current help text for `ergo init` says:

- `ergo init <project-dir> [--sdk-path <path-to-ergo-sdk-rust>] [--force]`
- default SDK path works inside the checkout
- use `--sdk-path` outside the checkout until `ergo-sdk-rust` is published

This becomes stale immediately after the SDK is published and the scaffold
switch is made.

### What is gained by rectifying it

- Prevents the shipped CLI from telling users old pre-publish instructions.
- Aligns command help with crates.io behavior.
- Makes `ergo help init` useful for context-free users.
- Reduces confusion around whether users need a local checkout.

Likely fix shape: when doing the scaffold switch, update help to say the default
uses the published `ergo-sdk` package and `--sdk-path` is for local development
or checkouts only, if retained.

## 11. Generated init summary still assumes a path dependency

### Disposition

**Resolved pre-publish.** `render_init_summary(...)` now reports the dependency
mode: `ergo-sdk = "0.1.0-alpha.1"` for default published mode, or `path <...>`
for explicit `--sdk-path`.

### Cause

`render_init_summary(...)` currently prints `sdk dependency: <path>`. This is
correct for the current local-checkout scaffold but wrong for the post-publish
default if the dependency becomes versioned.

### What is gained by rectifying it

- Keeps CLI output truthful after `ergo init`.
- Makes the generated-project path clearer for new users.
- Avoids implying that every scaffold has a local SDK path.

Likely fix shape: change summary rendering to describe the dependency mode:
`sdk dependency: ergo-sdk = "0.1.0-alpha.1"` for default published mode, or
`sdk dependency: path <...>` for explicit local override.

## 12. Scaffold tests must cover published mode, not only checkout mode

### Disposition

**Resolved pre-publish.** CLI scaffold tests now cover default published-mode
generation outside the workspace, local `--sdk-path` build/run behavior, help
text, stale-comment absence, and runtime-compatibility drift.

### Cause

Current tests are built around the existing path-based scaffold behavior. The
post-publish mode will need tests for the new default path.

Specific gaps:

- no post-publish/default versioned-dependency simulation
- tests assert old `--sdk-path` help and generated path dependency
- generated `Cargo.toml` parse/build behavior should be checked in versioned
  mode
- outside-checkout scaffold should succeed without `--sdk-path` after the switch

### What is gained by rectifying it

- Prevents regression to local-only scaffolds.
- Ensures the shipped CLI can generate usable projects from arbitrary
  directories.
- Catches stale help text and stale generated comments.
- Gives confidence before the context-free agent test.

## 13. Generated TOML/path escaping is narrow

### Disposition

**Post-alpha follow-up, low blast radius.** Default scaffolds no longer render a
path dependency, so this affects only explicit `--sdk-path` local-development
overrides. The current helper escapes backslashes and quotes, which covers the
most common TOML-breaking path characters. A later hardening pass should add
path edge-case tests or switch generated dependency rendering to a TOML
serializer if the template grows.

### Cause

`cargo_toml_contents()` currently escapes only backslashes and quotes in the SDK
dependency path via `escape_toml_string`.

That has been adequate for the current local path use, but the implementation is
narrow for arbitrary strings and path edge cases. This becomes less important if
the default becomes a version dependency, but it still matters if `--sdk-path`
remains.

### What is gained by rectifying it

- Makes `--sdk-path` robust for paths with special characters.
- Reduces generated invalid TOML edge cases.
- Makes local-development override less brittle.
- Helps Windows and unusual path scenarios.

Likely fix shape: keep `--sdk-path` path rendering tested with spaces, quotes,
and backslashes; prefer TOML serialization for generated dependency tables if
the template grows.

## 14. `cargo_toml_contents()` generated comment will become stale

### Disposition

**Resolved pre-publish.** The generated `Cargo.toml` no longer contains the
local-checkout/until-publish comment.

### Cause

Generated `Cargo.toml` currently includes a comment saying the scaffold points
at a local `ergo-sdk-rust` checkout until the SDK is published outside the
repository.

That comment becomes wrong as soon as the default dependency is versioned.

### What is gained by rectifying it

- Prevents every generated project from carrying obsolete pre-publish
  commentary.
- Makes the scaffold look intentional and production-ready.
- Avoids confusing new users about whether the SDK is actually published.

## 15. Adapter manifest/runtime compatibility stamping in scaffold may drift

### Disposition

**Resolved for runtime compatibility; residual template-version audit accepted
for alpha.** The generated adapter manifest now uses
`SCAFFOLD_RUNTIME_COMPATIBILITY`, and
`scaffold_runtime_compatibility_matches_runtime_version` asserts it matches
`ergo_runtime::runtime_version()`. Remaining generated `version = "0.1.0"` or
`version: 1.0.0` values are sample/user-artifact versions, not registry package
versions. A later authoring-doc pass can make that distinction more explicit.

### Cause

The scaffold contains hardcoded version-like values in generated project
artifacts, including generated project `Cargo.toml` version, generated
`ergo.toml` project version, and adapter manifest/runtime compatibility-like
fields.

Some values are template/project versions, which may be fine. But anything
representing runtime/protocol compatibility should not silently drift from the
real kernel/host constants.

### What is gained by rectifying it

- Prevents generated adapter manifests from becoming stale after future
  runtime/protocol bumps.
- Separates user project version from Ergo runtime/protocol compatibility.
- Reduces confusion about which `0.1.0` refers to the user app, SDK crate,
  runtime, or adapter protocol.

Likely fix shape: audit generated adapter and channel templates after publish;
keep user project `version = "0.1.0"` if intentionally a sample app version;
inject or centralize actual protocol/runtime compatibility constants where
applicable; document which versions are user-owned versus Ergo-owned.

## 16. docs.rs rustdoc may not carry the same internal-layer warnings as README

### Disposition

**Accepted for alpha; post-publish spot-check.** Workspace rustdoc builds clean
with broken intra-doc links denied, and crate READMEs now carry the main
crates.io landing guidance. Crate-root rustdoc does not yet systematically
duplicate every README warning that most users should start with SDK/CLI. This
is documentation polish, not a mechanical publish blocker. Spot-check rendered
docs.rs pages after publish and tighten crate-root rustdoc in a follow-up if
users land in the wrong layer. The current reconciliation is docs/manifests-only,
so it deliberately does not edit crate-root rustdoc comments inside source files.

### Cause

Crates.io READMEs now warn that internal crates are internal layers and most
users want SDK/CLI. But docs.rs users may land on crate rustdoc generated from
`lib.rs` comments instead of the README.

If crate-level rustdoc is more API-forward than the README, users may miss the
intended dependency guidance.

### What is gained by rectifying it

- Keeps crates.io and docs.rs user guidance aligned.
- Reduces accidental direct dependency on internal layers.
- Helps context-free agents choose the right crate.
- Makes internal-layer boundaries visible in API docs, not only the landing
  page.

Likely fix shape: spot-check docs.rs-rendered crate root docs after publish, or
pre-publish via local rustdoc, to ensure they carry equivalent "most users want
SDK/CLI" guidance where needed.

## 17. Package inclusion should be verified for docs and licenses

### Disposition

**Retag/sweep refresh required.** `cargo package --list --allow-dirty` was
inspected for the previous publish candidate. After removing the SDK-adjacent
DTO reservation, the nine-crate package inclusion check must be refreshed during
the retag/sweep stage. The expected package shape remains: each published crate includes
`README.md`, `LICENSE-MIT`, and `LICENSE-APACHE`, with no obvious bulk entries
such as `target/`, `.git/`, top-level `docs/`, or crate archives.

### Cause

All nine publishable crate directories should contain:

- `README.md`
- `LICENSE-MIT`
- `LICENSE-APACHE`

Required metadata is clean. But package inclusion/rendering is still worth
checking because crates.io packages are crate-root scoped.

The README link issue is the larger version of this: files outside the crate
root are not automatically part of the crate package.

### What is gained by rectifying it

- Confirms license files are included in published packages.
- Confirms README files are included and used as landing pages.
- Confirms no unintended files are packaged.
- Confirms no intended docs are missing from package tarballs.

Likely fix shape: for each crate, inspect `cargo package --list` or
`cargo publish --dry-run` output during the retag/sweep stage. The main
remaining concern is confirming package shape and external README link targets
from the current nine-crate state.

## 18. Metadata polish is resolved pre-publish

### Disposition

**Resolved pre-publish.** The nine publishable manifests now carry explicit
`readme = "README.md"`, `homepage.workspace = true`, keywords, and crates.io
categories. The workspace homepage is the repository URL because no separate
project site is recorded.

The chosen category slugs were verified against the crates.io category API
before being added:

- `ergo-runtime`: `development-tools`, `simulation`
- `ergo-adapter`: `development-tools`, `config`, `parser-implementations`
- `ergo-supervisor`: `development-tools`, `simulation`
- `ergo-loader`: `development-tools`, `config`, `parsing`, `filesystem`
- `ergo-host`: `development-tools`, `config`
- `ergo-prod-duration`: `date-and-time`, `parser-implementations`,
  `value-formatting`, `config`
- `ergo-fixtures`: `development-tools`, `command-line-utilities`,
  `parser-implementations`
- `ergo-sdk`: `api-bindings`, `development-tools`
- `ergo-cli`: `command-line-utilities`, `development-tools`, `visualization`

`documentation` and `authors` remain omitted for the first alpha. docs.rs will
derive documentation pages from the published crates, and repository ownership is
clearer than embedding an author list in every crate manifest.

### Cause

The workspace previously lacked some crates.io polish metadata:

- keywords
- categories
- homepage
- explicit `readme = "README.md"` in each manifest

Required metadata is present:

- description
- license
- repository
- README auto-discovery, now made explicit with `readme = "README.md"`

### What is gained by rectifying it

- better crates.io discoverability
- clearer landing-page metadata
- more polished crate cards

### Why this did not need a separate publish dry-run

These manifest additions are low risk once category slugs are verified. The full
nine-crate publish dry-run is still deferred to the retag/sweep stage because
that stage must validate the complete post-tag state, not only metadata parsing.

## 19. `ergo-fixtures` being publishable is correct

### Disposition

**Informational / accepted.** Keep `ergo-fixtures` in the publish set because
`ergo-cli` depends on it for shipped fixture commands. No corrective action is
needed.

### Cause

There was earlier uncertainty because it lives under `crates/shared/fixtures`,
but `ergo-cli` depends on it as a normal dependency for shipped commands:

- `ergo fixture inspect`
- `ergo fixture validate`
- `ergo csv-to-fixture`

### What is gained by leaving it publishable

- Keeps `ergo-cli` publishable without ripping out live commands.
- Makes fixture tooling reusable and accurately documented.
- Avoids hiding a real shipped dependency as test support.

Conclusion: do not remove `ergo-fixtures` from the publish set.

## 19A. Name availability is not guaranteed until the publish transaction

### Disposition

**PUB-7 procedure.** Exact-name availability must be re-checked for the nine
published names immediately before the retagged dry-run/publish pass. The
previous name check is historical because the SDK-adjacent DTO reservation has
been removed from the publish set. This still cannot be closed until crates.io
accepts each real publish transaction. During PUB-7, publish name-sensitive low-tier crates
promptly and stop if any name collision appears.

### Cause

Searches or absence of local conflicts can suggest that intended crate names are
available, but crates.io names are only reserved by the actual publish
transaction. This matters most for identity-sensitive names, especially the
decided SDK package name `ergo-sdk` before crates.io reserves it.

### What is gained by rectifying it

- Reduces race risk around the most important public crate identity.
- Encourages publishing name-sensitive low-risk crates as soon as the final name
  and version decision is settled.
- Keeps the release plan honest: a name is not truly secured until crates.io
  accepts the package.

## 20. Path + version dependencies are not themselves a publish blocker

### Disposition

**Informational / confirmed.** Final packaged manifests normalize internal
workspace dependencies to version-only requirements at `0.1.0-alpha.1`; the only
remaining `path =` entries in packaged manifests are Cargo target paths for
lib/bin/test entries, not dependency paths.

### Cause

Internal dependencies use the standard workspace-development form:

- `path = "..."`
- `version = "0.1.0-alpha.1"`

This can look suspicious, but it is the correct pattern for local workspace
development plus crates.io publish compatibility.

### What is gained by leaving them as-is

- Local workspace development keeps using path dependencies.
- Published crates carry version requirements for crates.io.
- Avoids unnecessary manifest churn before publish.

The real issue to track instead is the future semver/range policy for `0.1.x`
stack releases.
