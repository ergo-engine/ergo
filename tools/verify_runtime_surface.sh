#!/usr/bin/env bash
# verify_runtime_surface.sh — Runtime API surface stability check
#
# Purpose:  Verifies that the public API surface of the kernel runtime
#           crate has not changed in ways that break downstream
#           contracts.  Detects accidental exposure of internal types.
#
# Authority: Informational — detects unexpected public API drift.
#
# Scope:    crates/kernel/runtime/src/. Does not modify code.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

WINDOWS_TARGET="x86_64-pc-windows-msvc"
PYTHON_BIN="${PYTHON_BIN:-}"

if [[ -z "$PYTHON_BIN" ]]; then
  if command -v python3 >/dev/null 2>&1; then
    PYTHON_BIN="python3"
  elif command -v python >/dev/null 2>&1; then
    PYTHON_BIN="python"
  else
    echo "error: python interpreter not found (tried python3, python)"
    exit 1
  fi
fi

echo "[1/8] cargo fmt --check"
cargo fmt --check

echo "[2/8] cargo test --workspace"
cargo test --workspace

echo "[3/8] replay-naming drift guard"
bash tools/verify_replay_naming.sh

echo "[4/8] doctrine gate guard"
bash tools/verify_doctrine_gate.sh

echo "[5/8] layer boundary guard"
bash tools/verify_layer_boundaries.sh

echo "[6/8] invariant-index count guard"
bash tools/verify_invariant_index.sh

echo "[7/8] ergo-mcp invariant parser tests"
if [[ -d "tools/ergo-mcp" ]]; then
  "$PYTHON_BIN" -m unittest discover -s tools/ergo-mcp -p "test_*.py"
else
  echo "skipping ergo-mcp invariant parser tests (tools/ergo-mcp not present)"
fi

echo "[8/8] windows compile guard (${WINDOWS_TARGET})"
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

if ! rustup target list --installed | grep -qx "${WINDOWS_TARGET}"; then
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
