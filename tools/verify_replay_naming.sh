#!/usr/bin/env bash
# verify_replay_naming.sh — Replay type and naming convention check
#
# Purpose:  Enforces naming conventions for replay-related types,
#           functions, and error variants across the host and
#           supervisor crates to prevent naming drift.
#
# Authority: Informational — detects convention violations.
#
# Scope:    Replay-related source files in kernel and host. Does not modify code.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if command -v rg >/dev/null 2>&1; then
  SEARCH_CMD=(rg -n)
else
  SEARCH_CMD=(grep -En)
fi

if "${SEARCH_CMD[@]}" "demo-1-replay\\.json|fixture-replay\\.json|println!\\(\\\"replay artifact:" \
  crates/prod/clients/cli/src/main.rs \
  crates/kernel/supervisor/src/fixture_runner.rs \
  crates/kernel/adapter/src/fixture.rs; then
  echo "error: stale replay-oriented run artifact naming found"
  exit 1
fi

echo "replay naming guard passed"
