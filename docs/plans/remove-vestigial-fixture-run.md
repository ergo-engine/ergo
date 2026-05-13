---
Authority: PROJECT
Date: 2026-05-13
Plan-Owner: Sebastian (Architect)
Recorder: Auggie (Structural Auditor)
Status: ACTIVE
Scope: v1, pre-PUB-1
Parent-Decision: docs/ledger/decisions/remove-vestigial-fixture-run.md
Related-Plan: `docs/plans/crates-io-publish.md` (PUB-1 validation case)
Related-Doctrine: `docs/authoring/project-convention.md` (SDK-as-product),
  `docs/ledger/decisions/sdk-error-surface-wrapping.md` (Q-SURFACE)
Affects: `ergo-cli`, `ergo-host`, `ergo-supervisor`, authoring docs
Touches-Kernel-Surface: no (only removes a `feature = "demo"` enablement; no
  semantic kernel changes)
---

# Plan: Remove Vestigial `ergo fixture run` Command

This plan defines the removal of the `ergo fixture run` CLI subcommand:
technical scope, preservation invariants, doc sync, ledger entries, PUB-1
sequencing, verification gates, and the cross-link to
`docs/plans/crates-io-publish.md`.

---

## 1. Background and Justification

### 1.1 What the command does

`ergo fixture run <events.jsonl>` runs the hardcoded built-in `demo_1` graph
(15 nodes, four fixed-value number sources, two compute, two trigger, two
action) against a user-supplied JSONL event timeline and writes a capture
artifact. The graph is not user-supplied; only the event tape is.

This is functionally distinct from the canonical
`ergo run <graph.yaml> -f <events.jsonl>` path, which takes both the graph
and the fixture from the user.

### 1.2 Why it does not earn its keep in v1

- **SDK-as-product framing.** Per `docs/authoring/project-convention.md`, the
  product surface is the Rust SDK; the CLI is supporting tooling. A
  graph-locked smoke harness has no SDK analogue and no path through
  `ergo init`'s scaffolded project.
- **Host self-disclaims canonical role.** The header of
  `crates/prod/core/host/src/demo_fixture_usecase.rs` reads: "This is a demo
  convenience path, not a canonical host API surface."
- **Kernel feature-gates the dependency.** `crates/kernel/supervisor/src/lib.rs`
  gates `pub mod demo;` and `pub mod fixture_runner;` behind
  `#[cfg(any(test, feature = "demo"))]`. Two production crates
  (`ergo-host`, `ergo-cli`) currently enable that test feature, dragging
  `supervisor::demo` into every consumer of the published host crate.
- **PUB-1 has already classified this as the validation case.** See
  `docs/plans/crates-io-publish.md`, "Validation case: `ergo-supervisor`
  `demo` feature leak" (lines 169–188).
- **Adjacent commands cover the legitimate use cases.** `ergo run --fixture`
  covers user-graph fixture execution. `ergo fixture inspect` and
  `ergo fixture validate` cover graph-agnostic fixture operability and
  remain in scope.

### 1.3 What this plan does not do

- Does not remove the `ergo fixture` namespace; `inspect` and `validate`
  remain.
- Does not remove the `supervisor::demo` module or the `demo` feature flag
  itself; supervisor's own integration tests (`tests/integration.rs`,
  `tests/replay_harness.rs`) still depend on `cfg(any(test, feature = "demo"))`
  via supervisor's dev-dependency on itself.
- Does not modify any kernel semantics.
- Does not touch `csv-to-fixture`, which is graph-agnostic.

### 1.4 Decisions made without asking

The following three judgment calls were made inside this plan rather than
escalated. Each is recorded here so the reader can override before approving
implementation. The same items appear (with the same wording) in §10.

1. **The `demo` feature flag and `supervisor::demo` module stay.** Only the
   production-side enablement (`features = ["demo"]` on `ergo-supervisor`
   from `ergo-host` and `ergo-cli`) is removed. Reason: supervisor's own
   integration tests (`tests/integration.rs`, `tests/replay_harness.rs`)
   depend on the gated modules via supervisor's dev-dependency on itself.
   Removing the flag would break in-tree test scaffolding without a payoff
   relevant to PUB-1, which only cares about reachability from the
   published host crate.

