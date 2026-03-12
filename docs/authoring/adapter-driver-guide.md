---
Authority: CANONICAL
Version: v1
Last Updated: 2026-03-12
Owner: Codex (Implementation Assistant)
Scope: Project-level guidance for host ingress drivers
Change Rule: Tracks implementation
---

# Adapter Driver Guide

This guide explains how to author the driver side of an adapter package
for the canonical host ingress path.

An adapter package has two parts:

- `adapter.yaml` — the semantic contract
- driver implementation — the thing that produces events for host
  ingress

These two parts stay separate on purpose. `adapter.yaml` remains purely
semantic. Driver configuration and launch or wiring are host concerns.

## What A Driver Is Allowed To Do

A driver may produce `HostedEvent`.

A driver must never:

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
  compatibility, but new drivers should emit `Pump`.
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

If your driver emits:

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

After the driver hands off a `HostedEvent`, host owns the rest:

1. merge context (`incoming > store` precedence)
2. validate semantic payload and bind it through the adapter binder when
   an adapter is configured
3. execute the graph step
4. drain and apply effects through registered handlers
5. enrich the capture artifact
6. keep replay capture-driven and separate from live ingress

Drivers do not participate in any of those stages.

## Supported Ingress Shapes In This Branch

This branch supports two ingress shapes:

- `DriverConfig::Fixture` — built-in reference ingress for deterministic
  testing and local runs
- `DriverConfig::Process` — the public live driver model

No public Rust trait driver model ships in this branch.

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

## Process Driver Protocol (`DriverConfig::Process`)

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
- `stdin` is unused in v0
- protocol messages are UTF-8 JSON Lines, one message per line
- drivers should flush each line as it is written
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

For a clean completion, a process driver should:

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
driver backpressure is synchronous. If the host is busy stepping, the
child may block on `stdout`. That is the v0 backpressure mechanism.

Do not write extra protocol frames after `end`.
Do not assume that non-zero exit after `end` is still completion; host
surfaces that as interruption, not success.

## Complete Process Example

Minimal Python driver:

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

Run it through the CLI as a thin wrapper over host:

```text
ergo run graph.yaml --adapter adapter.yaml --driver-cmd python3 \
  --driver-arg driver.py
```

## What Not To Put In `adapter.yaml`

Do not add driver launch metadata to `adapter.yaml`.

Bad example:

```yaml
driver:
  type: process
  command: ["python3", "driver.py"]
```

Why this is forbidden:

- `adapter.yaml` is the semantic adapter contract
- host owns driver configuration and launch
- workspace-level discovery and ergonomics belong to `feat/ergo-init`

## Troubleshooting

- If host says `driver.protocol_invalid`, check `hello` ordering, JSON
  Lines shape, and `end` usage.
- If host says `driver.io_failed`, check process lifecycle, UTF-8
  output, and whether the driver is flushing `stdout`.
- If host rejects `semantic_kind` or payload, compare the emitted event
  against `adapter.yaml` `event_kinds` and the declared JSON schema.

## Related Docs

- [Adapter Manifest](../primitives/adapter.md)
- [YAML Graph Format](yaml-format.md)
- [Loader Contract](loader.md)
- [Kernel/Prod Separation](../system/kernel-prod-separation.md)
- [Adapter Ingress Surface
  Ledger](../ledger/dev-work/open/adapter-ingress-surface.md)
