# Repository Guidelines

## Agent-Specific Protocols

**If you are Claude (Claude Code):** Read `.agents/.claude/CLAUDE.MD` and `.agents/.claude/CLAUDE_CODE_PROTOCOL.md` before proceeding. You report to Claude Prime (Doctrine Owner).

**If you are Codex:** Read `.agents/.codex/CODEX_PROTOCOL.md` before proceeding. You report to ChatGPT (Build Orchestrator).

These protocols define your authority boundaries, escalation rules, and the multi-agent review flow. Deviation is a contract violation.

---

## Project Structure & Module Organization

- `crates/kernel/runtime/`, `crates/kernel/adapter/`, `crates/kernel/supervisor/`: kernel crates.
- `crates/prod/core/host/`, `crates/prod/core/loader/`: product core crates.
- `crates/prod/clients/cli/`, `crates/prod/clients/sdk-rust/`, `crates/prod/clients/sdk-types/`: thin client crates.
- `crates/shared/test-support/`, `crates/shared/fixtures/`: shared support crates.
- `docs/`: canonical documentation tree (authoritative).
- `docs/ledger/dev-work/`: delivery ledgers (implementation branches).
- `docs/ledger/gap-work/`: doctrine/risk/gap ledgers.
- `docs/ledger/decisions/`: authority decision records.
- `target/`: generated artifacts.

## Build, Test, and Development Commands

Rust (run from repo root):

- `cargo build` — build workspace.
- `cargo test` — run all Rust tests.
- `cargo test -p ergo-runtime` — run a single crate.
- `cargo fmt` — format with rustfmt.

UI:

- Reference client is intentionally removed from the active workspace.

## Coding Style & Naming Conventions

- Rust 2021; follow rustfmt defaults and standard Rust casing (`snake_case` modules/functions, `PascalCase` types).
- Core layers must stay domain-neutral; exceptions require PR justification (see `docs/system/terminology.md`).

## Testing Guidelines

- Unit tests live alongside code with `#[test]`; supervisor integration tests are in `crates/kernel/supervisor/tests`.
- Golden Spike tests are canonical execution paths: `crates/kernel/runtime/src/runtime/tests.rs` and `crates/kernel/supervisor/tests/integration.rs`.

## Commit & Pull Request Guidelines

- Commit messages use Conventional Commits with optional scope, e.g. `feat(supervisor): ...`.
- PRs must map invariants + test evidence (`docs/invariants/INDEX.md` and phase files).
- Supervisor internal behavior changes require doctrine review against `docs/orchestration/supervisor.md`.
- Serialized term renames require compatibility aliases + tests (see `docs/system/terminology.md`).

## GitHub Mechanics & Templates

- PRs use `.github/PULL_REQUEST_TEMPLATE.md`.
- Issues use `.github/ISSUE_TEMPLATE/` with structured templates:
  - `doctrine-gap.md` — gaps between doctrine and implementation.
  - `v1-proposal.md` — new semantics beyond frozen v0 kernel.
- `config.yml` disables blank issues to force structured selection.
- Structural forks require escalation, not ad hoc issues.

## Documentation Authority

- Canonical authority is `/docs`.
- Each doc declares authority in frontmatter.
- Authority precedence: `FROZEN -> STABLE -> CANONICAL -> PROJECT`.
- If implementation contradicts a higher-authority document, the implementation is wrong.
- Do not use `docs_legacy/` as normative authority unless explicitly instructed for historical comparison.

## Ledger Convention (Required)

Use the ledger lanes strictly:

1. **Dev work (delivery):** `docs/ledger/dev-work/open|closed`
   - One file per delivery scope/branch.
   - Closure rows must be objective and testable.
2. **Gap work (risk/doctrine):** `docs/ledger/gap-work/open|closed`
   - Use for unresolved ambiguity, contradiction, or escalation.
   - Every row must name the decision owner and unblock condition.
3. **Decisions:** `docs/ledger/decisions/`
   - Record final rulings that unblock dev or close gaps.

Rules:

- Never mix delivery tasks and gap/escalation tasks in the same ledger file.
- Move files from `open/` to `closed/` only when all closure conditions are met.
- Keep cross-links accurate between lane files and `docs/ledger/closure-register.md`.

## Multi-Agent Review Flow

All implementation work requires doctrine review before merge. No agent merges their own work; Sebastian is the sole merge authority.
