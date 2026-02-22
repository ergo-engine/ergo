#!/usr/bin/env python3
"""Regression tests for PHASE_INVARIANTS parsing in ergo-mcp."""

import importlib.util
from pathlib import Path
import sys
import types
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]
SERVER_PATH = REPO_ROOT / "tools" / "ergo-mcp" / "server.py"


def load_server_module():
    ensure_mcp_modules_available()
    spec = importlib.util.spec_from_file_location("ergo_mcp_server", SERVER_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def ensure_mcp_modules_available():
    """Provide lightweight MCP stubs when the optional dependency is unavailable."""
    try:
        import mcp.server  # type: ignore  # noqa: F401
        import mcp.types  # type: ignore  # noqa: F401
        return
    except ModuleNotFoundError:
        pass

    if "mcp.server" in sys.modules and "mcp.types" in sys.modules:
        return

    class StubServer:
        def __init__(self, *args, **kwargs):
            self.args = args
            self.kwargs = kwargs

        def list_resources(self):
            return lambda fn: fn

        def read_resource(self):
            return lambda fn: fn

        def list_tools(self):
            return lambda fn: fn

        def call_tool(self):
            return lambda fn: fn

    def stub_stdio_server(*args, **kwargs):
        raise RuntimeError("stdio_server is not available in test stubs")

    class StubMcpType:
        def __init__(self, *args, **kwargs):
            self.args = args
            self.kwargs = kwargs

    mcp_module = types.ModuleType("mcp")
    mcp_server_module = types.ModuleType("mcp.server")
    mcp_server_stdio_module = types.ModuleType("mcp.server.stdio")
    mcp_types_module = types.ModuleType("mcp.types")

    mcp_server_module.Server = StubServer
    mcp_server_stdio_module.stdio_server = stub_stdio_server
    mcp_types_module.Resource = StubMcpType
    mcp_types_module.Tool = StubMcpType
    mcp_types_module.TextContent = StubMcpType

    mcp_module.server = mcp_server_module
    mcp_module.types = mcp_types_module
    mcp_server_module.stdio = mcp_server_stdio_module

    sys.modules["mcp"] = mcp_module
    sys.modules["mcp.server"] = mcp_server_module
    sys.modules["mcp.server.stdio"] = mcp_server_stdio_module
    sys.modules["mcp.types"] = mcp_types_module


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
