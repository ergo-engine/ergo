---
Authority: PROJECT
Date: 2026-03-15
Decision-Owner: Sebastian (Architect)
Participants: Claude (Structural Auditor), Codex (Ontology Guardian)
Status: DECIDED
Scope: v1
Depends-On: effect-dispatch-and-channel-roles.md
---

# Decision: v1 External Effect Intent Model

## Context

The effect-dispatch-and-channel-roles decision established that Actions
emit effect intent, adapters declare the accepted effect contract, host
dispatches post-episode, and channels realize boundary I/O. It did not
define the concrete mechanism by which the graph produces external effect
intents like `place_order`.

At decision time, the system emitted exactly one effect kind:
`set_context`. The runtime hardcoded this in `execute.rs`, and the
action manifest schema supported only `effects.writes` (context key
writes). There was no way for a user to author an Action that emitted a
first-class external intent.

This decision defines the v1 model for external effect intents.

---

## The Fork

Two models were evaluated.

### Option B — External effects as context writes (rejected)

Under this model, there would be no new effect kinds. An Action wanting
to place an order would write `order_intent = "buy EURUSD 100"` to
context via `set_context`. An egress channel process would declare which
context keys it watches. The host would do the ContextStore write as
usual and also forward matching writes to the egress process.

**Why this was rejected (Codex, verified by Claude):**

1. It collapses semantic intent into storage mutation. The decision
   record says Actions emit effect intent and adapters declare accepted
   effect kinds with host dispatch by realization class — not by watched
   context keys.

2. It creates hidden routing outside declared effect vocabulary. The
   adapter manifest frames `accepts.effects` as the effect vocabulary.
   Option B smuggles routing into key-name conventions.

3. Replay-class split becomes ambiguous. Doctrine requires a host-
   internal vs truly-external distinction. If everything is
   `set_context`, classification depends on external config rather than
   the effect itself.

4. It turns ContextStore into a command bus. ContextStore is state
   substrate, not command transport. Using it as a channel for external
   work is coupling by accident.

5. It risks feedback pollution. Context values are merged into later
   incoming payloads. Command-like keys bleed into graph inputs unless
   explicitly excluded — a footgun users would hit immediately.

### Option A — External effects as first-class intent (accepted)

Under this model, `place_order` is a distinct effect kind emitted by the
runtime from action manifest declarations. The host routes it to an
egress channel. The graph's decision is a first-class artifact, not a
context side-effect.

---

## Ruling

### Two-correlated-projections model

One action attempt produces one decision with two projections sharing a
common `intent_id`:

1. **Internal mirror write (optional).** A ContextStore write derived
   from the intent fields, so subsequent episodes can see the decision
   was made. This uses the existing `set_context` handler path.

2. **External intent dispatch.** A first-class intent record forwarded
   to the egress channel for realization.

Both projections originate from the same action input snapshot at the
same step. The external intent payload is primary; mirror writes are a
secondary projection declared from intent fields in the same manifest
block.

**Rejected alternatives:**

- One overloaded `ActionEffect` that host splits into two roles. Mixed
  replay class inside one record is semantically dirty.
- Two independent graph nodes (one for recording, one for dispatch).
  Splits a single decision into two divergable artifacts.

### Action manifest shape (v1)

The action manifest gains an `intents` section alongside the existing
`writes` section:

```yaml
effects:
  writes:
    - name: last_decision
      type: String
      from_input: decision_label

  intents:
    - name: place_order
      fields:
        - name: symbol
          type: String
          from_input: symbol
        - name: side
          type: String
          from_input: side
        - name: qty
          type: Number
          from_input: qty
      mirror_writes:
        - name: last_order_symbol
          type: String
          from_field: symbol
```

Rules:

- `fields[].from_input` references a declared action input. Must type-
  match.
- `fields[].from_param` references a declared action parameter. Exactly
  one of `from_input` or `from_param` per field.
- `mirror_writes` is optional. If omitted or empty, the intent has no
  internal projection.
- `mirror_writes[].from_field` references a declared intent field in the
  same intent block. Must be validated at registration time.
- Existing `effects.writes` behavior is unchanged for `set_context`.

### Runtime emit path

Intent emission is manifest-derived, not implementation-emitted:

- Action implementations continue to return outputs only.
- The runtime generically constructs intent records from the manifest
  `intents` declarations and the action's input snapshot.
- The runtime emits a canonical effect stream:
  - internal `set_context` effect (`writes` only, `intents=[]`)
  - external effects keyed by real intent kind (`writes=[]`,
    `intents` non-empty, each `intent.kind == effect.kind`)
- The hardcoded assumption that effects are only `set_context` is
  removed.