2. **`demo_1` references stay in `docs/authoring/yaml-format.md`.** The
   "Canonical Example (Demo 1)" section (line 38 onward) is **not**
   touched. Only the "Current CLI contract" line (line 15) is amended to
   drop the `ergo fixture run` clause. Reason: `demo_1` is the
   YAML-format litmus test independent of the CLI command. The doc
   describes a YAML schema, not a CLI surface; the litmus test stays.

3. **`RunDemoFixtureRequest` is deleted, not retained as `#[doc(hidden)]`.**
   Reason: confirmed via grep that
   `crates/prod/clients/sdk-rust/src/lib.rs` does not re-export it, so it
   has no SDK consumer. `#[doc(hidden)]` would leave the type compiled and
   reachable via `pub use` paths from `ergo-host`, which is exactly the
   PUB-1 leak class we're closing. Removal is cleaner.

If any of these three decisions is wrong, flag before approving. The plan
itself is robust to flipping (1) or (3) with a follow-up pass; flipping (2)
would mean editing `yaml-format.md` more aggressively, which is also
mechanical.

---

## 2. Technical Scope of Removal

All paths are repository-relative. Each item is addressed in the implementation
sequence in §8.

### 2.1 CLI surface (`crates/prod/clients/cli/`)

| File | Change |
|------|--------|
| `Cargo.toml` (line 23) | Remove `, features = ["demo"]` from the `ergo-supervisor` dependency. If supervisor becomes unused at the production-dependency level, demote to `[dev-dependencies]` only. |
| `src/main.rs` (line 29) | Remove `#[cfg(test)] const DEMO_GRAPH_ID: &str = "demo_1";`. |
| `src/cli/dispatch.rs` (lines 53–98) | Remove the `"run"` arm of the `"fixture"` match. Keep the `"fixture"` outer arm so `inspect`/`validate` continue to dispatch. The `_ => invalid_fixture_subcommand(...)` fallback remains and now naturally rejects `run`. |
| `src/cli/dispatch.rs` (lines 149–152) | Update the `"run"` → `"fixture"` redirect (see §3.2 for new error text). |
| `src/cli/handlers.rs` (lines 1–39) | Remove the `run_fixture` handler, the `FixtureRunSummary` struct, and the import of `run_demo_fixture_from_path`/`RunDemoFixtureRequest`. Keep `replay_graph` and its imports. |
| `src/fixture_ops.rs` (lines 45–52) | Remove the `ergo fixture run …` line from `fixture_usage()`. Keep the `inspect` and `validate` lines. |
| `src/output/text.rs` (line 18, lines 59–61) | Remove the `ergo fixture run` line from `usage()`. In `help_topic`, drop the `"fixture run"` alias from the match arm; keep `"fixture"`, `"fixture inspect"`, `"fixture validate"`. |
| `src/output/text.rs` | Remove `render_fixture_run_summary` if no other caller remains (audit). |
| `src/tests.rs` (line 7, lines 633, 658, 698) | Remove the four `fixture_run_*` tests that exercise the demo path: `fixture_run_creates_capture_via_host_runner`, `fixture_run_pretty_capture_output_is_multiline`, `fixture_run_short_o_overrides_output_path`, `fixture_run_default_output_path_is_capture_named`. Keep `usage_moves_fixture_to_top_level_subcommand` and `help_topic_fixture_matches_fixture_usage` but update their assertions to no longer expect `fixture run` in the help/usage strings. |
| `tests/fixture_binary_smoke.rs` (entire file) | Remove the four binary-smoke tests (`fixture_run_empty_fixture_returns_cli_error`, `fixture_run_single_event_auto_creates_episode_and_writes_capture`, `fixture_run_episode_start_without_events_returns_cli_error`, `fixture_run_back_to_back_episode_starts_returns_cli_error`). The whole file can be removed if no other binary-smoke tests live in it; otherwise, remove only those functions. |

### 2.2 Host surface (`crates/prod/core/host/`)

