---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-16
Owner: Sebastian (Architect)
Scope: Project-level guidance for host egress channels
Change Rule: Tracks implementation
---

# Egress Channel Guide

This guide explains how to author the egress-channel side of the
canonical host effect-dispatch path.

Current code and CLI still use implementation terms such as
`EgressConfig`, `EgressRoute`, and `--egress-config`. This guide uses
the doctrinal term *egress channel* except when naming those concrete
APIs.

An adapter package and live execution surface now separate into three
concerns:

- `adapter.yaml` — the semantic contract
- ingress channel implementation — the thing that produces events for
  host ingress
- egress channel implementation — the thing that receives external
  effect intent from host

These stay separate on purpose. The adapter declares accepted effect
vocabulary. Routing and real external I/O are prod concerns.

For where egress channels and `egress/*.toml` configs live in an
SDK-first Ergo project, see
[Project Convention](project-convention.md).

## What An Egress Channel Is Allowed To Do

An egress channel may:

- receive external intent records from host
- durably accept them
- return `intent_ack` records to host
- forward the intents to real external systems under its own
  implementation responsibility

An egress channel must never:

- execute graph logic
- mutate host `ContextStore` directly
- decide routing policy that belongs to `EgressConfig`
- redefine replay semantics
- claim completion truth through the durable-accept ack

Completion truth belongs to future ingress observations, not the ack.

## Relationship To `adapter.yaml`

`adapter.yaml` declares which external effect kinds are accepted at the
contract boundary:

```yaml
accepts:
  effects:
    - name: place_order
      payload_schema:
        type: object
        additionalProperties: false
        properties:
          symbol: { type: string }
          qty: { type: number }
        required: [symbol, qty]
```

`adapter.yaml` does **not** say where `place_order` goes. Routing is a
host/egress concern, not an adapter concern.

## Relationship To `EgressConfig`

The current v1 host routing surface is `EgressConfig`, typically loaded
from `--egress-config <egress.toml>`.

The route table decides:

- which intent kind goes to which egress channel
- the ack timeout policy for that route

One intent kind maps to exactly one route in v1.

## Startup Handshake

The process egress protocol uses a readiness handshake before live run
events begin.

Host expects:

```json
{
  "type": "ready",
  "protocol": "ergo-egress.v1",
  "handled_kinds": ["place_order", "cancel_order"]
}
```

Rules:

- `ready` must be the first inbound frame from the egress process
- `protocol` must match `ergo-egress.v1`
- `handled_kinds` must contain every routed kind assigned to that
  channel
- duplicate `handled_kinds` entries are protocol errors

`handled_kinds` is runtime capability attestation. It is **not**
configured provenance.

## Outbound Intent Message

Host sends one JSON Lines frame per intent:

```json
{
  "type": "intent",
  "intent_id": "eid1:sha256:...",
  "kind": "place_order",
  "fields": {
    "symbol": "AAPL",
    "qty": 10
  }
}
```

Notes:

- `intent_id` is deterministic and replay-safe
- `fields` is the JSON projection of manifest-declared typed intent
  fields
- host sends only external intent kinds on the egress path; host-
  internal `set_context` does not go through egress

## Durable-Accept Ack

The egress channel must respond with durable-accept, not mere receipt:

```json
{
  "type": "intent_ack",
  "intent_id": "eid1:sha256:...",
  "status": "accepted",
  "acceptance": "durable",
  "egress_ref": "broker-123"
}
```

Required semantics:

- `intent_id` must match the dispatched intent
- `status` must be `"accepted"`
- `acceptance` must be `"durable"`

`egress_ref` is optional and opaque to host.

Durable-accept means the egress channel is claiming responsibility for
the intent such that a process crash after ack does not silently lose
it.

## Failure And Ordering Semantics

Host runs the current v1 model as:

1. graph executes
2. host-internal writes apply
3. external intents dispatch
4. host waits for durable-accept acks
5. next step begins only after the current step's acks settle

On dispatch failure:

- host stops on first failure
- prior durable acks are preserved
- host quiesces egress channels for consistency
- the run becomes interrupted rather than silently continuing

## Replay

Replay never launches egress channels.

During replay:

- host re-derives the same external intent records
- host verifies captured effect integrity
- host does **not** resend intents to real systems

Egress provenance is stored for audit, but it is not currently part of
strict replay validation.

## Companion Docs

- [Current Architecture](../system/current-architecture.md)
- [Adapter Manifest](../primitives/adapter.md)
- [Ingress Channel Guide](ingress-channel-guide.md)
- [Effect-Kind-to-Egress Routing Configuration](../ledger/decisions/egress-routing-config.md)
- [Egress Acknowledgment and Result Semantics](../ledger/decisions/egress-ack-model.md)
