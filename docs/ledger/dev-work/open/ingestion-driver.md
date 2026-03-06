---
Authority: PROJECT
Date: 2026-03-04
Author: Claude Opus 4.5 (Structural Auditor)
Status: OPEN
Branch: feat/ingestion-driver
Tier: 2 (Extension Plumbing)
Depends-On: feat/adapter-runtime
---

# Event Ingestion Driver

## Scope

Build the event loop that connects an adapter's event source to the host runner. Currently nobody calls `HostedRunner::step()` from a real data source — events come from fixtures or test code. This branch provides the driver that bridges live adapter event production to host execution.

Entirely prod-layer. No kernel changes. No frozen doc changes.

## Current State

`HostedRunner::step()` accepts a `HostedEvent` and does everything from there — event binding, supervisor scheduling, graph execution, effect application, capture. But the caller is always test code or a fixture loop. There is no production caller.

## What's Needed

A driver component that:

1. Receives events from an adapter's event source (as defined by `feat/adapter-runtime`)
2. Converts raw adapter output into `HostedEvent`
3. Calls `runner.step()` for each event
4. Manages lifecycle: startup, shutdown, graceful drain
5. Handles connection failures and reconnection (adapter-specific, delegated to adapter)

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| ID-1 | Design driver API | API reviewed. Defines how an adapter event source feeds the host runner. Must handle backpressure, shutdown, errors. | Codex + Claude | OPEN |
| ID-2 | Implement driver | Driver compiles. Accepts adapter + runner, runs event loop. | Codex | OPEN |
| ID-3 | Graceful shutdown | Driver stops accepting events, drains in-flight steps, produces final capture bundle. | Codex | OPEN |
| ID-4 | Error propagation | Adapter event source errors surface to caller. Host step errors surface to caller. Neither silently swallowed. | Codex | OPEN |
| ID-5 | Test: driver runs simulation adapter to completion | Fixture-backed simulation adapter feeds driver, driver feeds runner, capture bundle produced. | Codex | OPEN |
| ID-6 | Test: driver handles adapter disconnect | Adapter event source returns error mid-stream. Driver stops cleanly, partial capture bundle is valid. | Codex | OPEN |
| ID-7 | CLI integration | `ergo run <graph> --adapter <manifest>` uses the driver internally. | Codex | OPEN |

## Design Constraints

- The driver does not own adapter construction or manifest validation. Those happen before the driver starts.
- The driver does not own capture bundle finalization. `HostedRunner::into_capture_bundle()` is called after the driver completes.
- The driver is synchronous in v0 (one event at a time). Async/concurrent event handling is out of scope.
- No domain-specific language. "Event source" and "step" are the terms.