| File | Change |
|------|--------|
| `Cargo.toml` (line 12) | Remove `, features = ["demo"]` from the `ergo-supervisor` dependency. |
| `src/lib.rs` | Remove the `mod demo_fixture_usecase;` declaration and the `pub use demo_fixture_usecase::{run_demo_fixture_from_path, RunDemoFixtureRequest};` re-export (line 48). |
| `src/demo_fixture_usecase.rs` (entire file, 119 lines) | Delete. |

### 2.3 Supervisor (`crates/kernel/supervisor/`)

No changes. The `demo` feature stays defined; the `#[cfg(any(test, feature = "demo"))]` gates remain. Supervisor's own `[dev-dependencies]` self-reference with `features = ["demo"]` is unchanged. This is intentional: the kernel test scaffolding is still useful in-tree; only the production-layer enablement is removed.

### 2.4 Hardcoded `demo_1` references in production code

After §2.1 and §2.2, the only remaining production-code references to
`demo_1` should be in `crates/kernel/adapter/src/fixture.rs` (line 206,
inside test-only `fixture_output_path` example usage) and in supervisor's own
`#[cfg(any(test, feature = "demo"))]` modules. Verify with:

```
rg -n 'demo_1|DEMO_GRAPH_ID' crates/prod/ crates/kernel/adapter/src/
```

Any unguarded production hit outside supervisor's gated demo modules is a
finding; the implementer should report it before proceeding rather than
silently delete it.

### 2.5 Out-of-scope adjacent surfaces (audit, do not change)

- `crates/prod/clients/sdk-rust/src/lib.rs` — confirmed not to re-export
  `run_demo_fixture_from_path` or `RunDemoFixtureRequest`. No SDK change.
- `crates/prod/clients/sdk-rust/src/tests.rs` (line 20) — uses
  `ergo_host::PROCESS_DRIVER_PROTOCOL_VERSION`, unrelated to the demo path.
- `csv-to-fixture` command — graph-agnostic, untouched.

---

## 3. Preservation Invariants

### 3.1 Sibling commands remain

`ergo fixture inspect` and `ergo fixture validate` are graph-agnostic and
operationally valuable. They:

- Stay in `src/fixture_ops.rs`.
- Stay in `dispatch.rs`'s `"fixture"` arm.
- Stay in `usage()` and `help_topic()`.
- Their tests in `src/fixture_ops/tests.rs` are not touched.

### 3.2 Redirect `removed_run_fixture` to the canonical path

`crates/prod/clients/cli/src/output/errors.rs` currently defines
`removed_run_fixture()` whose fix string points users at the command being
removed. After this work, the redirect must point at the canonical path.

Before:

```rust
pub fn removed_run_fixture() -> String {
    render_cli_error(
        &CliErrorInfo::new(
            "cli.command_removed",
            "'ergo run fixture' was removed in v1",
        )
        .with_where("command 'run fixture'")
        .with_fix("use 'ergo fixture run <events.jsonl>'"),
    )
}
```

After:

```rust
pub fn removed_run_fixture() -> String {
    render_cli_error(
        &CliErrorInfo::new(
            "cli.command_removed",
            "'ergo run fixture' was removed in v1",
        )
        .with_where("command 'run fixture'")
        .with_fix("use 'ergo run <graph.yaml> -f <events.jsonl>'"),
    )
}
```

A second error helper for the now-removed `ergo fixture run` form is added
so users typing the old subcommand get a deterministic redirect rather than
the generic `invalid_fixture_subcommand` message:

```rust
pub fn removed_fixture_run() -> String {
    render_cli_error(
        &CliErrorInfo::new(
            "cli.command_removed",
            "'ergo fixture run' was removed in v1",
        )
        .with_where("command 'fixture run'")
        .with_fix("use 'ergo run <graph.yaml> -f <events.jsonl>'"),
    )
}
```

In `dispatch.rs`'s `"fixture"` arm, the `_ => invalid_fixture_subcommand(...)`
fallback gains an explicit case
`"run" => Err(output::errors::removed_fixture_run())` so the redirect is
deterministic regardless of argv order.

### 3.3 Behavioral invariants that must hold post-removal

- `ergo run <graph.yaml> -f <events.jsonl>` continues to work for any
  user-authored graph.
