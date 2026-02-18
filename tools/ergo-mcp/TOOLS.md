# Ergo MCP Tools Reference

## File Operations
- `read_file(path)` — Read file contents
- `list_directory(path?)` — List directory (default: project root)
- `search_code(query, path?, file_type?)` — Ripgrep search

## Build & Test
- `check_build()` — cargo check --workspace
- `run_tests(filter?, show_output?)` — cargo test

## Git
- `git_status()` — Current status
- `git_diff(branch?, file?)` — Diff (default: main)
- `git_log(count?)` — Recent commits (default: 10)

## Governance
- `list_invariants()` — All invariant IDs
- `check_invariant(invariant_id)` — Check if invariant exists
- `validate_against_governance(files[], change_description)` — Validate changes

## Claude Code (Async)
- `dispatch_task(task_prompt)` → session_id
- `dispatch_structured_task(description, invariant_id, files[], change, verification, constraints)` → session_id
- `get_claude_result(session_id)` — Get result or "still running"
- `list_claude_sessions(limit?)` — List all sessions

## Codex (Async)
- `ask_codex(question, context?, files?[])` → session_id
- `audit_with_codex(subject, content, audit_type, files?[])` → session_id
  - audit_type: "ontology" | "invariant" | "design" | "implementation"
- `verify_with_codex(claim, doctrine_refs?[], files?[])` → session_id
- `get_codex_result(session_id)` — Get result or "still running"
- `continue_codex(message, session_id?, files?[])` — Resume conversation
- `list_codex_sessions(limit?)` — List all sessions

## Verification
- `verify_task_completion(invariant_id, verification_command?, expected_files?[])` — Verify task
- `render_verdict(verdict, reasoning, invariant_ids?[])` — Record verdict
  - verdict: "compliant" | "violation" | "missing_enforcement" | "new_concept"

## Audit
- `get_audit_log(limit?)` — Recent audit entries (default: 50)

---

## Async Workflow Pattern

```
1. ask_codex(...) → "abc123"
2. get_codex_result("abc123") → "still running" or response
3. continue_codex(session_id="abc123", message="...") → "def456"
4. get_codex_result("def456") → response
```

Sessions persist across restarts.
