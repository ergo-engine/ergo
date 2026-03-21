#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

resolve_docs_root() {
  if [[ -d "docs/invariants" ]] && [[ -d "docs/ledger/gap-work/open" ]]; then
    echo "docs"
    return
  fi
  echo ""
}

DOCS_ROOT="$(resolve_docs_root)"
if [[ -z "$DOCS_ROOT" ]]; then
  echo "error: unable to locate current docs root (expected docs/invariants and docs/ledger/gap-work/open)"
  exit 1
fi

OPEN_GAP_DIR="${DOCS_ROOT}/ledger/gap-work/open"
CLAIM_PATTERN='canonical complete|full canonical closure'

if [[ ! -d "$OPEN_GAP_DIR" ]]; then
  echo "error: doctrine open-gap directory not found at $OPEN_GAP_DIR"
  exit 1
fi

OPEN_GAPS="$(python3 - "$OPEN_GAP_DIR" <<'PY'
import sys
from pathlib import Path

open_dir = Path(sys.argv[1])
rows = []

for path in sorted(open_dir.glob("*.md")):
    if path.name == ".gitkeep":
        continue
    rows.append(path.as_posix())

print("\n".join(rows))
PY
)"

if [[ -n "${OPEN_GAPS}" ]]; then
  if command -v rg >/dev/null 2>&1; then
    if rg -n -i "$CLAIM_PATTERN" \
      docs ./*.md \
      --glob "!docs/ledger/**" \
      >/tmp/ergo_doctrine_gate_matches.txt 2>/dev/null; then
      echo "error: DOC-GATE-1 violation: canonical-complete claim found with open doctrine gaps"
      echo "open gaps:"
      echo "$OPEN_GAPS" | sed 's/^/  - /'
      echo "matches:"
      cat /tmp/ergo_doctrine_gate_matches.txt
      rm -f /tmp/ergo_doctrine_gate_matches.txt
      exit 1
    fi
  else
    if grep -Ein "$CLAIM_PATTERN" docs/**/*.md ./*.md 2>/dev/null \
      | grep -v "docs/ledger/" \
      >/tmp/ergo_doctrine_gate_matches.txt 2>/dev/null; then
      echo "error: DOC-GATE-1 violation: canonical-complete claim found with open doctrine gaps"
      echo "open gaps:"
      echo "$OPEN_GAPS" | sed 's/^/  - /'
      echo "matches:"
      cat /tmp/ergo_doctrine_gate_matches.txt
      rm -f /tmp/ergo_doctrine_gate_matches.txt
      exit 1
    fi
  fi
fi

echo "doctrine gate passed"
