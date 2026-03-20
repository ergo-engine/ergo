# Ergo embedded integration plan

Date: 2026-03-19
Status: Proposed
Participants: Sebastian, Claude, Codex

---

## Origin

An app developer building a Rust backend (async HTTP framework) tried to embed Ergo as a stateful execution engine. Their app already owns the data pipeline, UI, and storage. The core engine worked well — the friction was entirely at the integration boundary. The SDK is built for a CLI-first, filesystem-first, single-run workflow. Embedding it into a long-running server requires workarounds for things the SDK doesn't surface.

## Scope

All work lives in `crates/prod/` — host, loader, and SDK crates. Kernel is untouched. No frozen boundary pressure. I don't see ontological pressure from this scope.

## Key insight

`HostedRunner` already solves most of the embedded use case at the low level. `step()` takes `HostedEvent` in memory (in-memory ingress). Effects come back in `HostedStepOutcome` (step-level streaming). It's the canonical host execution surface, not a second execution model. The problem is that the SDK hides it behind the filesystem-and-process-oriented high-level API.

**One caveat:** in-process effect *interception* is only half there. `step()` returns effects, but in live mode non-handler effects must still go through `EgressRuntime` or the runner errors out. So in-process egress is a real host change, not just SDK surfacing.

---

## Work items

### ⓪ Document HostedRunner as the embedded integration point

**Effort:** ~1 day
**What:** Write docs framing `HostedRunner` as the low-level canonical host surface for embedded use cases. Someone determined enough can use Ergo embedded today by wiring up the expansion pipeline manually.
**Design note:** Frame as the low-level canonical host surface, not a second execution model. This aligns with existing doc comments in `ergo-host`.

### ① In-memory loader

**Effort:** 3–4 days
**What:** The loader crate (`ergo-loader`) is filesystem-bound. `parse_graph_file`, `discover_cluster_tree`, and `load_project` all want `PathBuf`. Need to accept strings/bytes so embedded users don't have to materialize to temp directories.
**Design decisions from Codex exchange:**

- Add `parse_graph_str` / `parse_graph_bytes` siblings at the parse level. The internal path already goes through `decode_graph_yaml` which is string-backed — so the seam exists, it's just not public.
- At the discovery level, add `discover_cluster_tree_with_resolver(...)` over opaque source IDs rather than paths. The filesystem coupling in discovery is deep (candidate resolution, duplicate checks, cycle tracking, recursive nested loads are all path-anchored).
- Do *not* genericize `load_project(start: &Path)`. That function is genuinely about on-disk project-root discovery. In-memory project manifests are cleaner as separate constructors.
- Keep the resolver trait sync-only. For DB-backed storage, the pattern is: fetch manifest/graph/cluster texts asynchronously outside the loader, materialize an in-memory source map, then call the sync loader. Future-proof by making the resolver keyed by transport-agnostic source IDs, not by making the trait async.

### ② In-memory ingress + in-process egress

**Effort:** 2–3 days
**What:** New `DriverConfig::Embedded` variant for feeding events without a subprocess. New in-process egress channel for handling effects without spawning an external program.
**Design decisions from Codex exchange:**

- In-memory ingress is straightforward — `HostedRunner::step()` already accepts `HostedEvent` directly. The SDK-level work is a new `DriverConfig` variant that feeds events to the runner from a bounded in-memory event source. The specific API shape (Vec, iterator, channel) should be chosen at implementation time — start with the narrowest first version.
- In-process egress is a real host change. `EgressChannelConfig` is currently pure data — serializable, fingerprinted for provenance. Putting a closure directly into that enum would break config/runtime separation.
- **Solution:** Keep routes/config data-only. Add a runtime-side embedded channel registry (or explicit user-supplied egress provenance) that handles dispatch for in-process effects. This keeps `EgressConfig` declarative and moves "how to actually dispatch" to runtime.

### ③ Reusable engine handle

**Effort:** 1–2 days
**What:** `Ergo` is consumed on every `.run()` / `.replay()` call. Need `&self` methods (or an `Arc`-friendly handle) so a server can build once and run many times.
**Design decisions from Codex exchange:**

- `RuntimeHandle` already wants `Arc<CorePrimitiveCatalog>` and `Arc<CoreRegistries>` internally. The refactor is to store these as `Arc` inside `Ergo` from the start.
- `Send + Sync` status: `SourcePrimitive`, `ComputePrimitive`, and `ActionPrimitive` do **not** have `Send + Sync` bounds. Only `TriggerPrimitive` does. `CoreRegistries` stores `Box<dyn ...>` for these traits. Both host and adapter code have explicit `arc_with_non_send_sync` suppressions.
- This means: reusable `&self` handle on the same thread is feasible and local. Sharing across threads requires tightening trait bounds — that's a bigger conversation, not part of this work.

### ④ Async wrapper

**Effort:** Depends on scope
**What:** The developer's backend is tokio-based. Ergo is fully synchronous.
**Design decisions from Codex exchange:**

- **Feasible now:** `spawn_blocking(move || { build Ergo inside; run; return outcome })` works if the closure captures only `Send` inputs. The entire Ergo lifecycle happens on the blocking thread. The caller gets back a `JoinHandle<Result<RunOutcome, ...>>` which is `Send`.
- **Not feasible now:** A transparent async mirror of the current SDK surface. You can't build an `Ergo` on the tokio side with custom primitives and then send it into the blocking thread, because the primitives aren't `Send`.
- **Scoping: fresh-build async wrapper only.** Reusable async handles and async wrappers over arbitrary custom primitive registrations are out of scope unless primitive trait bounds are tightened.

---

## Work order

```
⓪  Doc HostedRunner as embedded path       (~1 day)
①  In-memory loader                        (~3-4 days)
②  In-memory ingress + in-process egress   (~2-3 days)
③  Reusable engine handle                  (~1-2 days)
④  Async wrapper (scoped)                  (~1 day)
                                    Total: ~1 week
```

The dependency chain: ⓪ is standalone. ① unblocks practical use of ②. ③ is independent but most useful after ① and ②. ④ depends on understanding from ③.

---

## What this does not address

- **INGEST-TIME-1** (cross-ingestion normalization parity) remains open. The supervisor's deterministic clock advances from `event.at`, which means fixture ingress (time zero) and process ingress (real timestamps) can produce different scheduling decisions on the same logical data. This is a deferred design question, not a blocker for this work. A new in-memory ingress path would let the caller set `at` to whatever they want, same as process ingress.
- **Kernel changes.** None required. The four-primitive ontology, wiring matrix, execution model, and adapter contract are untouched.
- **Adding `Send + Sync` bounds to primitive traits.** This would enable full async support and cross-thread sharing but is a separate decision with broader implications.
