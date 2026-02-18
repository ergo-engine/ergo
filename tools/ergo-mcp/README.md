# Ergo MCP Server v2 — Full Orchestration

Implements COLLABORATION_PROTOCOLS.md as executable infrastructure.

Claude Desktop becomes Structural Auditor + Doctrine Owner + Claude Code Supervisor.

## Capabilities

### Resources (Read Access)
- **Governance docs** — IDENTITY_AND_AUTHORITY.md, ONTOLOGICAL_BOUNDARIES.md, etc.
- **PHASE_INVARIANTS.md** — Canonical invariant reference
- **Frozen docs** — Read-only access (modifications blocked)

### Tools

**File Operations:**
- `read_file` — Read any file
- `list_directory` — List contents
- `search_code` — Ripgrep search

**Build & Test:**
- `check_build` — cargo check --workspace
- `run_tests` — cargo test with optional filter

**Git:**
- `git_status` — Current status
- `git_diff` — Diff against branch
- `git_log` — Recent commits

**Governance:**
- `list_invariants` — All invariant IDs from PHASE_INVARIANTS.md
- `check_invariant` — Check if invariant exists and get status
- `validate_against_governance` — Check if changes comply with docs

**Task Dispatch (Async, COLLABORATION_PROTOCOLS.md §7):**
- `create_task` — Create structured task (validates format)
- `dispatch_task` — Send task to Claude Code CLI (async, returns session_id)
- `dispatch_structured_task` — Create + validate + dispatch in one call (async)
- `get_claude_result` — Retrieve result by session_id
- `list_claude_sessions` — List all Claude Code sessions with status

**Verification:**
- `verify_task_completion` — Check invariant, files, run tests
- `render_verdict` — Record compliant/violation/missing_enforcement/new_concept

**Codex (Async Cross-Agent Verification):**
- `ask_codex` — Ask Codex a question (async, returns session_id)
- `get_codex_result` — Retrieve result by session_id
- `continue_codex` — Resume a specific or most recent session with follow-up
- `list_codex_sessions` — List all sessions with status
- `audit_with_codex` — Ask Codex to audit (ontology/invariant/design/implementation)
- `verify_with_codex` — Ask Codex to verify a claim against doctrine

All codex tools support a `files` parameter to reference specific files/directories.

**Audit:**
- `get_audit_log` — All decisions logged to audit.jsonl

## Claude Code Integration

Task dispatch runs asynchronously to avoid blocking Claude Desktop:

```
1. dispatch_task(task_prompt="...")
   → Returns session_id immediately (e.g., "def456")

2. get_claude_result(session_id="def456")
   → Returns "still running" or the completed response

3. list_claude_sessions()
   → Shows all sessions: running, completed, failed
```

For structured tasks with governance validation:

```
1. dispatch_structured_task(
     description="Add error handling",
     invariant_id="ADP-5",
     files=["crates/foo/src/lib.rs"],
     change="Add Result return type",
     verification="cargo test",
     constraints="Do not change public API"
   )
   → Validates against governance, returns session_id

2. get_claude_result(session_id="...")
   → Retrieve output when ready
```

Sessions persist to `claude_sessions.json` and survive Claude Desktop restarts.

## Codex Integration

Codex tools run asynchronously to avoid blocking Claude Desktop:

```
1. ask_codex(question="How does X work?", files=["crates/foo/src/lib.rs"])
   → Returns session_id immediately (e.g., "abc123")

2. get_codex_result(session_id="abc123")
   → Returns "still running" or the completed response

3. continue_codex(session_id="abc123", message="What about Y?")
   → Resumes that specific conversation with follow-up
   → If session_id omitted, resumes most recent session

4. list_codex_sessions()
   → Shows all sessions: running, completed, failed
```

### Session Resumption

When a Codex session completes, the server parses and stores the Codex CLI's internal session ID (e.g., `019be911-3288-7303-9a38-c3e579571b28`). This enables resuming specific conversations:

```
# Resume a specific session
continue_codex(session_id="abc123", message="Follow-up question")

# Resume most recent session (fallback)
continue_codex(message="Follow-up question")
```

### Completion Detection

Sessions are detected as complete when:
1. Output file exists
2. File is non-empty
3. File hasn't been modified in 2+ seconds (stable)

This file-based detection is more reliable than PID checking across restarts.

Sessions persist to `codex_sessions.json` and survive Claude Desktop restarts.

## Protocol Enforcement

The MCP server enforces:

1. **Task format** — All tasks must include invariant_id, files, change, verification, constraints
2. **Frozen file protection** — Cannot dispatch tasks that modify FROZEN docs
3. **Audit logging** — Every action logged with timestamp
4. **Verdict recording** — Per COLLABORATION_PROTOCOLS.md §6

## Setup

```bash
cd tools/ergo-mcp
pip install mcp
```

Add to Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "ergo": {
      "command": "/Users/sebastian/anaconda3/bin/python",
      "args": ["/Users/sebastian/Projects/ergo/tools/ergo-mcp/server.py"]
    }
  }
}
```

Restart Claude Desktop.

## Workflow

```
1. Claude reads governance docs (via resources)
2. Claude creates structured task (create_task)
3. Claude validates against governance (validate_against_governance)
4. Claude dispatches to Claude Code (dispatch_structured_task)
5. Claude checks result (get_claude_result)
6. Claude verifies completion (verify_task_completion)
7. Claude records verdict (render_verdict)

For cross-agent verification:
8. Claude asks Codex for review (ask_codex / audit_with_codex)
9. Claude retrieves Codex response (get_codex_result)
10. Claude continues conversation if needed (continue_codex)

All actions logged to audit.jsonl
```

## Requirements

- Python 3.10+
- `mcp` package (`pip install mcp`)
- `claude` CLI installed at `~/.local/bin/claude`
- `codex` CLI installed at `/opt/homebrew/bin/codex`
- `rg` (ripgrep) installed
- Rust/cargo installed at `~/.cargo/bin/cargo`

## Files

- `server.py` — MCP server implementation
- `audit.jsonl` — Append-only audit log
- `claude_sessions.json` — Persistent Claude Code session tracking
- `codex_sessions.json` — Persistent Codex session tracking
