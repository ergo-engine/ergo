#!/usr/bin/env python3
"""
Ergo MCP Server v2 - Full Orchestration

This server implements the multi-agent protocol from COLLABORATION_PROTOCOLS.md,
giving Claude Desktop the ability to:

1. Read and verify against governance docs
2. Dispatch structured tasks to Claude Code
3. Verify implementation against invariants
4. Track audit trail of all decisions
5. Enforce protocol compliance

The goal: Claude Desktop as Structural Auditor + Doctrine Owner,
with Claude Code as supervised Implementation Assistant.
"""

import subprocess
import json
import os
import re
import hashlib
import time
from datetime import datetime
from pathlib import Path
from dataclasses import dataclass, asdict
from typing import Optional
from enum import Enum

from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import Resource, Tool, TextContent

# =============================================================================
# CONFIGURATION
# =============================================================================

PROJECT_ROOT = Path(__file__).parent.parent.parent
DOCS_DIR = PROJECT_ROOT / "docs"
CRATES_DIR = PROJECT_ROOT / "crates"
AUDIT_LOG = PROJECT_ROOT / "tools" / "ergo-mcp" / "audit.jsonl"

# Governance documents (in order of authority per ONTOLOGICAL_BOUNDARIES.md)
GOVERNANCE_DOCS = [
    "IDENTITY_AND_AUTHORITY.md",
    "ONTOLOGICAL_BOUNDARIES.md",
    "COLLABORATION_PROTOCOLS.md",
    "EXTENSION_CONTRACTS_ROADMAP.md",
]

# Frozen docs that cannot be modified
FROZEN_DOCS = [
    "docs/FROZEN/ontology.md",
    "docs/FROZEN/execution_model.md",
    "docs/FROZEN/V0_FREEZE.md",
    "docs/FROZEN/adapter_contract.md",
]

server = Server("ergo-mcp-v2")


# =============================================================================
# DATA STRUCTURES
# =============================================================================

class TaskStatus(str, Enum):
    PENDING = "pending"
    DISPATCHED = "dispatched"
    COMPLETED = "completed"
    FAILED = "failed"
    REJECTED = "rejected"


class Verdict(str, Enum):
    COMPLIANT = "compliant"
    VIOLATION = "violation"
    MISSING_ENFORCEMENT = "missing_enforcement"
    NEW_CONCEPT = "new_concept"


@dataclass
class Task:
    """Structured task per COLLABORATION_PROTOCOLS.md §7.3"""
    task_id: str
    description: str
    invariant_id: str
    files: list[str]
    change: str
    verification: str
    constraints: str
    status: TaskStatus = TaskStatus.PENDING
    created_at: str = ""
    completed_at: str = ""
    result: str = ""
    
    def __post_init__(self):
        if not self.created_at:
            self.created_at = datetime.utcnow().isoformat()
        if not self.task_id:
            self.task_id = hashlib.sha256(
                f"{self.description}{self.created_at}".encode()
            ).hexdigest()[:12]
    
    def to_prompt(self) -> str:
        """Format as Claude Code task prompt."""
        return f"""**Task:** {self.description}

**Invariant ID:** {self.invariant_id}

**File(s):** {', '.join(self.files)}

**Change:** {self.change}

**Verification:** {self.verification}

**Constraints:** {self.constraints}"""


@dataclass
class AuditEntry:
    """Audit log entry for all decisions."""
    timestamp: str
    action: str
    task_id: Optional[str]
    invariant_id: Optional[str]
    verdict: Optional[str]
    details: dict

    def __post_init__(self):
        if not self.timestamp:
            self.timestamp = datetime.utcnow().isoformat()


@dataclass
class CodexSession:
    """Async codex session for non-blocking execution."""
    session_id: str
    prompt: str
    files: list[str]
    status: str  # "running", "completed", "failed"
    started_at: str
    completed_at: str = ""
    output_file: str = ""
    pid: int = 0
    codex_session_id: str = ""  # Actual Codex CLI session ID for resumption

    def __post_init__(self):
        if not self.started_at:
            self.started_at = datetime.utcnow().isoformat()
        if not self.session_id:
            self.session_id = hashlib.sha256(
                f"{self.prompt}{self.started_at}".encode()
            ).hexdigest()[:12]
        if not self.output_file:
            self.output_file = f"/tmp/codex_{self.session_id}.txt"


# =============================================================================
# CODEX SESSION STORAGE
# =============================================================================

CODEX_SESSIONS_FILE = PROJECT_ROOT / "tools" / "ergo-mcp" / "codex_sessions.json"


def save_codex_session(session: CodexSession):
    """Save or update a codex session."""
    sessions = load_all_codex_sessions()
    sessions[session.session_id] = asdict(session)
    CODEX_SESSIONS_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(CODEX_SESSIONS_FILE, "w") as f:
        json.dump(sessions, f, indent=2)


def load_codex_session(session_id: str) -> Optional[CodexSession]:
    """Load a codex session by ID."""
    sessions = load_all_codex_sessions()
    if session_id in sessions:
        return CodexSession(**sessions[session_id])
    return None


def load_all_codex_sessions() -> dict:
    """Load all codex sessions."""
    if not CODEX_SESSIONS_FILE.exists():
        return {}
    with open(CODEX_SESSIONS_FILE) as f:
        return json.load(f)


def check_codex_session_status(session: CodexSession) -> CodexSession:
    """Check if a running session has completed."""
    if session.status != "running":
        return session

    output_path = Path(session.output_file)

    # Check if file exists
    if not output_path.exists():
        return session  # Still running, no output yet

    # Check if file has content
    if output_path.stat().st_size == 0:
        return session  # File exists but empty, still running

    # Check if file was modified recently (within 2 seconds = still being written)
    mtime = output_path.stat().st_mtime
    if time.time() - mtime < 2:
        return session  # Still being written

    # File exists, non-empty, and stable → completed
    session.status = "completed"
    session.completed_at = datetime.utcnow().isoformat()

    # Parse Codex session ID from output for resumption
    try:
        output_text = output_path.read_text()
        # Look for "session id: <uuid>" in Codex output
        match = re.search(r'session id:\s*([0-9a-f-]{36})', output_text)
        if match:
            session.codex_session_id = match.group(1)
    except Exception:
        pass  # Non-critical, resumption just won't work

    save_codex_session(session)
    return session


