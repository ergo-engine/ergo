#!/usr/bin/env bash
# verify_layer_boundaries.sh — Kernel/prod layer separation enforcement
#
# Purpose:  Automated guard for the five bleed-detection rules from
#           kernel-prod-separation.md §4.  Scans all workspace crates
#           for cross-layer violations that would break kernel/prod
#           ownership boundaries.
#
# Authority: Enforces LAYER-1 through LAYER-5 of kernel-prod-separation.md.
#            This script is a CI-level gate — failures block merges.
#
# Scope:    All crates under crates/kernel/ and crates/prod/.
#           Does not modify code.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

python3 - <<'PY'
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib  # type: ignore

root = Path.cwd()
violations = []

kernel_cargos = [
    root / "crates/kernel/runtime/Cargo.toml",
    root / "crates/kernel/adapter/Cargo.toml",
    root / "crates/kernel/supervisor/Cargo.toml",
]

prod_root = (root / "crates/prod").resolve()
shared_root = (root / "crates/shared").resolve()


def resolve_dep_path(cargo_path: Path, rel_path: str) -> Path:
    return (cargo_path.parent / rel_path).resolve()


def check_dep_table(cargo_path: Path, table_name: str, deps: dict):
    for dep_name, dep_spec in deps.items():
        if not isinstance(dep_spec, dict):
            continue
        dep_path = dep_spec.get("path")
        if dep_path is None:
            continue
        resolved = resolve_dep_path(cargo_path, dep_path)
        in_prod = prod_root in resolved.parents or resolved == prod_root
        in_shared = shared_root in resolved.parents or resolved == shared_root

        if table_name == "dependencies":
            if in_prod:
                violations.append(
                    f"{cargo_path}: [dependencies] '{dep_name}' points into prod/* ({dep_path})"
                )
            if in_shared:
                violations.append(
                    f"{cargo_path}: [dependencies] '{dep_name}' points into shared/* ({dep_path})"
                )
        elif table_name == "dev-dependencies":
            if in_prod:
                violations.append(
                    f"{cargo_path}: [dev-dependencies] '{dep_name}' points into prod/* ({dep_path})"
                )


for cargo_path in kernel_cargos:
    data = tomllib.loads(cargo_path.read_text())
    deps = data.get("dependencies", {})
    if isinstance(deps, dict):
        check_dep_table(cargo_path, "dependencies", deps)

    dev_deps = data.get("dev-dependencies", {})
    if isinstance(dev_deps, dict):
        check_dep_table(cargo_path, "dev-dependencies", dev_deps)

if violations:
    print("error: layer boundary violations detected:")
    for violation in violations:
        print(f"  - {violation}")
    sys.exit(1)

print("kernel dependency boundary check passed")
PY

if command -v rg >/dev/null 2>&1; then
  SEARCH_CMD=(rg -n)
else
  SEARCH_CMD=(grep -En)
fi

echo "checking prod RuleViolation boundary (LAYER-2 / bleed rule 1)"
# Rule 1 from kernel-prod-separation §4: "Prod introduces new semantic
# rule meanings, rule IDs, or RuleViolation ownership."
#
# The host legitimately wraps kernel RuleViolation into HostRuleViolation,
# so the host is exempt.  The loader and clients must not create, return,
# or re-expose raw kernel RuleViolation.
python3 - <<'PY'
import sys
from pathlib import Path
import re

# Loader: must not use RuleViolation at all
# Clients/SDK/shared: must not use raw kernel RuleViolation (HostRuleViolation is OK)
loader_dir = Path("crates/prod/core/loader/src")
client_dirs = [
    Path("crates/prod/clients/cli/src"),
    Path("crates/prod/clients/sdk-rust/src"),
    Path("crates/prod/clients/sdk-types/src"),
    Path("crates/prod/shared/duration/src"),
]

comment_pattern = re.compile(r"^\s*(//|///|//!)")
raw_rule_violation = re.compile(r"\bRuleViolation\b")
host_rule_violation = re.compile(r"\bHostRuleViolation\b")
violations = []