- `ergo fixture inspect` / `ergo fixture validate` semantics unchanged.
- `ergo run fixture …` continues to return a redirect error (now updated to
  point at the canonical path).
- `ergo fixture run …` returns a redirect error rather than dispatching.
- The `ergo` binary built with `--release` no longer compiles
  `ergo_supervisor::demo` or `ergo_supervisor::fixture_runner`.


---

## 4. Documentation Sync

### 4.1 `docs/authoring/yaml-format.md`

**Line 15** currently reads:

> **Current CLI contract:** `ergo run <graph.yaml> (-f|--fixture <events.jsonl> | --driver-cmd <program> [--driver-arg <value> ...]) [-a|--adapter <adapter.yaml>] [--egress-config <egress.toml>] [-o|--capture|--capture-output <path>] [-p|--pretty-capture] [--cluster-path|--search-path <path> ...]` and `ergo fixture run <events.jsonl> [-o|--capture|--capture-output <path>] [-p|--pretty-capture]` (current flag names retain legacy `driver` terminology)

Replace with:

> **Current CLI contract:** `ergo run <graph.yaml> (-f|--fixture <events.jsonl> | --driver-cmd <program> [--driver-arg <value> ...]) [-a|--adapter <adapter.yaml>] [--egress-config <egress.toml>] [-o|--capture|--capture-output <path>] [-p|--pretty-capture] [--cluster-path|--search-path <path> ...]` (current flag names retain legacy `driver` terminology). For graph-agnostic fixture operability, see `ergo fixture inspect` and `ergo fixture validate`.

The "Canonical Example (Demo 1)" section (line 38 onward) is **not** modified.
`demo_1` remains the litmus test for the YAML format itself; this plan only
removes the CLI command that hardcoded that graph.

### 4.2 `docs/authoring/project-convention.md`

**Lines 244–247** currently read:

```
- validate
- replay
- fixture runs
- optional project-mode convenience commands
```

Replace `fixture runs` with a more precise entry that does not endorse a
graph-locked subcommand:

```
- validate
- replay
- fixture-driven `run` (`ergo run <graph.yaml> -f <events.jsonl>`)
- fixture inspect / validate
- optional project-mode convenience commands
```

