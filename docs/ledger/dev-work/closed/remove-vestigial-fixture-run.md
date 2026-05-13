---
Authority: PROJECT
Date: 2026-05-13
Author: Sebastian (Architect) + Codex (Build Orchestrator)
Status: CLOSED
Branch: chore/remove-vestigial-fixture-run
Tier: 4 (Production hardening — surface cleanup)
Depends-On: >-
  docs/ledger/decisions/remove-vestigial-fixture-run.md;
  docs/plans/remove-vestigial-fixture-run.md
Unblocks: PUB-1 validation case in docs/plans/crates-io-publish.md
---

# Dev-Work: Remove `ergo fixture run` Subcommand

| ID | Title | Acceptance | Owner | Status |
|----|-------|------------|-------|--------|
| RFR-1 | Drop CLI dispatch + handler | `dispatch.rs` `"fixture"` arm has no `"run"` branch; `handlers.rs::run_fixture` and `FixtureRunSummary` removed; CLI builds. | Codex | CLOSED |
| RFR-2 | Update CLI usage / help text | `usage()`, `fixture_usage()`, and `help_topic` no longer mention `fixture run`; tests updated to match. | Codex | CLOSED |
| RFR-3 | Update redirect errors | `removed_run_fixture` fix string points at `ergo run <graph.yaml> -f <events.jsonl>`; new `removed_fixture_run` helper added; dispatch wires it under `"fixture"` arm. | Codex | CLOSED |
| RFR-4 | Remove host demo-fixture module | `crates/prod/core/host/src/demo_fixture_usecase.rs` deleted; `host/src/lib.rs` re-exports removed. | Codex | CLOSED |
| RFR-5 | Drop `features = ["demo"]` enablement | `ergo-host/Cargo.toml` line 12 and `ergo-cli/Cargo.toml` line 23 no longer carry `features = ["demo"]` on the production `ergo-supervisor` dependency. | Codex | CLOSED |
| RFR-6 | Doc sync | `docs/authoring/yaml-format.md` and `docs/authoring/project-convention.md` updated per §4 of the plan; `docs/ledger/dev-work/open/in-memory-loader-phase-2.md` IMT2-10 note updated; `docs/INDEX.md` cross-links added. | Sebastian | CLOSED |
| RFR-7 | Verification gates | All gates in §7 of the plan pass: workspace `cargo check`, `cargo test --workspace`, `cargo build --release -p ergo-cli`, dependency-tree audit, `rg` audits. | Codex | CLOSED |
| RFR-8 | Cross-link from PUB-1 plan | `docs/plans/crates-io-publish.md` validation-case section gains the cross-link defined in §8 of the plan; the PUB-1 row references the resolution. | Sebastian | CLOSED |
