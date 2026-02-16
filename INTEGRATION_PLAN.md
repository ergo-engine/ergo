# Integration Plan: Wire YAML CLI Through Supervisor

**Status:** Agreed by Claude + Codex. Revised per Grok review (two rounds, 7 findings addressed). Pending final sign-off.

---

## Closure Target

Make `ergo run <graph.yaml>` execute via the canonical production path: `CapturingSession(Supervisor) → RuntimeHandle → validate/execute → RunTermination`. The current direct `runtime::run()` path becomes opt-in debug mode via `--direct`.

---

## Locked Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Supervised mode does not print graph outputs | RuntimeHandle::run() returns only RunTermination (SUP-2). This is intentional, not a regression. |
| 2 | Supervised mode prints orchestration/capture summary | Decision log entries, episode count, capture bundle path. |
| 3 | `--direct` flag enables one-shot runtime output printing | Explicit opt-in for the non-canonical debug path. |
| 4 | `--fixture <path>` required for supervised mode (now) | No implicit synthetic events in canonical mode. Live adapter comes later. |
| 5 | No implicit default event in canonical mode | Prevents "fake adapter syndrome" — CLI must not silently fabricate events. |
| 6 | Default `ergo run graph.yaml` routes through Supervisor | Direct runtime path is non-canonical. |

---

## Implementation Steps

```
ergo run <graph.yaml> --fixture <events.jsonl> [--adapter <adapter.yaml>] [--cluster-path <path> ...]
ergo run <graph.yaml> --direct  (debug mode, bypasses supervisor)
```

### Step 1: Parse args and mode

Parse CLI arguments. Determine mode: supervised (default, requires `--fixture`) or direct (`--direct` flag). Error if supervised mode and no `--fixture` provided. Error if `--direct` and `--fixture` are both provided — they are mutually exclusive. Error if `--direct` and `--capture-output` are both provided — capture artifacts are only produced in supervised mode. `--direct` means single-shot runtime execution with output printing; `--fixture` means supervised execution with capture.

**File:** `crates/ergo-cli/src/graph_yaml.rs` (modify `parse_run_options`)

### Step 2: Parse YAML and load clusters

Unchanged from current implementation. Parse YAML into `ClusterDefinition`, load nested clusters via `ClusterTreeBuilder`, resolve cluster paths.

**File:** `crates/ergo-cli/src/graph_yaml.rs` (existing code)

### Step 3: Expand to ExpandedGraph

Unchanged. Call `expand(&root, &loader, &catalog)`. Optional adapter composition checks if `--adapter` provided.

**File:** `crates/ergo-cli/src/graph_yaml.rs` (existing code)

### Step 4: Build RuntimeHandle

Construct `RuntimeHandle::new(Arc::new(expanded), Arc::new(catalog), Arc::new(registries), adapter_provides)`.

**File:** `crates/ergo-cli/src/graph_yaml.rs` (new code, replaces direct runtime::run call in supervised mode)

### Step 5: Build CapturingSession

Construct `CapturingSession::new(GraphId::new(cluster_id), constraints, decision_log, runtime_handle)`. Constraints default for now (no rate limits, no concurrency caps).

**File:** `crates/ergo-cli/src/graph_yaml.rs` (new code)

### Step 6: Parse fixture file into events

Read the `.jsonl` fixture file. Parse into `ExternalEvent` instances using `ergo_adapter::fixture::parse_fixture(path)` (in `crates/adapter/src/fixture.rs`, line 38). This returns `Vec<FixtureItem>` which contains episode boundaries and event data. Convert each event item into an `ExternalEvent` using the adapter's public factory methods.

Match fixture validation semantics from `fixture_runner`:
- Reject fixture with zero events.
- If `EpisodeStart` markers are used, reject any episode that has zero events.
- If events appear before the first `EpisodeStart`, assign them to an implicit first episode label (current fixture-runner behavior).

Note: `fixture_runner::run_fixture()` is not reusable here because it hardcodes the demo_1 graph and owns the full session lifecycle. The fixture *parsing* from `ergo_adapter::fixture` is the reusable piece; the event-feeding loop is new code in `graph_yaml.rs`.

**File:** `crates/ergo-cli/src/graph_yaml.rs` (new code using `ergo_adapter::fixture::parse_fixture`)

### Step 7: Feed events into session

Iterate over the parsed `Vec<FixtureItem>`. Only `FixtureItem::Event` items become `ExternalEvent` instances fed to `session.on_event(event)`. `FixtureItem::EpisodeStart` items are metadata — they mark episode boundaries for summary reporting (e.g., counting episodes in the orchestration summary printed in Step 8) but are not fed to the supervisor. The supervisor applies constraints, decides invoke/defer, calls `RuntimeHandle::run()` if invoking, logs every decision.