def format_codex_prompt(question: str, files: list[str], context: str = "") -> str:
    """Format a prompt with file references."""
    parts = []
    if files:
        parts.append(f"Focus on these files: {', '.join(files)}")
    if context:
        parts.append(f"Context: {context}")
    parts.append(question)
    return "\n\n".join(parts)


# =============================================================================
# CLAUDE SESSION STORAGE
# =============================================================================

@dataclass
class ClaudeSession:
    """Async Claude Code session for non-blocking task dispatch."""
    session_id: str
    task_id: str
    prompt: str
    status: str  # "running", "completed", "failed"
    started_at: str
    completed_at: str = ""
    output_file: str = ""
    pid: int = 0

    def __post_init__(self):
        if not self.started_at:
            self.started_at = datetime.utcnow().isoformat()
        if not self.session_id:
            self.session_id = hashlib.sha256(
                f"{self.prompt}{self.started_at}".encode()
            ).hexdigest()[:12]
        if not self.output_file:
            self.output_file = f"/tmp/claude_{self.session_id}.txt"


CLAUDE_SESSIONS_FILE = PROJECT_ROOT / "tools" / "ergo-mcp" / "claude_sessions.json"


def save_claude_session(session: ClaudeSession):
    """Save or update a claude session."""
    sessions = load_all_claude_sessions()
    sessions[session.session_id] = asdict(session)
    CLAUDE_SESSIONS_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(CLAUDE_SESSIONS_FILE, "w") as f:
        json.dump(sessions, f, indent=2)


def load_claude_session(session_id: str) -> Optional[ClaudeSession]:
    """Load a claude session by ID."""
    sessions = load_all_claude_sessions()
    if session_id in sessions:
        return ClaudeSession(**sessions[session_id])
    return None


def load_all_claude_sessions() -> dict:
    """Load all claude sessions."""
    if not CLAUDE_SESSIONS_FILE.exists():
        return {}
    with open(CLAUDE_SESSIONS_FILE) as f:
        return json.load(f)


def check_claude_session_status(session: ClaudeSession) -> ClaudeSession:
    """Check if a running session has completed."""
    if session.status != "running":
        return session

    output_path = Path(session.output_file)

    # Check if file exists
    if not output_path.exists():
        return session  # Still running, no output yet

    # Check if file has content
    if output_path.stat().st_size == 0:
        return session  # File exists but empty, still running

    # Check if file was modified recently (within 2 seconds = still being written)
    mtime = output_path.stat().st_mtime
    if time.time() - mtime < 2:
        return session  # Still being written

    # File exists, non-empty, and stable → completed
    session.status = "completed"
    session.completed_at = datetime.utcnow().isoformat()
    save_claude_session(session)
    return session


# =============================================================================
# AUDIT LOGGING
# =============================================================================

def log_audit(entry: AuditEntry):
    """Append to audit log."""
    AUDIT_LOG.parent.mkdir(parents=True, exist_ok=True)
    with open(AUDIT_LOG, "a") as f:
        f.write(json.dumps(asdict(entry)) + "\n")


def get_audit_log(limit: int = 50) -> list[dict]:
    """Read recent audit entries."""
    if not AUDIT_LOG.exists():
        return []
    
    entries = []
    with open(AUDIT_LOG) as f:
        for line in f:
            if line.strip():
                entries.append(json.loads(line))
    
    return entries[-limit:]


# =============================================================================
# GOVERNANCE HELPERS
# =============================================================================

PHASE_INVARIANTS_PATH = DOCS_DIR / "CANONICAL" / "PHASE_INVARIANTS.md"
LEGACY_PHASE_INVARIANTS_PATHS = [
    DOCS_DIR / "PHASE_INVARIANTS.md",
    PROJECT_ROOT / "PHASE_INVARIANTS.md",
]

INVARIANT_ID_PATTERN = re.compile(r"^[A-Z][A-Z0-9]*(?:[.-][A-Z0-9]+)*[.-][0-9]+$")


@dataclass
class ParsedPhaseInvariants:
    """Parsed invariant IDs from PHASE_INVARIANTS.md."""
    invariants: dict[str, dict]
    source_path: Optional[str]
    degraded_mode: bool
    declared_count: Optional[int]


def read_governance_doc(name: str) -> Optional[str]:
    """Read a governance document."""
    # Check project root first
    path = PROJECT_ROOT / name
    if path.exists():
        return path.read_text()
    
    # Check docs directory
    path = DOCS_DIR / name
    if path.exists():
        return path.read_text()
    
    return None


def get_all_governance_docs() -> dict[str, str]:
    """Load all governance docs into memory."""
    docs = {}
    for name in GOVERNANCE_DOCS:
        content = read_governance_doc(name)
        if content:
            docs[name] = content
    return docs


def extract_invariant_ids(content: str) -> list[str]:
    """Extract invariant IDs from content."""
    # Match patterns like: ADP-1, COMP-2, TRG-STATE-1, F.1, X.9
    pattern = r'\b([A-Z]{1,4}[-.](?:[A-Z0-9]+-)?[0-9]+)\b'
    ids = re.findall(pattern, content)
    return sorted(set(ids))


def check_frozen_file_modification(files: list[str]) -> list[str]:
    """Check if any files are frozen and should not be modified."""
    violations = []
    for f in files:
        for frozen in FROZEN_DOCS:
            if f.endswith(frozen) or frozen.endswith(f):
                violations.append(f"FROZEN: {f} cannot be modified (see ONTOLOGICAL_BOUNDARIES.md)")
    return violations


def resolve_phase_invariants_path() -> Optional[Path]:
    """Resolve canonical PHASE_INVARIANTS.md path, falling back to legacy locations."""
    if PHASE_INVARIANTS_PATH.exists():
        return PHASE_INVARIANTS_PATH

    for path in LEGACY_PHASE_INVARIANTS_PATHS:
        if path.exists():
            return path

    return None


def normalize_table_cell(cell: str) -> str:
    """Normalize common markdown formatting wrappers for table cells."""
    normalized = cell.strip()

    # Remove one layer of common wrappers while preserving interior text.
    wrapper_pairs = [("~~", "~~"), ("**", "**"), ("`", "`")]
    changed = True
    while changed:
        changed = False
        for left, right in wrapper_pairs:
            if normalized.startswith(left) and normalized.endswith(right) and len(normalized) > (len(left) + len(right)):
                normalized = normalized[len(left):len(normalized) - len(right)].strip()
                changed = True

    return normalized


