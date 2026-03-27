---
Authority: PROJECT
Version: v1
Last Updated: 2026-03-26
Owner: Documentation
Scope: Historical phase-4 design record for the in-memory loader workstream
Change Rule: Working record
---

# Phase 4 — Decision Record

Historical design-loop artifact for the shipped Phase 4 boundary choices.
Implemented closure is recorded in the
[closed delivery ledger](/Users/sebastian/Projects/ergo/docs/ledger/dev-work/closed/in-memory-loader-phase-1.md).

These decisions were locked by Sebastian + Claude before Phase 4 implementation
began.

---

## Decision 1: Transport-neutral error formatter

Change `summarize_expand_error` from `HashMap<(String, Version), PathBuf>` to
`HashMap<(String, Version), String>`. Change wording from "available cluster
files" to "available cluster sources."

Filesystem path passes `path.display().to_string()` as the label. In-memory
path passes the caller's `source_label`. For the live-prep lane, one formatter
serves both transports truthfully. Replay and DOT remain on their existing
filesystem-specific summary paths in this phase.

Update the tests that pin the old "files" wording.

## Decision 2: Rename the carrier type

`PrepareHostedRunnerRequest` → `PreparedGraphAssets`.

The type serves both `validate_graph` and `prepare_hosted_runner`. The old name
implied it belonged to one function. The new name describes what it IS — a
bundle of loader-validated graph assets — not what one consumer does with it.

## Decision 3: Loader declares, host re-exports

The loader crate declares `PreparedGraphAssets`. It includes a
`pub(crate) _sealed: ()` field. Only the loader's discovery functions can
construct it. The compiler prevents hand-building the struct outside the loader,
which prevents anyone from skipping the loader's invariants (conflict detection,
version filtering, discovery ordering).

The payload is also externally immutable: callers read it through accessor
methods (`root()`, `clusters()`, `cluster_diagnostic_labels()`) instead of
mutating the loaded graph assets in place.

The host crate re-exports `PreparedGraphAssets` via
`pub use ergo_loader::PreparedGraphAssets;` in the lower-level block of
`lib.rs` (around line 49, NOT in the canonical client-facing block).

Standard Rust pattern. Already done elsewhere in the codebase.

## Decision 4: Host exposes a loading function, not just consuming functions

The sealed type means callers can't construct `PreparedGraphAssets` by hand.
Without a host-level loading function, lower-level host callers would have to
drop down into the loader crate to obtain the type.

The host must expose functions that call the loader internally and return
`PreparedGraphAssets`:

```
pub fn load_graph_assets_from_paths(...) -> Result<PreparedGraphAssets, HostRunError>
pub fn load_graph_assets_from_memory(...) -> Result<PreparedGraphAssets, HostRunError>
```

These are thin wrappers. The host calls the loader, translates loader errors
into host errors, and returns the sealed type. The caller then passes it to
`validate_graph` or `prepare_hosted_runner`.

Flow: caller → host load function → loader → sealed type back to caller →
caller passes it to host validate/prepare function.

Lower-level callers can stay on the host crate for this lane, which preserves
the host-owned orchestration story. The loader lower-level surface still remains
public for advanced direct use, so the docs must not pretend it disappeared.

## Decision 5: Narrow the "one private helper" claim

The plan's claim that "exactly one private helper calls loader discovery from
paths" applies to the live-prep lane ONLY. Replay has its own discovery path.
DOT rendering has its own. This scope does not touch them. Do not overclaim.

## Decision 6: Diagnostic label fallback

If the error formatter looks up a label in `diagnostic_labels` and the key is
missing, fall back to displaying `"{id}@{version}"`. Do not crash. Do not
silently swallow the error.

This should be near-impossible because the sealed type guarantees the loader
populated both maps in the same discovery pass. The fallback is belt and
suspenders.

## Decision 7: Tests must verify ordering, not just buckets

Parity tests for the new object seam must verify error precedence, not just
that the right error types come back:

- `AdapterRequired` fires BEFORE runner construction
- Runner construction fires BEFORE egress startup
- Validation stops BEFORE any of the above

Take scenarios that produce `AdapterRequired` on `*_from_paths`. Run the same
scenarios through the new object seam. Verify identical error and identical
ordering. Same for other error stages.

## Decision 8: Update 07-orchestration.md

The new lower-level public functions (`load_graph_assets_from_memory`,
`validate_graph`, `prepare_hosted_runner`) must appear in
`docs/invariants/07-orchestration.md` in the lower-level section. Not in the
canonical section. The doc already has this distinction. Add the entries.

---

## Summary

The architecture is:

1. Caller calls `host.load_graph_assets_from_memory(root_source_id, sources, search_roots)` → gets sealed `PreparedGraphAssets`
2. Caller constructs `LivePrepOptions`
3. Caller calls `host.validate_graph(&assets, &options)` → validation result
4. Caller calls `host.prepare_hosted_runner(assets, &options)` → `HostedRunner`
5. Caller steps the runner as before

The loader creates. The host wraps and re-exports the lower-level asset lane.
The sealed type enforces construction invariants and, with accessor-only reads,
prevents external mutation of the loaded asset payload. The live-prep error
formatter is transport-neutral; replay and DOT intentionally remain on their
own summary paths. The tests verify ordering parity. The docs reflect the new
surface.