- Metadata-less execution paths (`execute` / `run`) reject
  intent-emitting graphs. Intent emission requires metadata-aware
  execution (`execute_with_metadata`) to derive deterministic
  `intent_id` values from real `graph_id` / `event_id`.

### Adapter manifest

`accepts.effects` continues to declare effect kinds and payload schemas.
No internal-vs-external classification in the adapter manifest. The
adapter says "I accept `place_order` with this payload schema." Whether
that intent routes to an egress channel or a host handler is a prod-
layer concern, not an adapter-contract concern.

### Host dispatch ordering

Within a single step, for each action that fires:

1. Apply `set_context` writes (existing path, via `SetContextHandler`).
2. Apply `mirror_writes` from any intents (same `SetContextHandler`
   path).
3. Dispatch external intent records to egress channel(s).

If mirror-write application fails, external dispatch does not proceed.
If external dispatch fails after mirror writes succeed, host behavior
must follow the artifact policy chosen in the separate artifact-policy
decision (see unresolved item 8 below). This introduces a new egress
interruption/failure class.

**Doctrinal guardrail:** Mirror state is decision state, not execution
confirmation. Execution truth returns via subsequent ingress events,
optionally keyed by `intent_id`.

### Replay behavior

- Internal mirror writes: re-apply during replay (subsequent episodes
  need the state).
- External intent records: verify intent matches capture, do NOT
  dispatch to egress. The real world must not be acted upon twice.
- Shared `intent_id` enables correlation between internal and external
  projections during replay verification.

### Startup coverage guarantee

The system will not start a run if a graph can emit an intent kind for
which no egress channel has declared coverage. This is enforced at run
startup via route-table ownership checks plus ready-handshake capability
attestation (`handled_kinds`). Therefore: if a decision was made during a
run, the ability to act on that intent kind was true at run start under
that mechanism.

This is a startup-time guarantee, not a step-time delivery guarantee.

### User-authoring surface

To get `place_order` working, a user touches four things:

1. **graph.yaml** — Action node wired to trigger and data inputs.
2. **adapter.yaml** — `accepts.effects` entry for `place_order` with
   payload schema.
3. **Egress route config** — Project-level mapping of `place_order` to a
   named egress channel. Lives in `ergo.toml` (preferred) or passed via
   `--egress-config`.
4. **Egress channel process** — User-authored script/binary that
   receives intent records and performs external work.

If using a custom intent kind (not a built-in action), the user also
authors an action manifest with `effects.intents` declared. No custom
runtime code; emission is manifest-derived.

---

## Follow-On Resolution Status

Resolved after this decision:

1. **ActionEffect payload shape** — `intent-payload-shape.md`
2. **`intent_id` semantics** — `intent-id-semantics.md`
3. **Routing configuration** — `egress-routing-config.md`
4. **Ack model** — `egress-ack-model.md`
5. **Timing/lifecycle** — `egress-timing-lifecycle.md`
6. **Crash consistency** — `crash-consistency.md`
7. **Artifact policy on dispatch failure** — resolved inline as Option C
   in GW-EFX-3B implementation.

Still open in the work plan:

1. **Egress provenance extension** (GW-EFX-3C)
2. **Egress failure taxonomy / partial-apply semantics** (GW-EFX-3J)

---

## Validation Requirement

`mirror_writes[].from_field` must reference a declared intent field
within the same `intents` entry. This must be enforced at action
manifest registration time. A missing or mismatched `from_field`
reference is a registration error, not a runtime error.

---

## Rationale

The graph is a decision engine. Its output is decisions. Those decisions
have two natural consequences: they must be remembered (for future
episodes) and they must be acted upon (in the real world). These are two
projections of one event, not two different events.

Making the external intent first-class — rather than encoding it as a
context write — preserves the declared effect vocabulary, gives replay a
clean classification boundary, keeps ContextStore as state rather than a
command bus, and prevents feedback pollution from command-like keys
bleeding into graph inputs.

The coupled projection model (shared `intent_id`, mirror writes derived
from intent fields) ensures that remembering a decision and acting on it
are correlated without being conflated. Optional `mirror_writes` avoids
forcing junk state for fire-and-forget intents.

The manifest-derived emit path keeps action implementations pure. All
computation stays in the graph. The action declares what it intends; the
runtime constructs the records; the host dispatches them.

---

## Impacted Ledger Files

- `docs/ledger/gap-work/closed/effect-realization-boundary.md` (GW-EFX-3
  and sub-gaps)
- `docs/ledger/decisions/effect-dispatch-and-channel-roles.md` (this
  decision extends but does not modify the prior ruling)
