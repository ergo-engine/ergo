---
Authority: PROJECT
Date: 2026-03-16
Decision-Owner: Sebastian (Architect)
Recorder: Codex (Docs)
Status: DECIDED
Scope: v1
Parent-Decision: effect-dispatch-and-channel-roles.md
Resolves: GW-EFX-2
---

# Decision: Multi-Ingress Host Direction

## Context

Current canonical host APIs accept one ingress configuration per run
(`DriverConfig` in current code, which is legacy naming relative to
the doctrinal term *ingress channel*).

`feat/ergo-init` needs a stable project/profile model before workspace
layout and command UX can be refined. The remaining open question was
whether host should evolve toward many ingress configs per run, or
whether many external feeds should be composed upstream into one
ingress channel that emits a single `HostedEvent` stream.

## The Fork

### Option A - Canonical host stays single-ingress; multi-source projects multiplex upstream - CHOSEN

One run/request/profile resolves to exactly one ingress source:

- fixture, or
- one process ingress command

Projects that consume many live feeds do so inside that one process
ingress channel. That ingress channel may subscribe to many upstream
systems, merge them, and emit one ordered `HostedEvent` stream to host.

### Option B - Canonical host gains multi-ingress request/config support

Host would accept many ingress configs, launch and supervise multiple
child processes, merge their event streams, and expose that surface
through CLI and `ergo.toml`.

**Rejected for v1.** This would turn host into a many-source event
orchestrator and force new product rules for readiness, shutdown,
interruption mapping, fairness, and merged-stream ownership before
`ergo-init` can even stabilize its profile model.

## Ruling

Canonical host remains single-ingress in v1.

This means:

- host run requests continue to accept exactly one ingress selector
- `DriverConfig` (or its future doctrinal rename) still means one
  ingress choice, not a collection
- no v1 CLI or `ergo.toml` profile model promises host-owned
  multi-ingress request/config support
- fixture + process mixing remains unsupported in one run/profile
- projects that need multiple live feeds must combine them upstream
  into one ingress channel process

## Why This Direction Fits

### 1. Boundary ownership stays clean

Doctrine already says ingress channels realize boundary I/O. Fan-in from
many outside feeds into one host-facing stream is ingress-channel work,
not new host doctrine.

### 2. Host lifecycle semantics stay tractable

Host currently owns one ingress lifecycle: startup, protocol truth,
clean completion, interruption, and bounded shutdown. Multi-ingress
host support would require new policy for:

- whether all channels must say `hello` before the run starts
- whether one channel ending completes the run or only that source
- how failures from one source interact with others
- how fairness and backpressure work across multiple live producers

Those are real product-surface commitments, not incidental plumbing.

### 3. Capture and replay stay simple

Capture and replay already reason over one linear `HostedEvent`
sequence. Upstream multiplexing preserves that model unchanged. Host
does not need to gain source-topology semantics just to support many
feeds.

### 4. `ergo-init` gets a stable profile contract now

`ergo-init` can standardize exactly one ingress source per profile
without waiting for a second host-ingress architecture. A profile may
still point at a user-authored multiplexer command when the project has
many external feeds.

## Consequences for `ergo-init`

- One profile resolves to one graph, one adapter, exactly one ingress
  source, and optional egress config.
- `channels/ingress/` may contain many helper programs, but profile
  resolution still selects one command to launch.
- `ergo init` may scaffold and document the multiplexer pattern, but it
  must not promise host-owned multi-ingress discovery, launch, or
  merged-stream policy.

## What This Does NOT Decide

- how a multiplexer ingress channel is implemented internally
- whether Ergo later ships helper templates or libraries for
  multiplexer channels
- any host-level multi-ingress request/config surface for v1
- any future reopening of host multi-ingress; that would require a new
  gap and decision record

## Impacted Ledger Files

- [effect-realization-boundary.md](../gap-work/closed/effect-realization-boundary.md)
- [ergo-init.md](../dev-work/closed/ergo-init.md)