# Loader: no RuleViolation at all
if loader_dir.is_dir():
    for rs_file in sorted(loader_dir.rglob("*.rs")):
        for lineno, line in enumerate(rs_file.read_text().splitlines(), start=1):
            if comment_pattern.match(line):
                continue
            if raw_rule_violation.search(line):
                violations.append(f"{rs_file}:{lineno}:{line.strip()}")

# Clients: no raw kernel RuleViolation (HostRuleViolation wrapper is OK).
# Strip the allowed wrapper first so that a line containing both
# HostRuleViolation and a raw RuleViolation is still caught.
for dir_path in client_dirs:
    if not dir_path.is_dir():
        continue
    for rs_file in sorted(dir_path.rglob("*.rs")):
        for lineno, line in enumerate(rs_file.read_text().splitlines(), start=1):
            if comment_pattern.match(line):
                continue
            stripped_line = host_rule_violation.sub("", line)
            if raw_rule_violation.search(stripped_line):
                violations.append(f"{rs_file}:{lineno}:{line.strip()}")

if violations:
    for v in violations:
        print(v)
    print("error: loader/clients must not create or re-expose kernel RuleViolation (kernel-prod-separation §4 rule 1)")
    sys.exit(1)
PY

echo "checking clients against parser-internal imports"
if "${SEARCH_CMD[@]}" "decode::yaml_graph::|decode::json_graph::|selector_matches_version|RawClusterDefinition" \
  crates/prod/clients/cli/src crates/prod/clients/sdk-rust/src crates/prod/clients/sdk-types/src; then
  echo "error: clients must not import parser internals"
  exit 1
fi

echo "checking canonical CLI files for removed --direct path"
if "${SEARCH_CMD[@]}" -- "--direct" \
  crates/prod/clients/cli/src/main.rs \
  crates/prod/clients/cli/src/cli/args.rs \
  crates/prod/clients/cli/src/cli/dispatch.rs \
  crates/prod/clients/cli/src/output/text.rs; then
  echo "error: --direct must not appear in CLI command contract files"
  exit 1
fi

echo "checking CLI print surface"
if command -v rg >/dev/null 2>&1; then
  if rg -n "print!\\(|println!\\(|eprintln!\\(" crates/prod/clients/cli/src \
    --glob '!crates/prod/clients/cli/src/output/*' \
    --glob '!crates/prod/clients/cli/src/init_project.rs'; then
    echo "error: CLI printing must be centralized in output/*"
    exit 1
  fi
else
  if find crates/prod/clients/cli/src -type f -name '*.rs' ! -path '*/output/*' ! -path '*/init_project.rs' -print0 \
    | xargs -0 grep -En "print!\\(|println!\\(|eprintln!\\("; then
    echo "error: CLI printing must be centralized in output/*"
    exit 1
  fi
fi

echo "checking CLI runtime import boundaries"
CLI_RUNTIME_FILES=(
  crates/prod/clients/cli/src/cli/args.rs
  crates/prod/clients/cli/src/cli/dispatch.rs
  crates/prod/clients/cli/src/cli/handlers.rs
  crates/prod/clients/cli/src/error_format.rs
  crates/prod/clients/cli/src/gen_docs.rs
  crates/prod/clients/cli/src/graph_to_dot.rs
  crates/prod/clients/cli/src/graph_yaml.rs
  crates/prod/clients/cli/src/output/errors.rs
  crates/prod/clients/cli/src/render.rs
  crates/prod/clients/cli/src/validate.rs
)
python3 - <<'PY'
import re
import sys
from pathlib import Path

