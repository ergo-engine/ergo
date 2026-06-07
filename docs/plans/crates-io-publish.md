---
Authority: PROJECT
Date: 2026-05-10
Author: Augment Agent (planning) + Sebastian (Architect)
Status: Active
---

# crates.io Publish Plan

Working scope-shaping doc for the first crates.io publish of the Ergo
workspace. Not authoritative. Final rulings graduate into
`docs/ledger/decisions/`; executable scope graduates into
`docs/ledger/dev-work/`.

This plan exists because publishing is the first action that turns
internal `pub` symbols into external semver commitments. Decisions made
here are cheaper than retractions made after first publish.

PUB-3 landed as
[`crates-io-publish-set.md`](../ledger/decisions/crates-io-publish-set.md).
That decision record resolves Q-SURFACE, Q-USER, Q-NAMING, Q-VERSION,
the publish set, host-crate semver propagation, and the PUB-7 gate. This
plan remains the working order-of-operations spine.

## Goal

Ship the user-facing Rust surface to crates.io so that
`ergo init` no longer requires `--sdk-path` and external users can
`cargo add` an SDK and `cargo install` a CLI.

Out of scope for this plan: programmatic graph-builder API
(deferred — see chat record), UI publication, non-Rust artifacts.

## Current State

| Concern | State | Action |
|---|---|---|
| Workspace `[workspace.package]` | Declares `license`, `repository`, `edition` | Crates need to inherit or redeclare |
| Per-crate `description` | Missing on all 11 crates | Mechanical fix |
| LICENSE files | `LICENSE-MIT` and `LICENSE-APACHE` present at workspace root | Per-crate symlinks so they ship inside each `.crate` tarball |
| Inter-crate deps | `path = "..."` only | Each needs `version = "x.y"` added for publish |
| Per-crate READMEs | SDK has one (per SDK-9 closure); kernel and host crates do not | Writing task |
| Doc comments | File-level `//!` headers exist (AGENTS.md §4A); type/method-level rustdoc is uneven | Audit + writing task |
| SDK public surface | ~25 host types `pub use`d transparently | Audit + decision (Q-SURFACE, Q-USER) |
| Versioning | All crates at `0.1.0` | Decide alpha vs. plain 0.1.0 (Q-VERSION) |

## Publish Set (sketch)

The SDK `pub use`s host types, so every crate transitively reachable
through the SDK's public surface must publish.

