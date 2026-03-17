---
Authority: PROJECT
Date: 2026-03-15
Decision-Owner: Sebastian (Architect)
Recorder: Codex (Docs)
Status: DECIDED
Decides: GW-EFX-1, GW-EFX-1A, GW-EFX-1B, GW-EFX-1C
---

# Decision: Effect Dispatch and Channel Roles

## Ruling

Ergo doctrine distinguishes four separate roles in the effect lifecycle:

1. **Actions emit effect intent.**
2. **Adapters declare the accepted effect contract.**
3. **Host owns post-episode effect dispatch.**
4. **Channels realize boundary I/O.**

More specifically:

- Actions do not directly perform real external work inside graph semantics.
- Action execution produces effect intent/instruction records.
- Adapters remain the declarative external interface contract. They
  define context keys, event kinds, and accepted effect kinds.
- Host is the canonical post-episode control locus. After an episode
  terminates, host drains buffered effects and dispatches them
  according to their realization class.
- **Ingress channels** bring external events into host execution.
- **Egress channels** perform true external I/O for externally realized effects.
- Host-internal effects may still be realized directly by host handlers.

This replaces any doctrine that implicitly treats the adapter itself as
the concrete runtime actor performing all real external I/O.

## Replay Ruling

Replay doctrine distinguishes two replay classes:

- **Host-internal effects** may be replay-realized when required to
  reconstruct deterministic cross-episode state.
- **Truly external effects** must be re-derived and verified during
  replay, but must not be re-executed against live external systems.

This replay split is doctrinal, not merely an implementation detail.

## Terminology Ruling

Doctrine now prefers:

- **ingress channel**
- **egress channel**

Current implementation terms such as `DriverConfig` may remain in code
and canonical implementation docs as legacy naming until renamed, but
they are not the preferred doctrinal terms for the generalized boundary
model.

## Transport Stance

Doctrine defines roles, not a required transport.

- `stdout` for ingress and `stdin` for egress is a valid
  **process-channel example**.
- No specific transport is required by this decision.
- Future channel realizations may use different transport mechanisms so
  long as they preserve the doctrinal role split above.

## Explicit Non-Closure

This decision does not claim the following are already solved:

- **Multi-ingress host support.** The current canonical host API accepts
  one ingress configuration per run. Workspaces that need multiple live
  sources currently require an upstream multiplexer channel or future
  host support for multiple ingress channel configs.
- **Egress-channel lifecycle and configuration.** This decision defines
  the doctrinal role of egress channels, but it does not define their
  launch model, routing model, handshake, backpressure contract,
  shutdown behavior, failure semantics, or replay/capture integration.

## Rationale

This ruling resolves a semantic blur across the current stack:

- higher-level doctrine describes adapters as the external effect
  boundary
- execution doctrine describes host as the concrete post-episode effect
  application locus
- current product code uses a `DriverConfig`/process model for ingress
- `set_context` is a host-internal effect path, not a general proof
  that all effects should be realized by host-local state mutation

The chosen split preserves kernel/prod separation:

- kernel-facing surfaces remain declarative and deterministic
- host remains the lifecycle/control boundary
- real external connections remain in prod-owned channel implementations

This also gives replay a stable doctrine:

- internal reconstruction may re-apply
- live external work may not

## Implications

- User-authored Actions should be framed as pure decision/intent
  producers, not as direct external-I/O plugins.
- Adapter manifests continue to declare effect vocabulary and
  acceptance, but not the concrete transport implementation.
- Host/canonical docs should describe host as the dispatcher, not as
  the sole realization actor for every effect kind.
- External-effect designs should target an egress-channel boundary
  rather than redefining adapters as prod control code.
- `set_context` remains a host-internal realization path.

## Impacted Ledger Files

- [effect-realization-boundary.md](/Users/sebastian/Projects/ergo/docs/ledger/gap-work/closed/effect-realization-boundary.md)

## Follow-Up Actions

1. Update the open gap record to reference this decision and move any
   now-resolved rows from `DECISION_PENDING` to `DECIDED` or
   closure-ready status.
2. Clarify doctrine wording across `/docs` so that:
   - Actions emit effect intent
   - adapters declare the effect contract
   - host dispatches post-episode
   - ingress/egress channels own boundary I/O realization
3. Add explicit documentation for the replay class split between
   host-internal and truly external effects.
4. Audit canonical implementation docs for legacy `driver` terminology
   and annotate it as implementation-era naming where needed.
5. Track multi-ingress and egress-channel lifecycle as explicit
   follow-on gaps after doctrine propagation is complete.