def is_strikethrough_cell(cell: str) -> bool:
    """Return True if the entire cell is wrapped as markdown strikethrough."""
    normalized = cell.strip()
    return normalized.startswith("~~") and normalized.endswith("~~") and len(normalized) > 4


def parse_declared_invariant_count(content: str) -> Optional[int]:
    """Read the declared tracked invariant count from PHASE_INVARIANTS header."""
    match = re.search(r"\*\*Tracked invariants:\*\*\s*([0-9]+)", content)
    if not match:
        return None
    return int(match.group(1))


def parse_phase_invariants() -> ParsedPhaseInvariants:
    """Parse PHASE_INVARIANTS.md to extract invariant IDs from the first table column."""
    phase_path = resolve_phase_invariants_path()
    if not phase_path:
        return ParsedPhaseInvariants(
            invariants={},
            source_path=None,
            degraded_mode=True,
            declared_count=None,
        )

    content = phase_path.read_text()
    source_relpath = str(phase_path.relative_to(PROJECT_ROOT))
    invariants = {}

    for line in content.split("\n"):
        stripped = line.strip()
        if not stripped.startswith("|"):
            continue

        # Markdown table row. First user cell is the invariant ID column.
        cells = [cell.strip() for cell in stripped.split("|")]
        if len(cells) < 3:
            continue

        first_cell = cells[1]
        if is_strikethrough_cell(first_cell):
            # Closed-gap appendix rows are historical notes, not tracked invariants.
            continue

        candidate_id = normalize_table_cell(first_cell)
        if INVARIANT_ID_PATTERN.fullmatch(candidate_id):
            invariants[candidate_id] = {"raw_line": line}

    return ParsedPhaseInvariants(
        invariants=invariants,
        source_path=source_relpath,
        degraded_mode=(phase_path != PHASE_INVARIANTS_PATH),
        declared_count=parse_declared_invariant_count(content),
    )


# =============================================================================
# RESOURCES
# =============================================================================

@server.list_resources()
async def list_resources():
    """List governance docs and key project files as resources."""
    resources = []
    
    # Governance docs
    for doc in GOVERNANCE_DOCS:
        for base in [PROJECT_ROOT, DOCS_DIR]:
            path = base / doc
            if path.exists():
                resources.append(Resource(
                    uri=f"ergo://governance/{doc}",
                    name=f"[GOVERNANCE] {doc}",
                    description=f"Governance document (authority level: {GOVERNANCE_DOCS.index(doc) + 1})",
                    mimeType="text/markdown"
                ))
                break
    
    # PHASE_INVARIANTS
    phase_path = resolve_phase_invariants_path()
    if phase_path:
        resources.append(Resource(
            uri="ergo://invariants/PHASE_INVARIANTS.md",
            name="[CANONICAL] PHASE_INVARIANTS.md",
            description=f"Canonical invariant reference ({phase_path.relative_to(PROJECT_ROOT)})",
            mimeType="text/markdown"
        ))
    
    # Frozen docs
    frozen_dir = DOCS_DIR / "FROZEN"
    if frozen_dir.exists():
        for doc in frozen_dir.glob("*.md"):
            resources.append(Resource(
                uri=f"ergo://frozen/{doc.name}",
                name=f"[FROZEN] {doc.name}",
                description="FROZEN: Cannot be modified until v1",
                mimeType="text/markdown"
            ))
    
    return resources


@server.read_resource()
async def read_resource(uri: str) -> str:
    """Read a resource by URI."""
    if uri.startswith("ergo://governance/"):
        name = uri.replace("ergo://governance/", "")
        content = read_governance_doc(name)
        if content:
            return content
        raise FileNotFoundError(f"Governance doc not found: {name}")
    
    elif uri.startswith("ergo://invariants/"):
        name = uri.replace("ergo://invariants/", "")
        if name == "PHASE_INVARIANTS.md":
            phase_path = resolve_phase_invariants_path()
            if phase_path and phase_path.exists():
                return phase_path.read_text()
        else:
            content = read_governance_doc(name) or read_governance_doc(f"docs/{name}")
            if content:
                return content
        raise FileNotFoundError(f"Invariants doc not found: {name}")
    
    elif uri.startswith("ergo://frozen/"):
        name = uri.replace("ergo://frozen/", "")
        path = DOCS_DIR / "FROZEN" / name
        if path.exists():
            return path.read_text()
        raise FileNotFoundError(f"Frozen doc not found: {name}")
    
    raise ValueError(f"Unknown URI scheme: {uri}")


# =============================================================================
# TOOLS
# =============================================================================

