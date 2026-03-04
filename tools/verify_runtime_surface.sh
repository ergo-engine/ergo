#!/usr/bin/env bash
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

if command -v rg >/dev/null 2>&1; then
  SEARCH_CMD=(rg -n)
else
  SEARCH_CMD=(grep -En)
fi

resolve_phase_invariants_path() {
  if [[ -f "docs_legacy/CANONICAL/PHASE_INVARIANTS.md" ]]; then
    echo "docs_legacy/CANONICAL/PHASE_INVARIANTS.md"
    return
  fi
  if [[ -f "docs/CANONICAL/PHASE_INVARIANTS.md" ]]; then
    echo "docs/CANONICAL/PHASE_INVARIANTS.md"
    return
  fi
  echo ""
}

PHASE_INVARIANTS_PATH="$(resolve_phase_invariants_path)"
if [[ -z "$PHASE_INVARIANTS_PATH" ]]; then
  echo "error: phase invariants file not found (expected docs_legacy/CANONICAL/PHASE_INVARIANTS.md or docs/CANONICAL/PHASE_INVARIANTS.md)"
  exit 1
fi

echo "[1/9] cargo fmt --check"
cargo fmt --check

echo "[2/9] cargo test -p ergo-supervisor"
cargo test -p ergo-supervisor

echo "[3/9] cargo test -p ergo-cli"
cargo test -p ergo-cli

echo "[4/9] cargo test"
cargo test

echo "[5/9] replay-naming drift guard"
if "${SEARCH_CMD[@]}" "demo-1-replay\\.json|fixture-replay\\.json|println!\\(\\\"replay artifact:" \
  crates/prod/clients/cli/src/main.rs \
  crates/kernel/supervisor/src/fixture_runner.rs \
  crates/kernel/adapter/src/fixture.rs; then
  echo "error: stale replay-oriented run artifact naming found"
  exit 1
fi

echo "[6/10] doctrine gate guard"
bash tools/verify_doctrine_gate.sh

echo "[7/10] layer boundary guard"
bash tools/verify_layer_boundaries.sh

echo "[8/10] phase-invariants count guard"
"$PYTHON_BIN" - "$PHASE_INVARIANTS_PATH" <<'PY'
import re
import sys
from pathlib import Path

doc_path = Path(sys.argv[1])
content = doc_path.read_text()

header_match = re.search(r"\*\*Tracked invariants:\*\*\s*([0-9]+)", content)
if not header_match:
    print("error: missing tracked invariant count header")
    sys.exit(1)

declared = int(header_match.group(1))
id_pattern = re.compile(r"^[A-Z][A-Z0-9]*(?:[.-][A-Z0-9]+)*[.-][0-9]+$")
ids = set()

for raw_line in content.splitlines():
    line = raw_line.strip()
    if not line.startswith("|"):
        continue
    cells = [cell.strip() for cell in line.split("|")]
    if len(cells) < 3:
        continue
    first_cell = cells[1]
    if first_cell.startswith("~~") and first_cell.endswith("~~") and len(first_cell) > 4:
        continue
    for left, right in [("~~", "~~"), ("**", "**"), ("`", "`")]:
        if first_cell.startswith(left) and first_cell.endswith(right) and len(first_cell) > (len(left) + len(right)):
            first_cell = first_cell[len(left):len(first_cell) - len(right)].strip()
    if id_pattern.fullmatch(first_cell):
        ids.add(first_cell)

parsed = len(ids)
if parsed != declared:
    print(f"error: invariant count drift detected (declared={declared}, parsed={parsed})")
    sys.exit(1)

print(f"phase invariants count check passed ({parsed})")
PY

echo "[9/10] ergo-mcp invariant parser tests"
if [[ -d "tools/ergo-mcp" ]]; then
  "$PYTHON_BIN" -m unittest discover -s tools/ergo-mcp -p "test_*.py"
else
  echo "skipping ergo-mcp invariant parser tests (tools/ergo-mcp not present)"
fi

echo "[10/10] windows compile guard (${WINDOWS_TARGET})"
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
