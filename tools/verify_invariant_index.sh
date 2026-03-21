#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

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

INVARIANT_INDEX="docs/invariants/INDEX.md"
if [[ ! -f "$INVARIANT_INDEX" ]]; then
  echo "error: invariant index not found at $INVARIANT_INDEX"
  exit 1
fi

"$PYTHON_BIN" - "$INVARIANT_INDEX" <<'PY'
import re
import sys
from pathlib import Path

index_path = Path(sys.argv[1])
doc_paths = [index_path, *sorted(index_path.parent.glob("[0-9][0-9]-*.md"))]
content = index_path.read_text()

header_match = re.search(r"\*\*Tracked invariants:\*\*\s*([0-9]+)", content)
if not header_match:
    print("error: missing tracked invariant count header")
    sys.exit(1)

declared = int(header_match.group(1))
id_pattern = re.compile(r"^[A-Z][A-Z0-9]*(?:[.-][A-Z0-9]+)*[.-][0-9]+$")
ids = set()

for doc_path in doc_paths:
    for raw_line in doc_path.read_text().splitlines():
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

print(f"invariant index count check passed ({parsed})")
PY