@server.list_tools()
async def list_tools():
    """List all available tools."""
    return [
        # --- FILE OPERATIONS ---
        Tool(
            name="read_file",
            description="Read a file from the repo",
            inputSchema={
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path relative to project root"}
                },
                "required": ["path"]
            }
        ),
        Tool(
            name="list_directory",
            description="List directory contents",
            inputSchema={
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path relative to project root (default: root)"}
                }
            }
        ),
        Tool(
            name="search_code",
            description="Search code with ripgrep",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search pattern"},
                    "path": {"type": "string", "description": "Path to search (default: entire repo)"},
                    "file_type": {"type": "string", "description": "File extension filter (e.g., 'rs', 'md')"}
                },
                "required": ["query"]
            }
        ),
        
        # --- BUILD & TEST ---
        Tool(
            name="check_build",
            description="Run cargo check --workspace",
            inputSchema={"type": "object", "properties": {}}
        ),
        Tool(
            name="run_tests",
            description="Run cargo test --workspace",
            inputSchema={
                "type": "object",
                "properties": {
                    "filter": {"type": "string", "description": "Test name filter"},
                    "show_output": {"type": "boolean", "description": "Show stdout from tests"}
                }
            }
        ),
        
        # --- GIT ---
        Tool(
            name="git_status",
            description="Get current git status",
            inputSchema={"type": "object", "properties": {}}
        ),
        Tool(
            name="git_diff",
            description="Get diff against branch",
            inputSchema={
                "type": "object",
                "properties": {
                    "branch": {"type": "string", "description": "Branch to diff against (default: main)"},
                    "file": {"type": "string", "description": "Specific file to diff"}
                }
            }
        ),
        Tool(
            name="git_log",
            description="Get recent commit log",
            inputSchema={
                "type": "object",
                "properties": {
                    "count": {"type": "integer", "description": "Number of commits (default: 10)"}
                }
            }
        ),
        
        # --- GOVERNANCE ---
        Tool(
            name="list_invariants",
            description="List all known invariant IDs from PHASE_INVARIANTS.md",
            inputSchema={"type": "object", "properties": {}}
        ),
        Tool(
            name="check_invariant",
            description="Check if an invariant ID exists and get its status",
            inputSchema={
                "type": "object",
                "properties": {
                    "invariant_id": {"type": "string", "description": "Invariant ID (e.g., ADP-5, TRG-STATE-1)"}
                },
                "required": ["invariant_id"]
            }
        ),
        Tool(
            name="validate_against_governance",
            description="Check if proposed changes comply with governance docs",
            inputSchema={
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Files that will be modified"
                    },
                    "change_description": {"type": "string", "description": "What the change does"}
                },
                "required": ["files", "change_description"]
            }
        ),
        
        # --- TASK DISPATCH ---
        Tool(
            name="create_task",
            description="Create a structured task for Claude Code (does not dispatch yet)",
            inputSchema={
                "type": "object",
                "properties": {
                    "description": {"type": "string", "description": "Brief task description"},
                    "invariant_id": {"type": "string", "description": "Invariant ID from PHASE_INVARIANTS.md"},
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Files to modify"
                    },
                    "change": {"type": "string", "description": "Precise description of what to do"},
                    "verification": {"type": "string", "description": "How to confirm correctness"},
                    "constraints": {"type": "string", "description": "What NOT to do"}
                },
                "required": ["description", "invariant_id", "files", "change", "verification", "constraints"]
            }
        ),
        Tool(
            name="dispatch_task",
            description="Dispatch a task to Claude Code CLI (async) - returns session_id immediately",
            inputSchema={
                "type": "object",
                "properties": {
                    "task_prompt": {"type": "string", "description": "Full task prompt (from create_task)"}
                },
                "required": ["task_prompt"]
            }
        ),
        Tool(
            name="dispatch_structured_task",
            description="Create and dispatch a task in one call (async, with governance validation) - returns session_id",
            inputSchema={
                "type": "object",
                "properties": {
                    "description": {"type": "string", "description": "Brief task description"},
                    "invariant_id": {"type": "string", "description": "Invariant ID from PHASE_INVARIANTS.md"},
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Files to modify"
                    },
                    "change": {"type": "string", "description": "Precise description of what to do"},
                    "verification": {"type": "string", "description": "How to confirm correctness"},
                    "constraints": {"type": "string", "description": "What NOT to do"}
                },
                "required": ["description", "invariant_id", "files", "change", "verification", "constraints"]
            }
        ),
        Tool(
            name="get_claude_result",
            description="Get the result of an async Claude Code session",
            inputSchema={
                "type": "object",
                "properties": {
                    "session_id": {"type": "string", "description": "Session ID returned by dispatch_task/dispatch_structured_task"}
                },
                "required": ["session_id"]
            }
        ),
        Tool(
            name="list_claude_sessions",
            description="List all Claude Code sessions with their status",
            inputSchema={
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "description": "Number of sessions to show (default: 10)"}
                }
            }
        ),

        # --- VERIFICATION ---
        Tool(
            name="verify_task_completion",
            description="Verify a completed task against its invariant and verification criteria",
            inputSchema={
                "type": "object",
                "properties": {
                    "invariant_id": {"type": "string", "description": "Invariant that was addressed"},
                    "verification_command": {"type": "string", "description": "Command to run (e.g., 'cargo test')"},
                    "expected_files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Files that should have been modified"
                    }
                },
                "required": ["invariant_id"]
            }
        ),
        Tool(
            name="render_verdict",
            description="Record a verdict on completed work (per COLLABORATION_PROTOCOLS.md §6)",
            inputSchema={
                "type": "object",
                "properties": {
                    "verdict": {
                        "type": "string",
                        "enum": ["compliant", "violation", "missing_enforcement", "new_concept"],
                        "description": "Verdict type"
                    },
                    "invariant_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Invariant IDs involved"
                    },
                    "reasoning": {"type": "string", "description": "Explanation of verdict"}
                },
                "required": ["verdict", "reasoning"]
            }
        ),
        
        # --- AUDIT ---
        Tool(
            name="get_audit_log",
            description="Get recent audit log entries",
            inputSchema={
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "description": "Number of entries (default: 50)"}
                }
            }
        ),
        
        # --- CODEX (ChatGPT for audit/verification) ---
        Tool(
            name="ask_codex",
            description="Ask Codex a question (async) - returns session_id immediately, use get_codex_result to retrieve response",
            inputSchema={
                "type": "object",
                "properties": {
                    "question": {"type": "string", "description": "Question to ask Codex"},
                    "context": {"type": "string", "description": "Optional context to include"},
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Files or directories to reference (e.g., ['crates/ergo-runtime/src/lib.rs', 'docs/'])"
                    }
                },
                "required": ["question"]
            }
        ),
        Tool(
            name="get_codex_result",
            description="Get the result of an async codex session",
            inputSchema={
                "type": "object",
                "properties": {
                    "session_id": {"type": "string", "description": "Session ID returned by ask_codex/audit_with_codex/verify_with_codex"}
                },
                "required": ["session_id"]
            }
        ),
        Tool(
            name="continue_codex",
            description="Continue a conversation with Codex (resumes a specific or the most recent session)",
            inputSchema={
                "type": "object",
                "properties": {
                    "session_id": {"type": "string", "description": "Our session ID to resume (from list_codex_sessions). If omitted, resumes the most recent."},
                    "message": {"type": "string", "description": "Follow-up message to send"},
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Additional files to reference"
                    }
                },
                "required": ["message"]
            }
        ),
        Tool(
            name="list_codex_sessions",
            description="List all codex sessions with their status",
            inputSchema={
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "description": "Number of sessions to show (default: 10)"}
                }
            }
        ),
        Tool(
            name="audit_with_codex",
            description="Ask Codex to audit/review something (async) - returns session_id immediately",
            inputSchema={
                "type": "object",
                "properties": {
                    "subject": {"type": "string", "description": "What to audit (invariant, change, design decision)"},
                    "content": {"type": "string", "description": "The content to review"},
                    "audit_type": {
                        "type": "string",
                        "enum": ["ontology", "invariant", "design", "implementation"],
                        "description": "Type of audit"
                    },
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Files or directories to reference"
                    }
                },
                "required": ["subject", "content", "audit_type"]
            }
        ),
        Tool(
            name="verify_with_codex",
            description="Ask Codex to verify a claim against Ergo's doctrine (async) - returns session_id immediately",
            inputSchema={
                "type": "object",
                "properties": {
                    "claim": {"type": "string", "description": "The claim or decision to verify"},
                    "doctrine_refs": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Relevant doctrine documents (e.g., 'ONTOLOGICAL_BOUNDARIES.md')"
                    },
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Files or directories to reference"
                    }
                },
                "required": ["claim"]
            }
        ),
    ]


