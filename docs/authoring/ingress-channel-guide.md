---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-15
Owner: Codex (Implementation Assistant)
Scope: Project-level guidance for host ingress channels
Change Rule: Tracks implementation
---

# Ingress Channel Guide

This guide explains how to author the ingress-channel side of an
adapter package for the canonical host ingress path.

Current code and CLI still use legacy implementation terms such as
`DriverConfig`, `--driver-cmd`, and `--driver-arg`. This guide uses the
doctrinal term *ingress channel* except when naming those concrete
APIs.

An adapter package has two parts:

- `adapter.yaml` — the semantic contract
- ingress channel implementation — the thing that produces events for
  host ingress

These two parts stay separate on purpose. `adapter.yaml` remains purely
semantic. Ingress-channel configuration and launch or wiring are host
concerns.

This guide covers ingress only. For the outbound side of the same
system, see [Egress Channel Guide](egress-channel-guide.md).

## What An Ingress Channel Is Allowed To Do

An ingress channel may produce `HostedEvent`.

An ingress channel must never:

- construct canonical `ExternalEvent`
- bypass `HostedRunner::step()`
- bypass `build_external_event()`
- own capture or replay semantics
- define orchestration behavior that belongs to host

The canonical path is:

```text
HostedEvent -> build_external_event() -> execute_step()
```

## `HostedEvent` Wire Shape

The canonical host event type is:

```rust
pub struct HostedEvent {
    pub event_id: String,
    pub kind: ExternalEventKind,
    pub at: EventTime,
    pub semantic_kind: Option<String>,
    pub payload: Option<serde_json::Value>,
}
```

### Fields

- `event_id`
  Stable event identity within the run. It must be unique for
  canonical runs. Host rejects duplicates.
- `kind`
  Mechanical event kind. Valid values are `Pump`, `DataAvailable`, and
  `Command`. Legacy `Tick` is accepted as a serde alias for
  compatibility, but new ingress channels should emit `Pump`.
- `at`
  Opaque event time. This is the JSON form of host `EventTime`. For
  simple examples, `{"secs":0,"nanos":0}` is valid.
- `semantic_kind`
  Semantic event type name. It is required for adapter-bound runs and
  must match a declared `event_kinds[].name` in `adapter.yaml`. Omit it
  for non-adapter runs.
- `payload`
  Semantic payload object. It is optional, but when present it must be
  a JSON object and must validate against the matching manifest
  `payload_schema` after host context merge.

### How `semantic_kind` And `payload` Connect To `adapter.yaml`

`adapter.yaml` declares semantic event kinds under `event_kinds`:

```yaml
event_kinds:
  - name: price_bar
    payload_schema:
      type: object
      additionalProperties: false
      properties:
        close:
          type: number
      required: [close]
```

If your ingress channel emits:

```json
{
  "event_id": "evt-1",
  "kind": "Command",
  "at": { "secs": 0, "nanos": 0 },
  "semantic_kind": "price_bar",
  "payload": { "close": 101.25 }
}
```

then host will:

1. require `semantic_kind` because the run is adapter-bound
2. look up `price_bar` in the validated adapter manifest
3. merge writable adapter context from `ContextStore` into the incoming
   payload where allowed
4. validate the resulting semantic payload against the adapter schema
5. bind that validated semantic event into canonical `ExternalEvent`
6. execute the graph through the canonical host step path

If `semantic_kind` is missing for an adapter-bound run, or if the
payload does not satisfy the schema after host context merge, host
rejects the event before execution.

## After Handoff: What Host Does

After the ingress channel hands off a `HostedEvent`, host owns the
rest:

1. merge context (`incoming > store` precedence)
2. validate semantic payload and bind it through the adapter binder when
   an adapter is configured
3. execute the graph step
4. drain and dispatch effects through registered handlers
   (handler-owned kinds) and egress channels (egress-owned kinds)
5. enrich the capture artifact
6. keep replay capture-driven and separate from live ingress

Ingress channels do not participate in any of those stages.

## Supported Ingress Shapes In This Branch

The current canonical host surface accepts one ingress-channel config
per run.

This branch supports two ingress shapes in current code:

- `DriverConfig::Fixture` — built-in reference ingress for deterministic
  testing and local runs
- `DriverConfig::Process` — the public live ingress-channel model

If you need multiple live sources today, combine them upstream into one
ingress channel or wait for future host multi-ingress support.

No public Rust trait ingress-channel model ships in this branch.

## Fixture Reference Path

Fixture ingress is the simplest reference path. The fixture file is JSON
Lines, not `HostedEvent` JSON directly.

Example:

```json
{"kind":"episode_start","id":"E1"}
{"kind":"event","event":{"type":"Command","id":"evt-1"}}
```

