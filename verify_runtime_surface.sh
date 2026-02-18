#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_ROOT"

WINDOWS_TARGET="x86_64-pc-windows-msvc"

echo "[1/6] cargo fmt --check"
cargo fmt --check

echo "[2/6] cargo test -p ergo-supervisor"
cargo test -p ergo-supervisor

echo "[3/6] cargo test -p ergo-cli"
cargo test -p ergo-cli

echo "[4/6] cargo test"
cargo test

echo "[5/6] replay-naming drift guard"
if rg -n "demo-1-replay\\.json|fixture-replay\\.json|println!\\(\\\"replay artifact:" \
  crates/ergo-cli/src/main.rs \
  crates/supervisor/src/fixture_runner.rs \
  crates/adapter/src/fixture.rs; then
  echo "error: stale replay-oriented run artifact naming found"
  exit 1
fi

echo "[6/6] windows compile guard (${WINDOWS_TARGET})"
HOST_OS="$(uname -s 2>/dev/null || echo unknown)"
STRICT_WINDOWS_GUARD="${ERGO_STRICT_WINDOWS_GUARD:-0}"

if [[ "$STRICT_WINDOWS_GUARD" != "1" ]] && \
  [[ "$HOST_OS" != MINGW* ]] && \
  [[ "$HOST_OS" != MSYS* ]] && \
  [[ "$HOST_OS" != CYGWIN* ]]; then
  echo "skipping windows compile guard on non-Windows host (${HOST_OS})"
  echo "set ERGO_STRICT_WINDOWS_GUARD=1 to enforce this check locally"
  echo "verification passed"
  exit 0
fi

if ! command -v rustup >/dev/null 2>&1; then
  echo "error: rustup is required for cross-target verification"
  exit 1
fi

if ! rustup target list --installed | rg -qx "${WINDOWS_TARGET}"; then
  echo "error: missing rust target '${WINDOWS_TARGET}'"
  echo "install it with: rustup target add ${WINDOWS_TARGET}"
  exit 1
fi

if ! cargo check -p ergo-supervisor --target "${WINDOWS_TARGET}"; then
  echo "error: windows target compilation failed"
  echo "this usually means the host lacks a compatible Windows C toolchain/SDK"
  exit 1
fi

echo "verification passed"