Line 19 ("The CLI remains a support tool for validation, replay, fixture
runs, and other development conveniences") is left as-is; the phrasing is
generic and accurate.

### 4.3 Other docs

- `docs/ledger/dev-work/closed/host-stop-lifecycle.md` — references "fixture
  runs" generically (lines 18, 37). No change required; the references are
  about run-lifecycle stop semantics, which apply equally to
  `ergo run --fixture`.
- `docs/ledger/dev-work/closed/ergo-init.md` — already does not surface
  `ergo fixture run`. No change.
- `docs/ledger/dev-work/open/in-memory-loader-phase-2.md` (lines 147–151,
  `IMT2-10`) — references
  `crates/prod/core/host/src/demo_fixture_usecase.rs` as a pinned data point
  ("demo-fixture remains path-only … so there is no in-memory demo-fixture
  lane"). Update this entry to read that the demo-fixture lane was removed
  outright in this work, citing the new DR by path. The `IMT2-10` argument
  ("no in-memory demo-fixture lane to resolve") is preserved by the removal,
  not invalidated by it.
- `docs/INDEX.md` — add an entry under the plans section linking to
  `docs/plans/remove-vestigial-fixture-run.md` and (once created) the DR
  under decisions.

### 4.4 README and crate docs

`crates/prod/clients/sdk-rust/README.md` does not mention the demo fixture
runner. Confirm at implementation time and leave unchanged if so.

---

## 5. Ledger and Tracking

### 5.1 Decision Record (new)

**Path:** `docs/ledger/decisions/remove-vestigial-fixture-run.md`

**Header (matches the convention used in `host-stop-lifecycle.md`):**

```markdown
---
Authority: PROJECT
Date: 2026-05-13
Decision-Owner: Sebastian (Architect)
Recorder: Auggie (Structural Auditor)
Status: PROPOSED
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
```

**Body sections (in order):**

1. **Context.** SDK is the product surface; CLI is supporting tooling. The
   `ergo fixture run` subcommand runs a hardcoded `demo_1` graph against a
   user-supplied fixture, providing nothing a user with their own graph can
   use. The host module that backs it self-disclaims canonical role. The
   command is the sole reason `ergo-host` and `ergo-cli` enable the
   supervisor `demo` test feature, dragging `supervisor::demo` into every
   consumer of the published host crate. PUB-1 has already classified this as
   the validation case for the publish methodology.

2. **Decision.** Remove `ergo fixture run` from the CLI. Remove
   `crates/prod/core/host/src/demo_fixture_usecase.rs` and its re-exports.
   Drop the `features = ["demo"]` enablement on `ergo-supervisor` from
   both `ergo-host` and `ergo-cli` non-test dependency lines. Preserve
   `ergo fixture inspect` and `ergo fixture validate`. Repoint the
   `removed_run_fixture` redirect at the canonical
   `ergo run <graph.yaml> -f <events.jsonl>`.

3. **Consequences.**
   - PUB-1's validation case is resolved at source rather than papered over
     with `#[doc(hidden)]` or feature renaming.
   - `ergo-host` no longer transitively publishes `supervisor::demo` items.
   - The `supervisor::demo` and `supervisor::fixture_runner` modules remain
     in-tree as test scaffolding (gated by `cfg(any(test, feature = "demo"))`),
     usable from supervisor's own dev-dependencies without leaking to prod.
   - Users who had wired the demo command into scripts get a deterministic
     CLI redirect to the canonical path.

4. **Non-goals.** This decision does not remove the `demo` feature flag,
   the `supervisor::demo` module, the `demo_1` graph helpers, or the
   "Canonical Example (Demo 1)" section of the YAML doc. Those remain
   useful as test scaffolding and as the YAML-format litmus test.

5. **Alternatives considered.**
   - *Keep the command, mark host module `#[doc(hidden)]`.* Rejected: leaves
     the production-layer `features = ["demo"]` enablement intact, which is
     the actual leak surfaced by PUB-1.
   - *Keep the command, rename supervisor's `demo` feature to
     `internal-test-fixtures`.* Rejected as insufficient: the production
     consumer relationship is the issue, not the feature name.
   - *Keep the command but require a user-supplied graph.* Rejected:
     duplicates `ergo run --fixture` with no added behavior.


### 5.2 Dev-work tracking entries

**Path:** `docs/ledger/dev-work/open/remove-vestigial-fixture-run.md`
(moves to `closed/` once all rows are CLOSED).

**Header (matches the convention used in `ergo-init.md`):**

```markdown
---
Authority: PROJECT
Date: 2026-05-13
Author: Sebastian (Architect) + Codex (Build Orchestrator)
Status: OPEN
Branch: chore/remove-vestigial-fixture-run
Tier: 4 (Production hardening — surface cleanup)
Depends-On: >-
  docs/ledger/decisions/remove-vestigial-fixture-run.md;
  docs/plans/remove-vestigial-fixture-run.md
Unblocks: PUB-1 validation case in docs/plans/crates-io-publish.md
---

# Dev-Work: Remove `ergo fixture run` Subcommand
```

**Rows:**

| ID | Title | Acceptance | Owner | Status |
|----|-------|------------|-------|--------|
| RFR-1 | Drop CLI dispatch + handler | `dispatch.rs` `"fixture"` arm has no `"run"` branch; `handlers.rs::run_fixture` and `FixtureRunSummary` removed; CLI builds. | Codex | OPEN |
| RFR-2 | Update CLI usage / help text | `usage()`, `fixture_usage()`, and `help_topic` no longer mention `fixture run`; tests updated to match. | Codex | OPEN |
| RFR-3 | Update redirect errors | `removed_run_fixture` fix string points at `ergo run <graph.yaml> -f <events.jsonl>`; new `removed_fixture_run` helper added; dispatch wires it under `"fixture"` arm. | Codex | OPEN |
| RFR-4 | Remove host demo-fixture module | `crates/prod/core/host/src/demo_fixture_usecase.rs` deleted; `host/src/lib.rs` re-exports removed. | Codex | OPEN |
| RFR-5 | Drop `features = ["demo"]` enablement | `ergo-host/Cargo.toml` line 12 and `ergo-cli/Cargo.toml` line 23 no longer carry `features = ["demo"]` on the production `ergo-supervisor` dependency. | Codex | OPEN |
| RFR-6 | Doc sync | `docs/authoring/yaml-format.md` and `docs/authoring/project-convention.md` updated per §4; `docs/ledger/dev-work/open/in-memory-loader-phase-2.md` IMT2-10 note updated; `docs/INDEX.md` cross-links added. | Sebastian | OPEN |
| RFR-7 | Verification gates | All gates in §7 pass: workspace `cargo check`, `cargo test --workspace`, `cargo build --release -p ergo-cli`, dependency-tree audit, `rg` audits. | Codex | OPEN |
| RFR-8 | Cross-link from PUB-1 plan | `docs/plans/crates-io-publish.md` validation-case section gains the cross-link defined in §7 of this plan; the PUB-1 row is updated to reference the resolution. | Sebastian | OPEN |

### 5.3 Closure

Once all RFR rows are CLOSED:

- Move `docs/ledger/dev-work/open/remove-vestigial-fixture-run.md` to
  `docs/ledger/dev-work/closed/`.
- Flip the new DR's `Status:` from `PROPOSED` to `DECIDED`.
- Update `docs/INDEX.md` to point at the closed dev-work entry.

---

## 6. PUB-1 Sequencing

**Verdict: mandatory prerequisite for PUB-1.**

Reasoning:

- `docs/plans/crates-io-publish.md` lines 169–188 names the
  `ergo-supervisor` `demo` feature leak as PUB-1's validation case. The
  plan explicitly states: "If PUB-1 does not flag them for pruning,
  ungating, or `#[doc(hidden)]` (plus likely removing the `features = ["demo"]`
  from host), the methodology is not working."
- The cleanest resolution — used as the ground-truth answer that PUB-1's
  classifier must reach — is removal at source. Performing this removal
  before PUB-1's classification pass means PUB-1 runs against the cleaned
  surface and validates that the methodology produces the same answer
  (no `supervisor::demo` reachable from published host).
- If the removal is deferred until after PUB-1, PUB-1 must invent a
  classification disposition (`#[doc(hidden)]`, feature rename) that the
  decision in §5.1 explicitly rejects as insufficient. That creates churn
  and risks shipping the first crates.io release with the leak intact.

**Sequencing constraint:** This dev-work (`chore/remove-vestigial-fixture-run`)
must merge before PUB-1's classification pass begins. If PUB-1 work is
already in flight, this work takes priority and PUB-1 rebases on it.

**Parallelism:** The doc-sync rows (RFR-6, RFR-8) can run in parallel with
the code rows (RFR-1 through RFR-5) because they touch disjoint files. RFR-7
is a gate, not a parallel row.

---

## 7. Verification Gates

### 7.1 Build and test gates

Run from repo root:

```
cargo check --workspace --all-targets
cargo test --workspace
cargo build --release -p ergo-cli
```

All three must pass with no warnings introduced by this change. The
`--release` build is the production-binary check that backs invariant 3.3
("the `ergo` binary built with `--release` no longer compiles
`ergo_supervisor::demo`").

### 7.2 Dependency-tree audit

Confirm the `demo` feature is no longer enabled on `ergo-supervisor` from
production paths:

```
cargo tree -p ergo-host --no-default-features
cargo tree -p ergo-cli  --no-default-features
cargo tree -e features -p ergo-host  | rg 'demo'
cargo tree -e features -p ergo-cli   | rg 'demo'
```

The two `rg` invocations should produce no matches (or, at most, matches
inside `[dev-dependencies]` paths if supervisor is retained as a dev-dep on
the CLI for tests).

### 7.3 Source audits

```
rg -n 'fixture run|fixture_run|run_demo_fixture|RunDemoFixtureRequest' \
   crates/prod/ docs/

rg -n 'DEMO_GRAPH_ID' crates/prod/

rg -n 'demo_1' crates/prod/ crates/kernel/adapter/src/
```

Expected post-removal state:

- First audit: hits only in `removed_run_fixture` / `removed_fixture_run`
  error helpers (the deliberate redirects), the new DR, the new dev-work
  entry, and this plan. No live call sites in production code.
- Second audit: zero hits in `crates/prod/`.
- Third audit: hits in `crates/kernel/adapter/src/fixture.rs` (test-only
  example) only; no hits in `crates/prod/`.

### 7.4 Tests removed

The following tests must be removed or updated (see §2.1 for details):

- `crates/prod/clients/cli/src/tests.rs`:
  - `fixture_run_creates_capture_via_host_runner` — removed
  - `fixture_run_pretty_capture_output_is_multiline` — removed
  - `fixture_run_short_o_overrides_output_path` — removed
  - `fixture_run_default_output_path_is_capture_named` — removed
  - `usage_moves_fixture_to_top_level_subcommand` — updated assertions
  - `help_topic_fixture_matches_fixture_usage` — updated assertions
- `crates/prod/clients/cli/tests/fixture_binary_smoke.rs`:
  - All four `fixture_run_*` tests — removed (file deleted if empty)
- New tests added under `crates/prod/clients/cli/src/output/errors` tests
  (or in `dispatch` tests):
  - `fixture_run_subcommand_returns_redirect_error` — confirms
    `ergo fixture run …` returns `removed_fixture_run()`
  - `removed_run_fixture_fix_points_at_canonical_run` — confirms the new
    fix string in `removed_run_fixture()`

### 7.5 Manual verification

After build, run the binary against three argument shapes:

```
./target/release/ergo fixture run path/to/anything.jsonl
./target/release/ergo run fixture path/to/anything.jsonl
./target/release/ergo fixture inspect path/to/anything.jsonl
```


Expected outcomes:

- The first two return the redirect error and exit non-zero.
- The third runs `inspect` normally.

---

## 8. Cross-Link to PUB-1 Plan

Add the following block to `docs/plans/crates-io-publish.md`, immediately
after the "Validation case: `ergo-supervisor` `demo` feature leak" section
(after the current paragraph ending "as a separate dev-work ledger row once
classified.").

```markdown
**Resolution (2026-05-13):** This validation case is resolved at source by
removing the `ergo fixture run` CLI subcommand and its supporting host
module, which together were the sole production-side consumers of the
`features = ["demo"]` enablement on `ergo-supervisor`. After that work
lands, `cargo tree -e features -p ergo-host | rg demo` returns no matches
on production paths, and `ergo_supervisor::demo` is no longer reachable
from the published host crate.

- Decision: [`docs/ledger/decisions/remove-vestigial-fixture-run.md`](../ledger/decisions/remove-vestigial-fixture-run.md)
- Plan: [`docs/plans/remove-vestigial-fixture-run.md`](remove-vestigial-fixture-run.md)
- Dev-work: [`docs/ledger/dev-work/closed/remove-vestigial-fixture-run.md`](../ledger/dev-work/closed/remove-vestigial-fixture-run.md)

PUB-1's classification pass should be run after this work merges, against
the cleaned surface, to confirm the methodology yields the same answer
(no `supervisor::demo` items reachable from the published host API).
```

While dev-work is OPEN, the `Resolution` block links should point at
`open/remove-vestigial-fixture-run.md`. RFR-8 includes flipping that link
to `closed/...` when the dev-work entry moves.

---

## 9. Execution Sequence (for the implementing agent)

This sequence assumes a fresh branch `chore/remove-vestigial-fixture-run`.

### Step 0 — Land this plan in the repo

This plan was authored in plan mode and copied into the workspace at
`docs/plans/remove-vestigial-fixture-run.md` as the first action of the
implementing pass.

### Step 1 — Land remaining planning artifacts (same commit as Step 0)

   1. Create `docs/ledger/decisions/remove-vestigial-fixture-run.md` from
      §5.1.
   2. Create `docs/ledger/dev-work/open/remove-vestigial-fixture-run.md`
      from §5.2.
   3. Add the cross-link block from §8 to
      `docs/plans/crates-io-publish.md`.
   4. Add INDEX entries per §4.3.

### Step 2 — Code removal (one logical commit)

   1. Delete `crates/prod/core/host/src/demo_fixture_usecase.rs`.
   2. Edit `crates/prod/core/host/src/lib.rs` to drop the module
      declaration and re-export.
   3. Edit `crates/prod/core/host/Cargo.toml` to drop
      `, features = ["demo"]` from the `ergo-supervisor` dependency.
   4. Edit `crates/prod/clients/cli/src/cli/dispatch.rs` to remove the
      `"run"` branch under the `"fixture"` arm and add the explicit
      redirect case (§3.2).
   5. Edit `crates/prod/clients/cli/src/cli/handlers.rs` to remove
      `run_fixture`, `FixtureRunSummary`, and the related imports.
   6. Edit `crates/prod/clients/cli/src/fixture_ops.rs` to drop the
      `fixture run` line from `fixture_usage()`.
   7. Edit `crates/prod/clients/cli/src/output/text.rs` to drop the
      `fixture run` line from `usage()` and the `"fixture run"` alias
      from `help_topic`. Remove `render_fixture_run_summary` if unused.
   8. Edit `crates/prod/clients/cli/src/output/errors.rs` to update
      `removed_run_fixture`'s fix string and add `removed_fixture_run`.
   9. Edit `crates/prod/clients/cli/src/main.rs` to remove the
      `DEMO_GRAPH_ID` constant.
   10. Edit `crates/prod/clients/cli/Cargo.toml` to drop
       `, features = ["demo"]` from the `ergo-supervisor` dependency.

### Step 3 — Test sync (same commit as Step 2 or follow-up)

   1. Remove the four demo-path tests in
      `crates/prod/clients/cli/src/tests.rs`.
   2. Update `usage_moves_fixture_to_top_level_subcommand` and
      `help_topic_fixture_matches_fixture_usage` to drop `fixture run`
      assertions.
   3. Remove `crates/prod/clients/cli/tests/fixture_binary_smoke.rs`
      (or its four `fixture_run_*` tests).
   4. Add the two new redirect-coverage tests per §7.4.

### Step 4 — Doc sync (same commit or follow-up)

   1. Edit `docs/authoring/yaml-format.md` per §4.1.
   2. Edit `docs/authoring/project-convention.md` per §4.2.
   3. Edit `docs/ledger/dev-work/open/in-memory-loader-phase-2.md`
      IMT2-10 note per §4.3.

### Step 5 — Verification (per §7)

### Step 6 — Closure (per §5.3)

Move dev-work entry to `closed/`, flip DR to `DECIDED`, repoint the
cross-link block in `docs/plans/crates-io-publish.md` from `open/` to
`closed/`.

---

## 10. Open Questions

None requiring user input before execution.

The three judgment calls made inside this plan are surfaced upfront in
§1.4 ("Decisions made without asking") so a reviewer sees them before
approving implementation. Summarized briefly:

- `demo` feature flag and `supervisor::demo` module stay; only the
  production-side `features = ["demo"]` enablement is removed.
- `demo_1` references in `docs/authoring/yaml-format.md` stay; only the
  "Current CLI contract" line is amended.
- `RunDemoFixtureRequest` is deleted, not retained as `#[doc(hidden)]`,
  because it has no SDK consumer.

See §1.4 for the full rationale and override criteria.

---

## 11. Risk Notes

- **Compatibility break for users scripting `ergo fixture run`.** Mitigated
  by the explicit redirect error pointing at `ergo run -f`. The message
  uses the `cli.command_removed` code, which matches the pattern already
  established for `ergo run fixture` in v1.
- **Dev-tree noise from `cargo tree`.** Supervisor remains a dev-dependency
  of CLI for tests; `cargo tree -e features` may still show `demo` under
  dev-only paths. Acceptable: the prod path is what PUB-1 cares about. The
  audit in §7.2 distinguishes the two.
- **In-flight worktree under `.claude/worktrees/hungry-haslett-b2590e`.** A
  copy of the same files exists there as session state. Not canonical;
  ignore. If it interferes with `rg` audits, scope audits to `crates/` and
  `docs/` explicitly.
