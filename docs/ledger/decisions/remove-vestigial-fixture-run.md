---
Authority: PROJECT
Date: 2026-05-13
Decision-Owner: Sebastian (Architect)
Recorder: Auggie (Structural Auditor)
Status: DECIDED
Scope: v1, pre-PUB-1
Parent-Decision: docs/authoring/project-convention.md (SDK-as-product)
Related-Decision: docs/ledger/decisions/sdk-error-surface-wrapping.md (Q-SURFACE)
Unblocks: PUB-1 validation case in docs/plans/crates-io-publish.md
---

# Decision: Remove the `ergo fixture run` CLI Subcommand

**Status:** Proposed
**Author:** Auggie (Structural Auditor), with Sebastian
**Date:** 2026-05-13
**Affects:** ergo-cli, ergo-host (no kernel semantic changes)

---

## Context

SDK is the product surface; CLI is supporting tooling. The
`ergo fixture run` subcommand runs a hardcoded `demo_1` graph against a
user-supplied fixture, providing nothing a user with their own graph can
use. The host module that backs it self-disclaims canonical role. The
command is the sole reason `ergo-host` and `ergo-cli` enable the
supervisor `demo` test feature, dragging `supervisor::demo` into every
consumer of the published host crate. PUB-1 has already classified this
as the validation case for the publish methodology.

## Decision

Remove `ergo fixture run` from the CLI. Remove
`crates/prod/core/host/src/demo_fixture_usecase.rs` and its re-exports.
Drop the `features = ["demo"]` enablement on `ergo-supervisor` from
both `ergo-host` and `ergo-cli` non-test dependency lines. Preserve
`ergo fixture inspect` and `ergo fixture validate`. Repoint the
`removed_run_fixture` redirect at the canonical
`ergo run <graph.yaml> -f <events.jsonl>`.

## Consequences

- PUB-1's validation case is resolved at source rather than papered over
  with `#[doc(hidden)]` or feature renaming.
- `ergo-host` no longer transitively publishes `supervisor::demo` items.
- The `supervisor::demo` and `supervisor::fixture_runner` modules remain
  in-tree as test scaffolding (gated by `cfg(any(test, feature = "demo"))`),
  usable from supervisor's own dev-dependencies without leaking to prod.
- Users who had wired the demo command into scripts get a deterministic
  CLI redirect to the canonical path.

## Non-goals

This decision does not remove the `demo` feature flag, the
`supervisor::demo` module, the `demo_1` graph helpers, or the
"Canonical Example (Demo 1)" section of the YAML doc. Those remain
useful as test scaffolding and as the YAML-format litmus test.

## Alternatives considered

- *Keep the command, mark host module `#[doc(hidden)]`.* Rejected: leaves
  the production-layer `features = ["demo"]` enablement intact, which is
  the actual leak surfaced by PUB-1.
- *Keep the command, rename supervisor's `demo` feature to
  `internal-test-fixtures`.* Rejected as insufficient: the production
  consumer relationship is the issue, not the feature name.
- *Keep the command but require a user-supplied graph.* Rejected:
  duplicates `ergo run --fixture` with no added behavior.