| Crate | Publish? | Why |
|---|---|---|
| `ergo-runtime` | Yes | Kernel semantics; SDK + host re-export types |
| `ergo-prod-duration` | Yes | Transitive dep of host and loader |
| `ergo-adapter` | Yes | Kernel; SDK re-exports `FixtureItem`, etc. |
| `ergo-supervisor` | Yes | Kernel; transitively required by host |
| `ergo-loader` | Yes | Used by SDK directly |
| `ergo-host` | Yes | Used by SDK and CLI directly |
| `ergo-sdk-types` | Yes | Stand-alone SDK-adjacent type crate in the publish set |
| `ergo-sdk` | Yes | Primary user-facing library; source path remains `crates/prod/clients/sdk-rust` |
| `ergo-cli` | Yes | User-installable binary (`ergo`) |
| `ergo-fixtures` | Yes | Non-optional dep of `ergo-cli` (CLI's `csv-fixture` and fixture-report subcommands). Publishing required unless cli's fixture surface is feature-gated as a separate refactor. |
| `ergo-test-support` | No | Workspace-internal test scaffolding; explicit `publish = false` to enforce. |

Initial publish set: **10 crates** (revised from 9 after PUB-2 dependency audit
discovered `ergo-fixtures` was non-optional).

## Naming Decision (Q-NAMING)

The pre-rename `ergo-sdk-rust` package made user code read
`use ergo_sdk_rust::Ergo`. Two real candidates:

- **`ergo`** — most idiomatic; requires that the name be available on
  crates.io. Needs a check before commitment.
- **`ergo-sdk`** — drop the `-rust` suffix; honest about being the
  user-facing SDK without claiming the bare name.

The CLI binary is already `ergo` (in `[[bin]]` of `ergo-cli`), so
either crate name works without binary collision. Decision must land
before PUB-7.

## Versioning Decision (Q-VERSION)

Two viable postures:

- `0.1.0` — standard pre-1.0 cargo semver; minor bumps may break.
- `0.1.0-alpha.1` — pre-release suffix; signals "shipping for users to
  try, no stability promise."

Recommended: alpha for first publish so the SDK public-surface audit
(PUB-1) can iterate without burning numerical headroom. Decision must
land before PUB-7.

## Surface Philosophy Decision (Q-SURFACE)

The SDK's behavioral closure is decided (SDK-CANON-1/2/3 — SDK delegates
orchestration to host, doesn't invent a second execution model). The
SDK's surface closure is not. PUB-1 cannot classify `pub use` items
without first pinning which philosophy `ergo-sdk` follows:

- **Thin facade.** Host types ARE the SDK's public contract. Re-exports
  are intentional. The SDK is a curated window into host; users get
  real types with no wrapping. Host crates' semver discipline becomes
  part of the SDK's semver discipline by transitivity. Zero abstraction
  overhead; no drift between SDK and underlying system.
- **Thick wrapper.** SDK exposes branded types (`SdkError`,
  `SdkConfig`, etc.) that hold or convert from host types. Host types
  are not part of the SDK's normal public matching contract. Explicit
  authoring/configuration carve-outs can still remain direct SDK API
  where documented. Wrapping code and conversion ceremony; SDK can evolve
  independently of host.

Q-SURFACE is load-bearing for PUB-1's classification table. Must land
before classification has meaning. Feeds PUB-3.

### Host-crate semver propagation (Q-SURFACE corollary)

If Q-SURFACE resolves as thin facade, the non-SDK crates reachable
through SDK public surface become part of the public contract by
transitivity. PUB-3's decision record must state whether these are
"publicly published crates that happen to be re-exported through SDK" or
"internal crates exposed only through SDK with no independent public
contract." The answer governs how breaking changes propagate: a
host-type signature change requires a bump on host alone, on SDK alone,
or on both.

## Audit User Decision (Q-USER)

PUB-1's per-type intent justification requires a concrete consumer to
answer against. Two postures:

- **Hypothetical external user.** Threshold is "would this be
  reasonable to expose to an unknown future user." Decisions get made
  on aesthetic grounds; the filter is fuzzy.
- **Concrete known consumer.** The only known external user is
  Sebastian, building applications against Ergo. Per-type intent
  justification answers against that single concrete consumer.

Stated audit user for PUB-1: **Sebastian as concrete consumer.** This
is a sharper filter and produces auditable answers. If a type has no
defensible answer to "what does Sebastian do with this when he writes
his app against Ergo," it is an accident, not a decision.

**Publish-grade rigor, not publish commitment.** Q-USER stays at
"Sebastian as concrete consumer" through PUB-1. The publish-target
framing only calibrates the rigor of the audit — items get classified
as if they *could* end up external, not as if they are guaranteed
internal forever. The decision to actually push to crates.io is gated
at PUB-7 (see Order of Operations). If that gate lands as publish,
Q-USER may revise to widen as a deliberate downstream decision, not as
drift.

## PUB-1 Audit Methodology

For each public item reachable from `ergo-sdk` (direct or via
`pub use`), require **per-type intent justification**, not just intent
declaration. The justification must answer concretely:

1. What does the audit user (Q-USER) do with this type?
2. Is that what the SDK intends them to do?
3. Under the chosen philosophy (Q-SURFACE), is exposing it consistent?

If the answer to (1) is "I don't know" or "nothing in particular," the
item is an accident, not a decision. Accidents get pruned or
`#[doc(hidden)]` regardless of which Q-SURFACE philosophy wins.

This is the filter that distinguishes "thin SDK by design" from "thin
SDK by neglect."

### Validation case: historical `ergo-supervisor` `demo` feature leak

Earlier publish prep found that `crates/prod/core/host/Cargo.toml`
enabled `ergo-supervisor = { features = ["demo"] }` as a non-test
dependency. The `demo` feature gated `pub mod demo;` in
`crates/kernel/supervisor/src/lib.rs` behind
`#[cfg(any(test, feature = "demo"))]`. Result: `ergo_supervisor::demo`
was compiled into every consumer of the published host crate.

Under the methodology above, `demo` items have no defensible answer to
"what does Sebastian do with this when he writes his app against
Ergo." They are accidental surface. If PUB-1 does not flag them for
pruning, ungating, or `#[doc(hidden)]` (plus likely removing the
`features = ["demo"]` from host), the methodology is not working.

The mechanical fix lives outside PUB-1 (host stops enabling the demo
feature, or supervisor renames it to `internal-test-fixtures` and
narrows its consumers). PUB-1 owns the classification; PUB-3 owns the
decision. The fix itself lands as part of PUB-2 or as a separate
dev-work ledger row once classified.

**Resolution (2026-05-13):** This validation case is resolved at source by
removing the `ergo fixture run` CLI subcommand and its supporting host
module, which together were the sole production-side consumers of the
`features = ["demo"]` enablement on `ergo-supervisor`. After that work
lands, `cargo tree -e features -p ergo-host | rg demo` returns no matches
on production paths, and `ergo_supervisor::demo` is no longer reachable
from the published host crate.

**PUB-6 follow-up (2026-05-31):** first-publish dry-run prep found that
`ergo-supervisor` still had a self dev-dependency to expose the same
feature to its own integration tests. The feature, `src/demo/`, and
`src/fixture_runner.rs` were removed before first publish; the demo graph
helper now lives under `crates/kernel/supervisor/tests/support/`.

- Decision: [`docs/ledger/decisions/remove-vestigial-fixture-run.md`](../ledger/decisions/remove-vestigial-fixture-run.md)
- Plan: [`docs/plans/remove-vestigial-fixture-run.md`](remove-vestigial-fixture-run.md)
- Dev-work: [`docs/ledger/dev-work/closed/remove-vestigial-fixture-run.md`](../ledger/dev-work/closed/remove-vestigial-fixture-run.md)

PUB-1's classification pass should be run after this work merges, against
the cleaned surface, to confirm the methodology yields the same answer
(no `supervisor::demo` items reachable from the published host API).

### Pre-loaded inventory: `ergo-loader` public surface

The March in-memory-loader audit
(`docs/plans/in-memory-loader-decision-rationale.md` §0A, §0A.1, §0B,
and §0D) already enumerated the public loader surface that ships under
`pub mod discovery` and `pub mod io`, plus the in-memory and prepared
asset carrier types. Fold the following into PUB-1's classification
table without re-auditing from scratch:

- `ergo_loader::discovery::{ClusterDiscovery, InMemoryClusterDiscovery,
  discover_cluster_tree, discover_in_memory_cluster_tree,
  load_cluster_tree, resolve_cluster_candidates}`
- `ergo_loader::io::{LoaderError, LoaderIoError, LoaderDecodeError,
  LoaderDiscoveryError, FilesystemGraphBundle, InMemoryGraphBundle,
  PreparedGraphAssets, load_graph_sources,
  load_graph_assets_from_paths, load_in_memory_graph_sources,
  load_graph_assets_from_memory}`

Correction to the March list: `canonicalize_or_self` is already
`pub(crate)` and is not part of the public surface; it does not need
classification.

The classification still has to run against the chosen Q-SURFACE
philosophy and the Q-USER consumer, but the inventory is pre-loaded.

## Phased Work (rough sizing)

| ID | Name | Sized | Output |
|---|---|---|---|
| PUB-1 | SDK public-surface audit | M | Classified table of every public item reachable from the SDK source crate (direct or via `pub use`): keep / prune / wrap-in-typed-error / `#[doc(hidden)]`. Each row carries a per-type intent justification answered against Q-USER and consistent with Q-SURFACE. Demo-feature leak is the validation case; loader inventory is pre-loaded. |
| PUB-2 | Mechanical publish blockers | S | All `Cargo.toml` files have `description` and `repository.workspace = true`. All inter-workspace path deps carry `version = "x.y"`. `LICENSE-MIT` and `LICENSE-APACHE` symlinks present in each publishing crate dir. `ergo-test-support` marked `publish = false`. Independent of all Q-* decisions. |
| PUB-3 | Decision record | S | `docs/ledger/decisions/crates-io-publish-set.md` finalizing publish set, Q-NAMING, Q-VERSION, Q-SURFACE, Q-USER, host-crate semver propagation, and PUB-1's classification table. Names graph-builder future-surface debt without solving it. |
| PUB-4 | SDK rustdoc pass | M | Type and method docs for every kept public item from PUB-1. Doctests for `Ergo::builder`, `from_project`, `run_profile`, `replay`, `runner_for_profile`, `ProfileRunner` lifecycle. |
| PUB-5 | Per-crate user-facing READMEs | S/M | One README per published crate aimed at crates.io readers, not internal contributors. **Minimal stubs only until Q-SURFACE lands**; full READMEs after PUB-1 so they reflect the chosen philosophy. Kernel crate READMEs may stay terse and link out; SDK and CLI READMEs are the landing pages users see. |
| PUB-6 | Dry-run publishes in dependency order | S | `cargo publish --dry-run` from kernel up; fix anything that surfaces. Valid terminal state if PUB-7's go/no-go lands as no-go. |
| PUB-7 | First publish | S | **Gated on final go/no-go after PUB-1, PUB-2, PUB-4, PUB-5, and PUB-6 land.** If go: `cargo publish` in dep order: runtime → prod-duration → adapter → supervisor → loader → host → sdk-types → fixtures → ergo-sdk → cli. If no-go: hold at PUB-6; the audit still produced the spine. |

Sizing key: S ≈ one focused session; M ≈ multi-session.

## Order of Operations

1. **Q-SURFACE decision** (thin facade vs. thick wrapper). Must land
   before PUB-1 classification has meaning.
2. **Q-USER decision** stated explicitly (stated above as: Sebastian
   as concrete consumer, with publish-grade rigor). Confirm or revise
   before PUB-1 starts.
3. **PUB-1 classification** proceeds with philosophy and user pinned,
   calibrated to publish-grade rigor.
4. **Demo-feature leak** handled as PUB-1's validation case.
5. **Loader pre-loaded inventory** folded into PUB-1's table without
   re-audit.
6. **PUB-3 decision record** consumes Q-SURFACE, Q-USER, Q-NAMING,
   Q-VERSION, the host-crate semver propagation question, and PUB-1's
   classification table.
7. **PUB-4 (rustdoc) and PUB-5 (full READMEs)** proceed once Q-SURFACE
   has landed and PUB-1's keep-set is known.
8. **PUB-6 dry-runs** verify the publish chain mechanically.
9. **Final go/no-go gate before PUB-7.** Look at the classified
   surface; decide whether to push. If no-go, hold at PUB-6 — the
   audit and dry-run still produced the spine. If go, PUB-7 executes.

**PUB-2 (mechanical blockers) is independent** of all Q-* decisions
and runs in parallel as cheap motion while the decisions cook. Per-crate
READMEs land as minimal stubs under PUB-2 if needed; full README
content waits for Q-SURFACE.

## Open Questions and Risks

1. **Typed SDK errors vs. transparent re-export.** Resolved by
   [`sdk-error-surface-wrapping.md`](../ledger/decisions/sdk-error-surface-wrapping.md)
   and consumed by
   [`crates-io-publish-set.md`](../ledger/decisions/crates-io-publish-set.md):
   the SDK uses thick, `Ergo*` wrapped errors with opaque
   `ErgoErrorSource` source-chain preservation. The typed parent-accessor
   model was superseded before first publish.
2. **Host-crate semver propagation.** Resolved by
   [`crates-io-publish-set.md`](../ledger/decisions/crates-io-publish-set.md):
   non-SDK crates are public crates with their own semver, but they are
   not the primary user-facing SDK surface.
3. **Graph-builder future-surface debt.** The programmatic graph
   builder is deferred (see chat record). Once 0.1.0-alpha.1 ships,
   adding a builder is additive (non-breaking) at semver level, but
   the surface decision recurs: does the builder live as a new module
   in `ergo-sdk`, a new sibling crate (`ergo-graph-builder`), or
   loader-side? And does its public surface get classified under the
   same Q-SURFACE philosophy? PUB-3 names this question without
   solving it.
4. **Documentation source split.** `/docs/` is internal authoring
   docs. crates.io users see READMEs and rustdoc only. Decide whether
   any `/docs/` content should be mirrored into rustdoc (e.g., a
   condensed getting-started in the SDK's `lib.rs` `//!` header).
5. **`ergo init` SDK path injection.** Today the scaffold writes a
   relative `path = "..."` dependency. After publish the scaffold must
   default to a versioned crates.io dep and keep `--sdk-path` only as
   an opt-in for in-checkout development. Touched in PUB-7 or as a
   follow-on dev-work ledger.
6. **CI publish gate.** First publish should run from a tagged commit,
   not local. Out of scope for this plan, but flagged so it isn't
   discovered at PUB-7.

## References

- `docs/INDEX.md` — authority taxonomy
- `docs/system/kernel-prod-separation.md` — boundary contract
- `docs/ledger/dev-work/closed/sdk-rust.md` — closed SDK ledger
- `docs/ledger/dev-work/closed/in-memory-loader-phase-1.md` — closed
  loader phase ledger
- `docs/plans/in-memory-loader-decision-rationale.md` §0A, §0A.1, and
  §0B — source of the pre-loaded loader public-surface inventory
- `docs/authoring/getting-started-sdk.md` — current user workflow
  (still references `--sdk-path`)
- `AGENTS.md` — hardening posture and downstream audit rule (§4G)
