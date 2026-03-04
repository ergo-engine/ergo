#!/usr/bin/env bash
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

echo "checking loader RuleViolation boundary"
if "${SEARCH_CMD[@]}" "RuleViolation" crates/prod/core/loader/src; then
  echo "error: loader must not expose or return RuleViolation"
  exit 1
fi

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
  if rg -n "print!\\(|println!\\(|eprintln!\\(" crates/prod/clients/cli/src --glob '!crates/prod/clients/cli/src/output/*'; then
    echo "error: CLI printing must be centralized in output/*"
    exit 1
  fi
else
  if find crates/prod/clients/cli/src -type f -name '*.rs' ! -path '*/output/*' -print0 \
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
if "${SEARCH_CMD[@]}" "ergo_(runtime|adapter|supervisor)::" "${CLI_RUNTIME_FILES[@]}"; then
  echo "error: CLI runtime modules must not reference ergo_runtime, ergo_adapter, or ergo_supervisor"
  exit 1
fi

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

echo "layer boundary checks passed"