files = [
    "crates/prod/clients/cli/src/cli/args.rs",
    "crates/prod/clients/cli/src/cli/dispatch.rs",
    "crates/prod/clients/cli/src/cli/handlers.rs",
    "crates/prod/clients/cli/src/error_format.rs",
    "crates/prod/clients/cli/src/gen_docs.rs",
    "crates/prod/clients/cli/src/graph_to_dot.rs",
    "crates/prod/clients/cli/src/graph_yaml.rs",
    "crates/prod/clients/cli/src/output/errors.rs",
    "crates/prod/clients/cli/src/render.rs",
    "crates/prod/clients/cli/src/validate.rs",
]
pattern = re.compile(r"ergo_(runtime|adapter|supervisor)::")
test_mod_pattern = re.compile(r"(pub\s+)?mod tests\b")
# Regex to strip string literals and comments before counting braces.
# This prevents false positives from braces inside strings or comments.
strip_noise = re.compile(r'"(?:[^"\\]|\\.)*"|//.*$')

def strip_cfg_test_modules(text: str) -> str:
    """Remove #[cfg(test)] mod tests { ... } blocks from source text.

    Uses brace-depth tracking on content after stripping string literals
    and line comments to avoid miscounting braces in strings.
    Also handles the declaration-only form #[cfg(test)] mod tests; (no body).
    """
    lines = text.splitlines()
    kept = []
    i = 0
    while i < len(lines):
        if lines[i].strip() == "#[cfg(test)]":
            j = i + 1
            while j < len(lines) and not lines[j].strip():
                j += 1
            if j < len(lines) and test_mod_pattern.match(lines[j].strip()):
                # Declaration-only: #[cfg(test)] mod tests;
                if lines[j].strip().endswith(";"):
                    i = j + 1
                    continue
                # Inline block: #[cfg(test)] mod tests { ... }
                cleaned = strip_noise.sub("", lines[j])
                brace_depth = cleaned.count("{") - cleaned.count("}")
                i = j + 1
                while i < len(lines) and brace_depth > 0:
                    cleaned = strip_noise.sub("", lines[i])
                    brace_depth += cleaned.count("{") - cleaned.count("}")
                    i += 1
                continue
        kept.append(lines[i])
        i += 1
    return "\n".join(kept)

violations = []
for file_path in files:
    stripped = strip_cfg_test_modules(Path(file_path).read_text())
    for lineno, line in enumerate(stripped.splitlines(), start=1):
        if pattern.search(line):
            violations.append(f"{file_path}:{lineno}:{line}")

if violations:
    for violation in violations:
        print(violation)
    print("error: CLI runtime modules must not reference ergo_runtime, ergo_adapter, or ergo_supervisor")
    sys.exit(1)
PY

echo "checking canonical run/replay orchestration boundaries in CLI"
if "${SEARCH_CMD[@]}" "RuntimeHandle::new|compile_event_binder|HostedRunner::new|adapter_fingerprint|validate_adapter_composition|scan_adapter_dependencies\\(" \
  crates/prod/clients/cli/src/graph_yaml.rs; then
  echo "error: canonical run orchestration belongs in host, not graph_yaml.rs"
  exit 1
fi
if "${SEARCH_CMD[@]}" "prepare_graph_runtime\\(|ReplayGraphRequest\\{|parse_adapter_manifest\\(|compile_event_binder\\(|validate_adapter_composition\\(" \
  crates/prod/clients/cli/src/cli/handlers.rs; then
  echo "error: canonical replay orchestration belongs in host, not cli/handlers.rs"
  exit 1
fi

echo "checking SDK run/validation orchestration boundaries"
python3 - <<'PY'
import re
import sys
from pathlib import Path

root = Path("crates/prod/clients/sdk-rust/src")
patterns = [
    re.compile(r"RuntimeHandle::new"),
    re.compile(r"compile_event_binder"),
    re.compile(r"HostedRunner::new"),
    re.compile(r"adapter_fingerprint"),
    re.compile(r"validate_adapter_composition"),
    re.compile(r"scan_adapter_dependencies\("),
    re.compile(r"parse_graph_file"),
    re.compile(r"discover_cluster_tree"),
    re.compile(r"\bexpand\("),
    re.compile(r"validate_adapter\("),
    re.compile(r"validate_egress_config\("),
    re.compile(r"ensure_handler_coverage\("),
]
cfg_test_pattern = re.compile(r"#\[\s*cfg\s*\((?=[^\]]*\btest\b)[^\]]*\)\s*\]")

