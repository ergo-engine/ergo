---
Authority: PROJECT
Date: 2026-03-16
Author: Codex (Implementation)
Status: CLOSED
Branch: feat/sdk-rust
Tier: 3 (Developer Experience — product API)
Depends-On: >-
  feat/catalog-builder, feat/adapter-runtime, feat/egress-surface;
  docs/ledger/decisions/custom-implementation-loading.md
  (SDK registers custom primitives in-process through CatalogBuilder);
  docs/ledger/decisions/multi-ingress-host-direction.md
  (v1 remains one ingress source per profile)
---

# Rust SDK Surface

## Scope

Implement the first real SDK product surface for Ergo.

This branch turns `crates/prod/clients/sdk-rust` from a placeholder
crate into the canonical Rust entrypoint for:

- in-process custom primitive registration
- project discovery from `ergo.toml`
- explicit run configuration
- profile execution
- strict replay
- project validation

The SDK wraps the existing host + loader + runtime surfaces. It does
not define a second execution model and it does not replace host
semantics.

## Current State

Before this branch:

- `ergo-sdk-rust` was a placeholder
- project/profile resolution existed only as planned product shape
- production users still had to wire host-facing types directly
- cluster-aware project resolution was not available through an SDK API

After this branch:

- `Ergo::builder()` and `Ergo::from_project(...)` exist
- custom primitives register through `CatalogBuilder`
- `ergo.toml` profiles resolve through the SDK
- project `clusters/` is added implicitly to cluster search paths
- `run`, `run_profile`, `replay`, `replay_profile`, and
  `validate_project` delegate to canonical host paths

## Implemented Shape

1. `ergo-sdk-rust` exposes a public builder over `CatalogBuilder`.
2. User code can register custom `Source`, `Compute`, `Trigger`, and
   `Action` implementations in-process.
3. The SDK can discover a project root by finding `ergo.toml`.
4. `ergo.toml` profiles resolve:
   - `graph`
   - optional `adapter`
   - exactly one ingress source:
     - `fixture`, or
     - process ingress command
   - optional `egress` TOML path
   - optional capture output path
5. Loader-owned project resolution automatically adds
   `project_root/clusters` to loader search paths.
6. `run_profile()` delegates to the existing host run path.
7. `replay_profile()` delegates to strict host replay.
8. `validate_project()` resolves every profile and validates graph,
   cluster, adapter, and egress surfaces through the existing runtime
   rules plus shared host egress validation.

## Closure Ledger

| ID | Task | Closure Condition | Owner | Status |
|----|------|-------------------|-------|--------|
| SDK-1 | Expose SDK builder API | `ergo-sdk-rust` exposes `Ergo::builder()` and `Ergo::from_project(...)` as the canonical Rust entry surface. | Codex | CLOSED |
| SDK-2 | Support in-process primitive registration | Builder admits custom `Source`, `Compute`, `Trigger`, and `Action` implementations through the same `CatalogBuilder` path selected by `custom-implementation-loading.md`. | Codex | CLOSED |
| SDK-3 | Resolve project manifests | Loader-owned project discovery resolves `ergo.toml`, parses named profiles, and resolves project-relative paths plus implicit `clusters/` search paths for SDK consumption. | Codex | CLOSED |
| SDK-4 | Support explicit run configuration | SDK exposes explicit run configuration for non-project execution without bypassing host semantics. | Codex | CLOSED |
| SDK-5 | Implement profile execution path | `run_profile(...)` resolves one named profile into graph + adapter + ingress source + optional egress config and delegates through the canonical host run path. | Codex | CLOSED |
| SDK-6 | Implement replay surface | `replay(...)` and `replay_profile(...)` delegate to strict host replay without inventing a second replay model. | Codex | CLOSED |
| SDK-7 | Implement project validation surface | `validate_project()` resolves every named profile, validates graph/adapter composition, validates referenced egress config parsing when present, and returns typed SDK errors. | Codex | CLOSED |
| SDK-8 | Make clusters first-class in SDK project resolution | Project resolution automatically adds `project_root/clusters` to loader search paths, and tests prove cluster-backed profile execution succeeds. | Codex | CLOSED |
| SDK-9 | Document SDK-first product surface | `crates/prod/clients/sdk-rust/README.md` explains SDK ownership boundaries and current handle semantics, and `/docs` canonically reflects the SDK-first project model. | Codex | CLOSED |
| SDK-10 | Prove end-to-end branch stability | `cargo test -p ergo-sdk-rust` and `cargo test --workspace` pass after the SDK implementation lands. | Codex | CLOSED |

## Design Constraints

- The SDK delegates canonical run and replay to `ergo-host`.
- Project resolution must not invent a second execution model.
- The built `Ergo` handle is currently one-shot: `run`, `run_profile`,
  `replay`, `replay_profile`, and `validate_project` consume it.
- A reusable engine handle is a future ergonomics improvement, not a
  prerequisite for v1.
- Shared project resolution now lives in `ergo-loader`; future CLI
  convenience paths can consume the same surface, but SDK-first is the
  canonical product surface now.

## What This Branch Enables

After `feat/sdk-rust` merges:

1. Real production users can embed Ergo from a Rust crate without
   wiring host internals directly.
2. `feat/ergo-init` can scaffold a Rust project against a real SDK
   surface instead of a placeholder.
3. Project-mode CLI conveniences can become optional wrappers over the
   same resolved project model rather than the primary product surface.