Adapter-bound example:

```json
{"kind":"episode_start","id":"E1"}
{"kind":"event","event":{
  "type":"Command",
  "id":"evt-1",
  "semantic_kind":"price_bar",
  "payload":{"close":101.25}
}}
```

Notes:

- `id` inside the event record is optional for fixtures; host generates
  `fixture_evt_<n>` when omitted
- fixture events do not carry `at`; the fixture path uses host defaults
  for reference runs
- payload, when present, must still be a JSON object

## Process Ingress Channel Protocol (`DriverConfig::Process`)

The public live ingress model is:

```rust
DriverConfig::Process { command: Vec<String> }
```

Rules:

- `command` is argv-based, not a shell string
- host launches a direct child process
- v0 lifecycle management covers that direct child process only; full
  descendant process-tree containment is not part of this branch
- `stdout` is the protocol channel
- `stderr` is diagnostics only
- `stdin` is unused in v0 for ingress; egress-channel protocol is not
  defined by this guide
- protocol messages are UTF-8 JSON Lines, one message per line
- ingress channels should flush each line as it is written
- host applies internal startup and termination grace windows as
  operational waiting policy; those grace windows do not change what
  clean completion means

### Protocol Frames

The process protocol uses three message types.

#### `hello`

Must be first.

```json
{"type":"hello","protocol":"ergo-driver.v0"}
```

#### `event`

Carries one `HostedEvent`.

```json
{
  "type": "event",
  "event": {
    "event_id": "evt-1",
    "kind": "Command",
    "at": { "secs": 0, "nanos": 0 },
    "semantic_kind": "price_bar",
    "payload": { "close": 101.25 }
  }
}
```

#### `end`

Marks graceful end-of-stream.

```json
{"type":"end"}
```

### Completion And Interruption Expectations

Protocol truth and host waiting policy are separate.

For a clean completion, a process ingress channel should:

1. send valid `hello`
2. send one or more valid `event` frames
3. send `end`
4. stop writing to `stdout`
5. exit with status `0`

Host currently applies an internal `startup_grace` while waiting for the
first protocol observation / `hello`, and an internal
`termination_grace` after `end` or `stdout` EOF while it waits to
observe the terminal process state. Those grace windows are host
operational policy, not protocol law.

The host reads one event at a time through the canonical step path, so
ingress-channel backpressure is synchronous. If the host is busy stepping, the
child may block on `stdout`. That is the v0 backpressure mechanism.

Do not write extra protocol frames after `end`.
Do not assume that non-zero exit after `end` is still completion; host
surfaces that as interruption, not success.

## Complete Process Example

Minimal Python ingress channel:

```python
#!/usr/bin/env python3
import json
import sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

send({"type": "hello", "protocol": "ergo-driver.v0"})
send({
    "type": "event",
    "event": {
        "event_id": "evt-1",
        "kind": "Command",
        "at": {"secs": 0, "nanos": 0},
        "semantic_kind": "price_bar",
        "payload": {"close": 101.25},
    },
})
send({"type": "end"})
```

Run it through the CLI as a thin wrapper over host. The current CLI
flag names still use legacy `driver` terminology:

```text
ergo run graph.yaml --adapter adapter.yaml --driver-cmd python3 \
  --driver-arg driver.py
```

## What Not To Put In `adapter.yaml`

Do not add ingress- or egress-channel launch metadata to
`adapter.yaml`.

Bad example:

```yaml
driver:
  type: process
  command: ["python3", "driver.py"]
```

Why this is forbidden:

- `adapter.yaml` is the semantic adapter contract
- host owns ingress-channel configuration and launch
- workspace-level discovery and ergonomics belong to `feat/ergo-init`
- egress-channel design is separate follow-on work

## Troubleshooting

Current host error strings still use legacy `driver.*` wording.

- If host says `driver.protocol_invalid`, check `hello` ordering, JSON
  Lines shape, and `end` usage.
- If host says `driver.io_failed`, check process lifecycle, UTF-8
  output, and whether the ingress channel is flushing `stdout`.
- If host rejects `semantic_kind` or payload, compare the emitted event
  against `adapter.yaml` `event_kinds` and the declared JSON schema.

## Related Docs

- [Effect Dispatch and Channel Roles Decision](../ledger/decisions/effect-dispatch-and-channel-roles.md)
- [Adapter Manifest](../primitives/adapter.md)
- [YAML Graph Format](yaml-format.md)
- [Loader Contract](loader.md)
- [Kernel/Prod Separation](../system/kernel-prod-separation.md)
- [Adapter Ingress Surface
  Ledger](../ledger/dev-work/closed/adapter-ingress-surface.md)