def strip_cfg_test_items(text: str) -> str:
    lines = text.splitlines()
    kept = []
    i = 0
    while i < len(lines):
        if cfg_test_pattern.match(lines[i].strip()):
            j = i + 1
            while j < len(lines) and (
                not lines[j].strip() or lines[j].lstrip().startswith("#[")
            ):
                j += 1
            if j >= len(lines):
                break

            i = j
            brace_depth = 0
            while i < len(lines):
                brace_depth += lines[i].count("{") - lines[i].count("}")
                line = lines[i]
                i += 1
                if brace_depth > 0:
                    while i < len(lines) and brace_depth > 0:
                        brace_depth += lines[i].count("{") - lines[i].count("}")
                        i += 1
                    break
                if ";" in line:
                    break
            continue
        kept.append(lines[i])
        i += 1
    return "\n".join(kept)

violations = []
for file_path in sorted(root.rglob("*.rs")):
    stripped = strip_cfg_test_items(file_path.read_text())
    for lineno, line in enumerate(stripped.splitlines(), start=1):
        if any(pattern.search(line) for pattern in patterns):
            violations.append(f"{file_path}:{lineno}:{line}")

if violations:
    for violation in violations:
        print(violation)
    print("error: SDK run/validation orchestration must delegate to host-owned orchestration")
    sys.exit(1)
PY

echo "checking LAYER-4: prod must not reinterpret primitive ontology (bleed rule 4)"
python3 - <<'PY'
import sys
from pathlib import Path
import re

prod_dirs = [
    "crates/prod/core/host/src",
    "crates/prod/core/loader/src",
    "crates/prod/clients/cli/src",
    "crates/prod/clients/sdk-rust/src",
    "crates/prod/clients/sdk-types/src",
]
comment_pattern = re.compile(r"^\s*(//|///|//!)")
ontology_patterns = [
    re.compile(r"\benum\s+PrimitiveKind\b"),
    re.compile(r"\bfn\s+wiring_legality\b"),
    re.compile(r"\bfn\s+slot_type_check\b"),
    re.compile(r"\bfn\s+validate_slot_compatibility\b"),
]
violations = []
for dir_path in prod_dirs:
    d = Path(dir_path)
    if not d.is_dir():
        continue
    for rs_file in sorted(d.rglob("*.rs")):
        for lineno, line in enumerate(rs_file.read_text().splitlines(), start=1):
            if comment_pattern.match(line):
                continue
            for pat in ontology_patterns:
                if pat.search(line):
                    violations.append(f"{rs_file}:{lineno}:{line.strip()}")

if violations:
    for v in violations:
        print(v)
    print("error: prod crates must not reinterpret primitive ontology (kernel-prod-separation §4 rule 4)")
    sys.exit(1)
PY

echo "checking LAYER-5: loader must not perform kernel semantic enforcement (bleed rule 5)"
python3 - <<'PY'
import sys
from pathlib import Path
import re

comment_pattern = re.compile(r"^\s*(//|///|//!)")
semantic_patterns = [
    re.compile(r"\bfn\s+validate_graph_semantics\b"),
    re.compile(r"\bfn\s+expand\("),
    re.compile(r"\bfn\s+schedule_execution\b"),
    re.compile(r"\bfn\s+validate_wiring\b"),
    re.compile(r"\bRuleViolation\b"),
]
violations = []
d = Path("crates/prod/core/loader/src")
if d.is_dir():
    for rs_file in sorted(d.rglob("*.rs")):
        for lineno, line in enumerate(rs_file.read_text().splitlines(), start=1):
            if comment_pattern.match(line):
                continue
            for pat in semantic_patterns:
                if pat.search(line):
                    violations.append(f"{rs_file}:{lineno}:{line.strip()}")

if violations:
    for v in violations:
        print(v)
    print("error: loader must not perform semantic enforcement belonging in kernel (kernel-prod-separation §4 rule 5)")
    sys.exit(1)
PY

echo "layer boundary checks passed"
