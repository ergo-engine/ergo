#!/usr/bin/env python3
"""Regression tests for PHASE_INVARIANTS parsing in ergo-mcp."""

import importlib.util
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]
SERVER_PATH = REPO_ROOT / "tools" / "ergo-mcp" / "server.py"


def load_server_module():
    spec = importlib.util.spec_from_file_location("ergo_mcp_server", SERVER_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class PhaseInvariantParserTests(unittest.TestCase):
    def setUp(self):
        self.server = load_server_module()

    def test_uses_canonical_phase_invariants_path(self):
        parsed = self.server.parse_phase_invariants()
        self.assertEqual(
            parsed.source_path,
            "docs/CANONICAL/PHASE_INVARIANTS.md",
        )
        self.assertFalse(parsed.degraded_mode)

    def test_declared_count_matches_parsed_ids(self):
        parsed = self.server.parse_phase_invariants()
        self.assertIsNotNone(parsed.declared_count)
        self.assertEqual(parsed.declared_count, len(parsed.invariants))

    def test_known_invariants_are_present(self):
        parsed = self.server.parse_phase_invariants()
        for invariant_id in ["X.1", "ADP-17", "RUN-CANON-1", "REP-7"]:
            self.assertIn(invariant_id, parsed.invariants)


if __name__ == "__main__":
    unittest.main()