**File:** `crates/ergo-cli/src/graph_yaml.rs` (new code)

### Step 8: Retrieve and persist capture bundle

Call `session.into_bundle()`. Write the `CaptureBundle` to disk as JSON.

Default artifact path must be safe and remain under `target/`:
- Derive from graph file stem (not cluster id), e.g. `target/<graph-file-stem>-capture.json`.
- Sanitize stem to `[A-Za-z0-9_-]` (replace all other chars with `_`) before path assembly.

Override via `--capture-output <path>` flag. Print orchestration summary: episodes invoked, episodes deferred, capture bundle path.

**File:** `crates/ergo-cli/src/graph_yaml.rs` (new code)

### Step 9: --direct mode (debug path)

If `--direct` flag: skip steps 4-8, call `runtime::run()` directly as today, print outputs. This is the existing behavior, relocated behind a flag.

**File:** `crates/ergo-cli/src/graph_yaml.rs` (existing code, gated behind flag)

### Step 10: Update CLI usage/help text

Update the `usage()` string in `crates/ergo-cli/src/main.rs` (line 103) to reflect the new flags: `--fixture <events.jsonl>`, `--direct`, `--capture-output <path>`. The current usage string does not mention any of these. Also update any `--help` output or error messages that reference `ergo run` syntax.

**File:** `crates/ergo-cli/src/main.rs` (modify `usage()`)

### Step 11: replay_checked() remains unchanged

`replay_checked(bundle, runtime)` constructs a fresh `Supervisor::with_runtime(...)` with a `MemoryDecisionLog`, feeds rehydrated events via `supervisor.on_event()`, and returns the replayed decision records. It does not compare decisions itself — the caller performs the comparison (e.g., `main.rs:257` checks `records == &bundle.decisions`). It verifies scheduling determinism, not output equivalence. No changes needed.

**File:** `crates/supervisor/src/replay.rs` (no changes)

### Step 12: Verification and regression tests

Add/adjust tests and command-level checks to lock closure behavior:
- Supervised mode requires `--fixture` (missing fixture returns clear error).
- `--direct` and `--fixture` are mutually exclusive.
- Supervised run writes capture artifact (default path and `--capture-output` override).
- Produced bundle can be replay-checked and decision comparison passes.
- `--direct` mode preserves current one-shot output printing behavior.

Run at minimum:
- `cargo test -p ergo-cli`
- `cargo test -p ergo-supervisor`

---

## Invariant Compliance

| Invariant | How Satisfied |
|-----------|---------------|
| CXT-1 | `ExecutionContext::new()` is `pub(crate)` in ergo-adapter — CLI cannot construct arbitrary runtime contexts. However, CLI *can* emit synthetic events via public `ExternalEvent` constructors (`mechanical`, `with_payload`). This is a discipline boundary, not absolute impossibility. Mitigated by requiring `--fixture` in canonical mode: no implicit synthetic events. |
| SUP-2 | `RuntimeHandle::run()` consumes `ExecutionReport` internally, returns only `RunTermination`. Supervisor never sees graph outputs. |
| SUP-3 | Every decision logged via `DecisionLog`. `CapturingSession` captures events + decisions into `CaptureBundle`. Replay via `replay_checked()` verifies identical scheduling. |
| SUP-7 | `DecisionLog` trait exposes only `fn log(&self, entry)`. No read/query surface. |

---

## Failure Modes Guarded Against

**Fake adapter syndrome:** CLI constructing events/contexts ad-hoc. Partially prevented by `pub(crate)` on `ExecutionContext::new()`. Mitigated by requiring `--fixture` in canonical mode — no implicit synthetic events. The boundary remains convention/discipline: `ExternalEvent` constructors are public, so code review is the final enforcement layer against ad-hoc event fabrication outside fixtures.

**Two-path relapse:** Direct runtime path remains default and supervised path becomes optional. Prevented by making supervised the default and `--direct` the explicit opt-in.

---

## What Does Not Change

- YAML parser (`graph_yaml.rs` parsing logic)
- Cluster tree loading and resolution
- `expand()` and validation
- Supervisor internals (`lib.rs`)
- `CapturingSession` / `CapturingDecisionLog` (`capture.rs`)
- `replay_checked()` (`replay.rs`)
- `RuntimeHandle` / `RuntimeInvoker` trait (`adapter/src/lib.rs`)
- Frozen specifications

---

## Open for Later

- Live adapter event source (replaces `--fixture` for production)
- Constraints configuration via CLI flags or YAML
- Output observation side channel (if supervised mode needs to show outputs without violating SUP-2)
- `Decision::Skip` emission logic in supervisor