@server.call_tool()
async def call_tool(name: str, arguments: dict) -> list[TextContent]:
    """Execute a tool and return results."""
    
    try:
        # --- FILE OPERATIONS ---
        if name == "read_file":
            path = PROJECT_ROOT / arguments["path"]
            if not path.exists():
                return [TextContent(type="text", text=f"❌ File not found: {arguments['path']}")]
            return [TextContent(type="text", text=path.read_text())]
        
        elif name == "list_directory":
            path = PROJECT_ROOT / arguments.get("path", "")
            if not path.exists():
                return [TextContent(type="text", text=f"❌ Directory not found: {arguments.get('path', '')}")]
            
            entries = []
            for entry in sorted(path.iterdir()):
                if entry.name.startswith("."):
                    continue
                prefix = "📁" if entry.is_dir() else "📄"
                entries.append(f"{prefix} {entry.name}")
            
            return [TextContent(type="text", text="\n".join(entries) or "(empty)")]
        
        elif name == "search_code":
            cmd = ["rg", "--line-number", "--no-heading", arguments["query"]]
            if "file_type" in arguments:
                cmd.extend(["-t", arguments["file_type"]])
            cmd.append(arguments.get("path", "."))
            
            result = subprocess.run(cmd, cwd=PROJECT_ROOT, capture_output=True, text=True)
            output = result.stdout or "No matches found"
            return [TextContent(type="text", text=output)]
        
        # --- BUILD & TEST ---
        elif name == "check_build":
            result = subprocess.run(
                ["/Users/sebastian/.cargo/bin/cargo", "check", "--workspace"],
                cwd=PROJECT_ROOT, capture_output=True, text=True, timeout=120
            )
            status = "✅ Build OK" if result.returncode == 0 else "❌ Build FAILED"
            output = result.stderr or result.stdout
            return [TextContent(type="text", text=f"{status}\n\n{output}")]
        
        elif name == "run_tests":
            cmd = ["/Users/sebastian/.cargo/bin/cargo", "test", "--workspace"]
            if arguments.get("filter"):
                cmd.append(arguments["filter"])
            if arguments.get("show_output"):
                cmd.append("--nocapture")
            
            result = subprocess.run(cmd, cwd=PROJECT_ROOT, capture_output=True, text=True, timeout=300)
            
            # Parse test count
            test_match = re.search(r'(\d+) passed', result.stdout)
            passed = test_match.group(1) if test_match else "?"
            
            status = "✅ Tests passed" if result.returncode == 0 else "❌ Tests FAILED"
            return [TextContent(type="text", text=f"{status} ({passed} passed)\n\n{result.stdout}\n{result.stderr}")]
        
        # --- GIT ---
        elif name == "git_status":
            result = subprocess.run(
                ["git", "status", "--short", "--branch"],
                cwd=PROJECT_ROOT, capture_output=True, text=True
            )
            return [TextContent(type="text", text=result.stdout or "Working tree clean")]
        
        elif name == "git_diff":
            cmd = ["git", "diff", arguments.get("branch", "main")]
            if arguments.get("file"):
                cmd.extend(["--", arguments["file"]])
            
            result = subprocess.run(cmd, cwd=PROJECT_ROOT, capture_output=True, text=True)
            return [TextContent(type="text", text=result.stdout or "No changes")]
        
        elif name == "git_log":
            count = arguments.get("count", 10)
            result = subprocess.run(
                ["git", "log", f"-{count}", "--oneline"],
                cwd=PROJECT_ROOT, capture_output=True, text=True
            )
            return [TextContent(type="text", text=result.stdout)]
        
        # --- GOVERNANCE ---
        elif name == "list_invariants":
            parsed = parse_phase_invariants()
            invariants = parsed.invariants

            if not invariants:
                # Fallback: search all governance docs
                all_ids = set()
                for doc_name, content in get_all_governance_docs().items():
                    all_ids.update(extract_invariant_ids(content))

                output = "⚠️ degraded mode: canonical phase invariants document not found.\n"
                output += f"Found {len(all_ids)} fallback invariant IDs from governance docs:\n\n"
                output += "\n".join(sorted(all_ids))
                return [TextContent(type="text", text=output)]

            output = f"Found {len(invariants)} invariants in {parsed.source_path}:\n"
            if parsed.degraded_mode:
                output += "⚠️ degraded mode: using legacy PHASE_INVARIANTS path.\n"
            if parsed.declared_count is not None and parsed.declared_count != len(invariants):
                output += (
                    f"⚠️ count drift: declared={parsed.declared_count}, parsed={len(invariants)}.\n"
                )
            output += "\n"
            for inv_id in sorted(invariants.keys()):
                output += f"• {inv_id}\n"
            return [TextContent(type="text", text=output)]
        
        elif name == "check_invariant":
            inv_id = arguments["invariant_id"]
            parsed = parse_phase_invariants()
            invariants = parsed.invariants
            
            if inv_id in invariants:
                source = parsed.source_path or "PHASE_INVARIANTS.md"
                return [TextContent(type="text", text=f"✅ {inv_id} found in {source}\n\n{invariants[inv_id].get('raw_line', '')}")]
            
            # Search governance docs
            for doc_name, content in get_all_governance_docs().items():
                if inv_id in content:
                    return [TextContent(type="text", text=f"⚠️ {inv_id} found in {doc_name} but not in PHASE_INVARIANTS.md")]
            
            return [TextContent(type="text", text=f"❌ {inv_id} not found in any governance document")]
        
        elif name == "validate_against_governance":
            files = arguments["files"]
            description = arguments["change_description"]
            
            issues = []
            
            # Check frozen files
            frozen_violations = check_frozen_file_modification(files)
            issues.extend(frozen_violations)
            
            # Check for new concepts
            governance_docs = get_all_governance_docs()
            known_invariants = set()
            for content in governance_docs.values():
                known_invariants.update(extract_invariant_ids(content))
            known_invariants.update(parse_phase_invariants().invariants.keys())
            
            mentioned_ids = extract_invariant_ids(description)
            unknown_ids = [i for i in mentioned_ids if i not in known_invariants]
            if unknown_ids:
                issues.append(f"NEW CONCEPT: {unknown_ids} not found in governance docs")
            
            log_audit(AuditEntry(
                timestamp="",
                action="validate_against_governance",
                task_id=None,
                invariant_id=None,
                verdict="issues_found" if issues else "passed",
                details={"files": files, "description": description, "issues": issues}
            ))
            
            if issues:
                return [TextContent(type="text", text="❌ Governance validation FAILED:\n\n" + "\n".join(f"• {i}" for i in issues))]
            
            return [TextContent(type="text", text="✅ Governance validation passed\n\nNo frozen file violations or unknown concepts detected.")]
        
        # --- TASK DISPATCH ---
        elif name == "create_task":
            task = Task(
                task_id="",
                description=arguments["description"],
                invariant_id=arguments["invariant_id"],
                files=arguments["files"],
                change=arguments["change"],
                verification=arguments["verification"],
                constraints=arguments["constraints"]
            )
            
            prompt = task.to_prompt()
            
            log_audit(AuditEntry(
                timestamp="",
                action="create_task",
                task_id=task.task_id,
                invariant_id=task.invariant_id,
                verdict=None,
                details=asdict(task)
            ))
            
            return [TextContent(type="text", text=f"📋 Task created: {task.task_id}\n\n{prompt}\n\n---\nUse dispatch_task with this prompt to execute.")]
        
        elif name == "dispatch_task":
            prompt = arguments["task_prompt"]

            # Create session
            session = ClaudeSession(
                session_id="",
                task_id="",
                prompt=prompt,
                status="running",
                started_at=""
            )

            log_audit(AuditEntry(
                timestamp="",
                action="dispatch_task",
                task_id=None,
                invariant_id=None,
                verdict=None,
                details={"prompt_length": len(prompt), "session_id": session.session_id}
            ))

            # Start claude async
            with open(session.output_file, "w") as f:
                proc = subprocess.Popen(
                    ["/Users/sebastian/.local/bin/claude", "-p", prompt],
                    cwd=PROJECT_ROOT,
                    stdout=f,
                    stderr=subprocess.STDOUT,
                    start_new_session=True
                )
                session.pid = proc.pid

            save_claude_session(session)
            return [TextContent(type="text", text=f"🚀 Claude Code session started: **{session.session_id}**\n\nUse `get_claude_result` with this session_id to retrieve the response.")]

        elif name == "dispatch_structured_task":
            # First validate against governance
            files = arguments["files"]
            frozen_violations = check_frozen_file_modification(files)
            if frozen_violations:
                return [TextContent(type="text", text="❌ BLOCKED: Cannot modify frozen files:\n\n" + "\n".join(frozen_violations))]

            # Create task
            task = Task(
                task_id="",
                description=arguments["description"],
                invariant_id=arguments["invariant_id"],
                files=files,
                change=arguments["change"],
                verification=arguments["verification"],
                constraints=arguments["constraints"]
            )

            prompt = task.to_prompt()

            # Create session
            session = ClaudeSession(
                session_id="",
                task_id=task.task_id,
                prompt=prompt,
                status="running",
                started_at=""
            )

            log_audit(AuditEntry(
                timestamp="",
                action="dispatch_structured_task",
                task_id=task.task_id,
                invariant_id=task.invariant_id,
                verdict=None,
                details={**asdict(task), "session_id": session.session_id}
            ))

            # Start claude async
            with open(session.output_file, "w") as f:
                proc = subprocess.Popen(
                    ["/Users/sebastian/.local/bin/claude", "-p", prompt],
                    cwd=PROJECT_ROOT,
                    stdout=f,
                    stderr=subprocess.STDOUT,
                    start_new_session=True
                )
                session.pid = proc.pid

            save_claude_session(session)
            return [TextContent(type="text", text=f"🚀 Claude Code task **{task.task_id}** dispatched: session **{session.session_id}**\n\nUse `get_claude_result` with this session_id to retrieve the response.")]

        elif name == "get_claude_result":
            session_id = arguments["session_id"]
            session = load_claude_session(session_id)

            if not session:
                return [TextContent(type="text", text=f"❌ Session not found: {session_id}")]

            session = check_claude_session_status(session)

            if session.status == "running":
                return [TextContent(type="text", text=f"⏳ Session **{session_id}** is still running (started {session.started_at})")]

            if session.status == "failed":
                return [TextContent(type="text", text=f"❌ Session **{session_id}** failed - no output file found")]

            # Read output
            try:
                output = Path(session.output_file).read_text()
                return [TextContent(type="text", text=f"**Claude Code response** (session {session_id}):\n\n{output}")]
            except Exception as e:
                return [TextContent(type="text", text=f"❌ Error reading output: {e}")]

        elif name == "list_claude_sessions":
            limit = arguments.get("limit", 10)
            sessions = load_all_claude_sessions()

            if not sessions:
                return [TextContent(type="text", text="No Claude Code sessions found.")]

            # Sort by started_at descending and limit
            sorted_sessions = sorted(
                sessions.values(),
                key=lambda s: s.get("started_at", ""),
                reverse=True
            )[:limit]

            output = f"**Claude Code Sessions** (last {len(sorted_sessions)}):\n\n"
            for s in sorted_sessions:
                session = ClaudeSession(**s)
                session = check_claude_session_status(session)
                status_icon = {"running": "⏳", "completed": "✅", "failed": "❌"}.get(session.status, "❓")
                output += f"- {status_icon} **{session.session_id}** ({session.status}) - {session.started_at[:19]}\n"
                if session.task_id:
                    output += f"  Task: {session.task_id}\n"
                output += f"  Prompt: {session.prompt[:60]}...\n"

            return [TextContent(type="text", text=output)]

        # --- VERIFICATION ---
        elif name == "verify_task_completion":
            inv_id = arguments["invariant_id"]
            verification_cmd = arguments.get("verification_command", "cargo test --workspace")
            expected_files = arguments.get("expected_files", [])
            
            results = []
            
            # Check invariant exists
            parsed = parse_phase_invariants()
            invariants = parsed.invariants
            if inv_id not in invariants:
                results.append(f"⚠️ Invariant {inv_id} not in PHASE_INVARIANTS.md")
            else:
                results.append(f"✅ Invariant {inv_id} found")
            
            # Check files were modified
            if expected_files:
                status_result = subprocess.run(
                    ["git", "status", "--short"],
                    cwd=PROJECT_ROOT, capture_output=True, text=True
                )
                modified = status_result.stdout
                for f in expected_files:
                    if f in modified or Path(PROJECT_ROOT / f).exists():
                        results.append(f"✅ File exists/modified: {f}")
                    else:
                        results.append(f"❌ File not found/modified: {f}")
            
            # Run verification command
            if verification_cmd:
                cmd_parts = verification_cmd.split()
                result = subprocess.run(
                    cmd_parts,
                    cwd=PROJECT_ROOT, capture_output=True, text=True, timeout=300
                )
                if result.returncode == 0:
                    results.append(f"✅ Verification passed: {verification_cmd}")
                else:
                    results.append(f"❌ Verification failed: {verification_cmd}\n{result.stderr}")
            
            log_audit(AuditEntry(
                timestamp="",
                action="verify_task_completion",
                task_id=None,
                invariant_id=inv_id,
                verdict="passed" if all("✅" in r for r in results) else "failed",
                details={"results": results}
            ))
            
            return [TextContent(type="text", text="\n".join(results))]
        
        elif name == "render_verdict":
            verdict = arguments["verdict"]
            inv_ids = arguments.get("invariant_ids", [])
            reasoning = arguments["reasoning"]
            
            log_audit(AuditEntry(
                timestamp="",
                action="render_verdict",
                task_id=None,
                invariant_id=",".join(inv_ids) if inv_ids else None,
                verdict=verdict,
                details={"reasoning": reasoning}
            ))
            
            emoji = {
                "compliant": "✅",
                "violation": "❌",
                "missing_enforcement": "⚠️",
                "new_concept": "🧱"
            }.get(verdict, "❓")
            
            output = f"""
{emoji} **VERDICT: {verdict.upper()}**

**Invariants:** {', '.join(inv_ids) if inv_ids else 'N/A'}

**Reasoning:** {reasoning}

---
Recorded in audit log.
"""
            return [TextContent(type="text", text=output)]
        
        # --- AUDIT ---
        elif name == "get_audit_log":
            limit = arguments.get("limit", 50)
            entries = get_audit_log(limit)
            
            if not entries:
                return [TextContent(type="text", text="Audit log is empty")]
            
            output = f"Last {len(entries)} audit entries:\n\n"
            for entry in entries:
                output += f"[{entry['timestamp']}] {entry['action']}"
                if entry.get('invariant_id'):
                    output += f" ({entry['invariant_id']})"
                if entry.get('verdict'):
                    output += f" → {entry['verdict']}"
                output += "\n"
            
            return [TextContent(type="text", text=output)]
        
        # --- CODEX (async) ---
        elif name == "ask_codex":
            question = arguments["question"]
            context = arguments.get("context", "")
            files = arguments.get("files", [])

            prompt = format_codex_prompt(question, files, context)

            # Create session
            session = CodexSession(
                session_id="",
                prompt=prompt,
                files=files,
                status="running",
                started_at=""
            )

            log_audit(AuditEntry(
                timestamp="",
                action="ask_codex",
                task_id=None,
                invariant_id=None,
                verdict=None,
                details={"question": question[:200], "session_id": session.session_id}
            ))

            # Start codex async
            with open(session.output_file, "w") as f:
                proc = subprocess.Popen(
                    ["/opt/homebrew/bin/codex", "exec", prompt],
                    cwd=PROJECT_ROOT,
                    stdout=f,
                    stderr=subprocess.STDOUT,
                    start_new_session=True
                )
                session.pid = proc.pid

            save_codex_session(session)
            return [TextContent(type="text", text=f"🚀 Codex session started: **{session.session_id}**\n\nUse `get_codex_result` with this session_id to retrieve the response.")]

        elif name == "get_codex_result":
            session_id = arguments["session_id"]
            session = load_codex_session(session_id)

            if not session:
                return [TextContent(type="text", text=f"❌ Session not found: {session_id}")]

            session = check_codex_session_status(session)

            if session.status == "running":
                return [TextContent(type="text", text=f"⏳ Session **{session_id}** is still running (started {session.started_at})")]

            if session.status == "failed":
                return [TextContent(type="text", text=f"❌ Session **{session_id}** failed - no output file found")]

            # Read output
            try:
                output = Path(session.output_file).read_text()
                return [TextContent(type="text", text=f"**Codex response** (session {session_id}):\n\n{output}")]
            except Exception as e:
                return [TextContent(type="text", text=f"❌ Error reading output: {e}")]

        elif name == "continue_codex":
            message = arguments["message"]
            files = arguments.get("files", [])
            resume_session_id = arguments.get("session_id")

            # Look up Codex session ID if our session_id provided
            codex_session_id = None
            if resume_session_id:
                prev_session = load_codex_session(resume_session_id)
                if prev_session and prev_session.codex_session_id:
                    codex_session_id = prev_session.codex_session_id
                elif prev_session:
                    return [TextContent(type="text", text=f"❌ Session {resume_session_id} has no Codex session ID stored. It may still be running or the ID wasn't parsed.")]
                else:
                    return [TextContent(type="text", text=f"❌ Session not found: {resume_session_id}")]

            prompt = format_codex_prompt(message, files)

            # Create new session for continuation
            new_session = CodexSession(
                session_id="",
                prompt=prompt,
                files=files,
                status="running",
                started_at=""
            )

            log_audit(AuditEntry(
                timestamp="",
                action="continue_codex",
                task_id=None,
                invariant_id=None,
                verdict=None,
                details={"message": message[:200], "session_id": new_session.session_id, "resuming": resume_session_id or "last"}
            ))

            # Build resume command
            if codex_session_id:
                # Resume specific session
                cmd = ["/opt/homebrew/bin/codex", "exec", "resume", codex_session_id, message]
            else:
                # Resume most recent session
                cmd = ["/opt/homebrew/bin/codex", "exec", "resume", "--last", message]

            with open(new_session.output_file, "w") as f:
                proc = subprocess.Popen(
                    cmd,
                    cwd=PROJECT_ROOT,
                    stdout=f,
                    stderr=subprocess.STDOUT,
                    start_new_session=True
                )
                new_session.pid = proc.pid

            save_codex_session(new_session)
            resuming_msg = f"session {resume_session_id}" if resume_session_id else "last session"
            return [TextContent(type="text", text=f"🚀 Codex continuation started (resuming {resuming_msg}): **{new_session.session_id}**\n\nUse `get_codex_result` with this session_id to retrieve the response.")]

        elif name == "list_codex_sessions":
            limit = arguments.get("limit", 10)
            sessions = load_all_codex_sessions()

            if not sessions:
                return [TextContent(type="text", text="No codex sessions found.")]

            # Sort by started_at descending and limit
            sorted_sessions = sorted(
                sessions.values(),
                key=lambda s: s.get("started_at", ""),
                reverse=True
            )[:limit]

            output = f"**Codex Sessions** (last {len(sorted_sessions)}):\n\n"
            for s in sorted_sessions:
                session = CodexSession(**s)
                session = check_codex_session_status(session)
                status_icon = {"running": "⏳", "completed": "✅", "failed": "❌"}.get(session.status, "❓")
                output += f"- {status_icon} **{session.session_id}** ({session.status}) - {session.started_at[:19]}\n"
                output += f"  Prompt: {session.prompt[:60]}...\n"

            return [TextContent(type="text", text=output)]

        elif name == "audit_with_codex":
            subject = arguments["subject"]
            content = arguments["content"]
            audit_type = arguments["audit_type"]
            files = arguments.get("files", [])

            audit_prompts = {
                "ontology": f"Review this for ontological consistency with Ergo's four-primitive model (Source, Compute, Trigger, Action):\n\nSubject: {subject}\n\n{content}\n\nDoes this respect the primitive boundaries? Any concerns?",
                "invariant": f"Review this invariant enforcement:\n\nSubject: {subject}\n\n{content}\n\nIs the enforcement complete? Any gaps or edge cases?",
                "design": f"Review this design decision for Ergo:\n\nSubject: {subject}\n\n{content}\n\nIs this consistent with Ergo's minimalist, deterministic philosophy? Concerns?",
                "implementation": f"Review this implementation:\n\nSubject: {subject}\n\n{content}\n\nDoes it match the spec? Any semantic drift?"
            }

            base_prompt = audit_prompts.get(audit_type, f"Review: {subject}\n\n{content}")
            prompt = format_codex_prompt(base_prompt, files)

            session = CodexSession(
                session_id="",
                prompt=prompt,
                files=files,
                status="running",
                started_at=""
            )

            log_audit(AuditEntry(
                timestamp="",
                action="audit_with_codex",
                task_id=None,
                invariant_id=None,
                verdict=None,
                details={"subject": subject, "audit_type": audit_type, "session_id": session.session_id}
            ))

            with open(session.output_file, "w") as f:
                proc = subprocess.Popen(
                    ["/opt/homebrew/bin/codex", "exec", prompt],
                    cwd=PROJECT_ROOT,
                    stdout=f,
                    stderr=subprocess.STDOUT,
                    start_new_session=True
                )
                session.pid = proc.pid

            save_codex_session(session)
            return [TextContent(type="text", text=f"🚀 Codex audit ({audit_type}) started: **{session.session_id}**\n\nUse `get_codex_result` with this session_id to retrieve the response.")]

        elif name == "verify_with_codex":
            claim = arguments["claim"]
            doctrine_refs = arguments.get("doctrine_refs", [])
            files = arguments.get("files", [])

            # Load referenced doctrine docs
            doctrine_content = ""
            for ref in doctrine_refs:
                doc_content = read_governance_doc(ref)
                if doc_content:
                    doctrine_content += f"\n\n--- {ref} ---\n{doc_content[:2000]}"

            base_prompt = f"""Verify this claim against Ergo's doctrine:

Claim: {claim}

{f"Relevant doctrine:{doctrine_content}" if doctrine_content else ""}

Is this claim consistent with the doctrine? Any conflicts or concerns?"""

            prompt = format_codex_prompt(base_prompt, files)

            session = CodexSession(
                session_id="",
                prompt=prompt,
                files=files,
                status="running",
                started_at=""
            )

            log_audit(AuditEntry(
                timestamp="",
                action="verify_with_codex",
                task_id=None,
                invariant_id=None,
                verdict=None,
                details={"claim": claim[:200], "refs": doctrine_refs, "session_id": session.session_id}
            ))

            with open(session.output_file, "w") as f:
                proc = subprocess.Popen(
                    ["/opt/homebrew/bin/codex", "exec", prompt],
                    cwd=PROJECT_ROOT,
                    stdout=f,
                    stderr=subprocess.STDOUT,
                    start_new_session=True
                )
                session.pid = proc.pid

            save_codex_session(session)
            return [TextContent(type="text", text=f"🚀 Codex verification started: **{session.session_id}**\n\nUse `get_codex_result` with this session_id to retrieve the response.")]

        else:
            return [TextContent(type="text", text=f"❌ Unknown tool: {name}")]
    
    except subprocess.TimeoutExpired:
        return [TextContent(type="text", text="❌ Command timed out")]
    except Exception as e:
        return [TextContent(type="text", text=f"❌ Error: {str(e)}")]


# =============================================================================
# MAIN
# =============================================================================

async def main():
    """Run the MCP server."""
    async with stdio_server() as (read_stream, write_stream):
        await server.run(read_stream, write_stream, server.create_initialization_options())


if __name__ == "__main__":
    import asyncio
    asyncio.run(main())
