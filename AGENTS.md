# Repository Guidelines

## Agent-Specific Protocols

**If you are Claude (Claude Code):** Read `.agents/.claude/CLAUDE.MD` and `.agents/.claude/CLAUDE_CODE_PROTOCOL.md` before proceeding. You report to Claude Prime (Doctrine Owner).

**If you are Codex:** Read `.agents/.codex/CODEX_PROTOCOL.md` before proceeding. You report to ChatGPT (Build Orchestrator).

These protocols define your authority boundaries, escalation rules, and the multi-agent review flow. Deviation is a contract violation.

---

## Project Structure & Module Organization
- `crates/runtime/`, `crates/adapter/`, `crates/supervisor/`: core Rust crates.
- `crates/ui-authoring/`: Vite/React authoring UI (excluded from Cargo workspace).
- `docs/`: authoritative specs and contracts (`docs/INDEX.md`); `target/` is generated.

## Build, Test, and Development Commands
Rust (run from repo root):
- `cargo build` — build workspace.
- `cargo test` — run all Rust tests.
- `cargo test -p ergo-runtime` — run a single crate.
- `cargo fmt` — format with rustfmt.

UI (run from `crates/ui-authoring`):
- `npm install` — install dependencies.
- `npm run dev` — start Vite dev server.
- `npm run build` — production build.
- `npm run typecheck` — TypeScript typecheck.

## Coding Style & Naming Conventions
- Rust 2021; follow rustfmt defaults and standard Rust casing (`snake_case` modules/functions, `PascalCase` types).
- Core layers must stay domain-neutral; exceptions require PR justification (see `docs/CANONICAL/TERMINOLOGY.md`).
- UI components in `crates/ui-authoring/src/ui` use `PascalCase.tsx`.

## Testing Guidelines
- Unit tests live alongside code with `#[test]`; integration tests are in `crates/supervisor/tests`.
- Golden Spike tests are canonical execution paths: `crates/runtime/src/runtime/tests.rs` and `crates/supervisor/tests/integration.rs`.

## Commit & Pull Request Guidelines
- Commit messages use Conventional Commits with optional scope, e.g. `feat(supervisor): ...`.
- PRs must map invariants + test evidence (`docs/CANONICAL/PHASE_INVARIANTS.md`); Supervisor internals require doctrine review (`docs/FROZEN/SUPERVISOR.md`); serialized term renames need compat aliases + tests (`docs/CANONICAL/TERMINOLOGY.md`).

## GitHub Mechanics & Templates
- PRs use `.github/PULL_REQUEST_TEMPLATE.md`.
- Issues use `.github/ISSUE_TEMPLATE/` with structured templates:
  - `doctrine-gap.md` — gaps between doctrine and implementation (COLLABORATION_PROTOCOLS.md §10)
  - `v1-proposal.md` — new semantics beyond frozen v0 kernel (KERNEL_CLOSURE.md)
- `config.yml` disables blank issues to force structured selection.
- Structural forks require escalation, not issues (COLLABORATION_PROTOCOLS.md §9).

## Documentation Authority
- `docs/` is the authoritative source. When specs conflict, higher authority wins: FROZEN → STABLE → CANONICAL → PROJECT.
- CONTRACTS (`docs/CONTRACTS/`) define external interfaces separately.
- If implementation contradicts higher-authority docs, the code is wrong.

## Multi-Agent Review Flow

All implementation work requires doctrine review before merge. No agent merges their own work; Sebastian is the sole merge authority.
