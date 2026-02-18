#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_ROOT"

WINDOWS_TARGET="x86_64-pc-windows-msvc"

echo "[1/8] cargo fmt --check"
cargo fmt --check

echo "[2/8] cargo test -p ergo-supervisor"
cargo test -p ergo-supervisor

echo "[3/8] cargo test -p ergo-cli"
cargo test -p ergo-cli

echo "[4/8] cargo test"
cargo test

echo "[5/8] replay-naming drift guard"
if rg -n "demo-1-replay\\.json|fixture-replay\\.json|println!\\(\\\"replay artifact:" \
  crates/ergo-cli/src/main.rs \
  crates/supervisor/src/fixture_runner.rs \
  crates/adapter/src/fixture.rs; then
  echo "error: stale replay-oriented run artifact naming found"
  exit 1
fi

echo "[6/8] phase-invariants count guard"
python3 - <<'PY'
import re
import sys
from pathlib import Path

doc_path = Path("docs/CANONICAL/PHASE_INVARIANTS.md")
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

echo "[7/8] ergo-mcp invariant parser tests"
python3 -m unittest discover -s tools/ergo-mcp -p "test_*.py"

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
