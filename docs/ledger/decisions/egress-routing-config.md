---
Authority: PROJECT
Date: 2026-03-16
Decision-Owner: Sebastian (Architect)
Participants: Claude (Structural Auditor), Codex (Ontology Guardian)
Status: DECIDED
Scope: v1
Parent-Decision: v1-external-effect-intent-model.md
Resolves: GW-EFX-3D
---

# Decision: Effect-Kind-to-Egress Routing Configuration

## Context

The v1 external effect intent model establishes that external intents
are dispatched to egress channels. This decision defines how the user
declares the mapping from intent kinds to egress channels, and what
the canonical internal representation looks like.

---

## The Fork

### Adapter manifest (rejected by doctrine)

Routing is prod-layer execution policy, not adapter vocabulary. The
adapter declares what effect kinds exist (`accepts.effects`). Routing
declares where they go at runtime. These are different concerns.

### Option (i) — Host run request only

Route table is a field on the host run request struct. SDK users pass
it programmatically. CLI users have no file-based surface.

**Rejected as sole surface.** Correct as canonical internal model,
but insufficient alone — CLI users need a file-based path.

### Option (ii) — Standalone config file only

`--egress-config path/to/egress.toml`. Parsed into internal model.
Works without `ergo-init`.

**Viable for v0 but insufficient alone.** SDK users shouldn't need
a file to pass a route table programmatically.

### Option (iii) — `ergo.toml` only

Project-level config. Ergonomic. But `feat/ergo-init` doesn't exist
yet.

**Rejected for v0.** Dependency on unbuilt infrastructure.

### Option (iv) — Hybrid (chosen)

Host run request as canonical internal model. File-based surfaces
(standalone TOML now, `ergo.toml` later) parse into it. All paths
normalize to one canonical shape.

---

## Ruling

### Canonical internal model

```rust
pub struct EgressConfig {
    pub default_ack_timeout: Duration,
    pub channels: BTreeMap<String, EgressChannelConfig>,  // channel_id -> config
    pub routes: BTreeMap<String, EgressRoute>,             // intent_kind -> route
}

pub struct EgressRoute {
    pub channel: String,                     // must reference a key in channels
    pub ack_timeout: Option<Duration>,       // overrides default_ack_timeout
}

pub enum EgressChannelConfig {
    Process { command: Vec<String> },        // mirrors ingress DriverConfig::Process
}
```

Design choices:

- **`BTreeMap` not `HashMap`.** Deterministic iteration order. Required
  for future provenance hashing (3b). Matches existing patterns
  (handlers use `BTreeMap`).
- **Channels and routes are separate maps.** One channel can serve
  multiple intent kinds. One intent kind maps to exactly one route
  (enforced by map key uniqueness).
- **`Process` variant uses `Vec<String>` for command.** Mirrors the
  ingress `DriverConfig::Process` shape. Avoids shell-parse ambiguity.
  First element is executable, rest are arguments.
- **`ack_timeout` per-route with default fallback.** Different intent
  kinds may have different latency expectations (a broker API vs a
  notification service).

### Startup validation rules

1. **Route channel exists.** Every `EgressRoute.channel` must reference
   a key in `EgressConfig.channels`. Error if not.
2. **Route kind is adapter-accepted.** Every routed intent kind must
   appear in the adapter's `accepts.effects`. Error if not.
3. **Coverage completeness.** Every graph-emittable, adapter-accepted
   intent kind that is NOT handled by a local `EffectHandler` must
   have a route. Enforced by feeding routed kinds into
   `ensure_handler_coverage(..., egress_claimed_kinds)` (Phase 1 work).
4. **Non-emittable routed kind is a warning, not an error.** A route
   for an intent kind the current graph doesn't emit is permitted —
   shared configs across graphs should work. Warn, don't reject.
5. **One owner per kind.** If a kind has both a local handler AND an
   egress route, `ensure_handler_coverage` returns
   `ConflictingCoverage` (Phase 1 work).
6. **Channel capability attestation.** At startup, each channel's
   ready handshake must declare protocol `ergo-egress.v1` and
   `handled_kinds`. For every routed kind assigned to that channel,
   `handled_kinds` must include the kind. Missing kinds or duplicate
   declarations are startup protocol errors. Extra declared kinds are
   allowed.

### File format for v0

Standalone TOML file, passed via `--egress-config <path>`. Structured
so it can be embedded verbatim under `[egress]` in a future
`ergo.toml` with no schema rewrite.

```toml
default_ack_timeout = "5s"

[channels.broker]
type = "process"
command = ["python3", "broker.py"]

[routes.place_order]
channel = "broker"
ack_timeout = "10s"

[routes.cancel_order]
channel = "broker"
```

### Host run request integration

`RunGraphFromPathsRequest` (or equivalent) gains an optional
`egress_config: Option<EgressConfig>` field. CLI parses the TOML file
and populates this field. SDK users construct it directly.

If `egress_config` is `None` and the graph emits intent kinds, the
coverage check fails at startup (existing behavior from Phase 1).

### Provenance (see 3b)

This decision defines the canonical normalization that the later
egress-provenance work now relies on. The `BTreeMap` ordering
guarantees deterministic serialization, so the provenance hash can be
computed from the normalized config without re-shaping the model.

The provenance field itself is specified by
`decisions/egress-provenance.md`, not by this decision.

---

## What This Does NOT Decide

- **`ergo.toml` integration.** That's `feat/ergo-init`.
- **Capture provenance for egress config.** That's 3b.
- **Multi-channel dispatch for a single intent kind.** Not supported.
  One intent kind → one route → one channel.
- **Dynamic route changes during a run.** Route table is immutable
  for the duration of a run.

---

## Impacted Files

- Host egress types: `EgressConfig`, `EgressRoute`, `EgressChannelConfig`
- Host/SDK request surfaces carrying `egress_config`
- CLI / project resolution surfaces that parse TOML into the canonical
  host model
- Host validation/startup path — validation against adapter + graph +
  handler registry
- Hosted-runner validation / `ensure_handler_coverage` call sites —
  routed kinds are treated as egress-claimed kinds
