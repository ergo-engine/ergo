#!/usr/bin/env bash
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
