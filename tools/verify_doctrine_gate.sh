#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

resolve_docs_root() {
  if [[ -d "docs_legacy/CANONICAL" ]]; then
    echo "docs_legacy"
    return
  fi
  if [[ -d "docs/CANONICAL" ]]; then
    echo "docs"
    return
  fi
  echo ""
}

DOCS_ROOT="$(resolve_docs_root)"
if [[ -z "$DOCS_ROOT" ]]; then
  echo "error: unable to locate docs root (expected docs_legacy/CANONICAL or docs/CANONICAL)"
  exit 1
fi

LEDGER="${DOCS_ROOT}/CANONICAL/DOCTRINE_GAPS/SUP2_RUNRESULT_ALIGNMENT.md"
CLAIM_PATTERN='canonical complete|full canonical closure'

if [[ ! -f "$LEDGER" ]]; then
  echo "error: doctrine ledger not found at $LEDGER"
  exit 1
fi

OPEN_ROWS="$(python3 - "$LEDGER" <<'PY'
import re
import sys
from pathlib import Path

ledger = Path(sys.argv[1]).read_text().splitlines()
open_rows = []

for line in ledger:
    if not line.strip().startswith("| D"):
        continue
    cells = [cell.strip() for cell in line.split("|")]
    if len(cells) < 7:
        continue
    row_id = cells[1]
    status = cells[6].upper()
    if status != "CLOSED":
        open_rows.append(f"{row_id}:{cells[6]}")

print("\n".join(open_rows))
PY
)"

if [[ -n "${OPEN_ROWS}" ]]; then
  if command -v rg >/dev/null 2>&1; then
    SEARCH_ROOTS=("$DOCS_ROOT")
    if [[ "$DOCS_ROOT" != "docs" && -d "docs" ]]; then
      SEARCH_ROOTS+=("docs")
    fi
    if rg -n -i "$CLAIM_PATTERN" \
      "${SEARCH_ROOTS[@]}" ./*.md \
      --glob "!${LEDGER}" \
      >/tmp/ergo_doctrine_gate_matches.txt 2>/dev/null; then
      echo "error: DOC-GATE-1 violation: canonical-complete claim found with open doctrine gaps"
      echo "open rows:"
      echo "$OPEN_ROWS" | sed 's/^/  - /'
      echo "matches:"
      cat /tmp/ergo_doctrine_gate_matches.txt
      rm -f /tmp/ergo_doctrine_gate_matches.txt
      exit 1
    fi
  else
    if grep -Ein "$CLAIM_PATTERN" "${DOCS_ROOT}"/*.md ./*.md \
      | grep -v "$LEDGER" \
      >/tmp/ergo_doctrine_gate_matches.txt 2>/dev/null; then
      echo "error: DOC-GATE-1 violation: canonical-complete claim found with open doctrine gaps"
      echo "open rows:"
      echo "$OPEN_ROWS" | sed 's/^/  - /'
      echo "matches:"
      cat /tmp/ergo_doctrine_gate_matches.txt
      rm -f /tmp/ergo_doctrine_gate_matches.txt
      exit 1
    fi
  fi
fi

echo "doctrine gate passed"
