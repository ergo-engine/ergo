#!/usr/bin/env bash
# verify_code_map_drift.sh — Kernel and prod CODE_MAP drift guard
#
# Purpose:  Resolves every path-and-symbol citation in
#           crates/kernel/CODE_MAP.md and crates/prod/CODE_MAP.md
#           against the current source, and re-checks the structural
#           claims those docs make (trait bounds, sealed markers,
#           counts, boundary invariants).
#
# Authority: Informational — detects when the kernel/prod Code Maps
#            have drifted from the current code shape.  Fail when a
#            citation no longer resolves or a structural claim breaks.
#
# Scope:    crates/kernel/CODE_MAP.md, crates/prod/CODE_MAP.md, and
#           the source files they cite.  Does not modify code.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

fail=0
total=0

check() {
  local path="$1"; local sym="$2"
  total=$((total+1))
  if [[ ! -f "$path" ]]; then
    echo "MISSING FILE: $path"; fail=1; return
  fi
  if ! grep -qE "(fn|struct|enum|trait|const|impl|mod|use|type) +(<[^>]+> +)?${sym}\\b" "$path"; then
    echo "MISSING: $path :: $sym"; fail=1
  fi
}

# =====================================================================
# Kernel CODE_MAP citations
# =====================================================================

# §1 — stdlib catalog entry points
check crates/kernel/runtime/src/catalog.rs build_core
check crates/kernel/runtime/src/catalog.rs CatalogBuilder
check crates/kernel/runtime/src/catalog.rs build_core_catalog
check crates/kernel/runtime/src/catalog.rs core_registries
check crates/kernel/runtime/src/catalog.rs CoreRegistrationError
check crates/kernel/runtime/src/catalog.rs CorePrimitiveCatalog
check crates/kernel/runtime/src/catalog.rs CoreRegistries

# §2 — graph pipeline
check crates/kernel/runtime/src/cluster.rs ClusterDefinition
check crates/kernel/runtime/src/cluster.rs ExpandedGraph
check crates/kernel/runtime/src/cluster.rs expand
check crates/kernel/runtime/src/cluster.rs PrimitiveCatalog
check crates/kernel/runtime/src/runtime/types.rs ValidatedGraph
check crates/kernel/runtime/src/runtime/types.rs ExecutionReport
check crates/kernel/runtime/src/runtime/types.rs Registries
check crates/kernel/runtime/src/runtime/types.rs ExecutionContext
check crates/kernel/runtime/src/runtime/validate.rs validate
check crates/kernel/runtime/src/runtime/execute.rs execute
check crates/kernel/runtime/src/runtime/execute.rs execute_with_metadata
check crates/kernel/runtime/src/runtime/execute.rs should_skip_action
check crates/kernel/runtime/src/runtime/mod.rs run
check crates/kernel/runtime/src/common/intent_id.rs derive_intent_id

# §3 — adapter entry points
check crates/kernel/adapter/src/lib.rs RuntimeInvoker
check crates/kernel/adapter/src/lib.rs RuntimeHandle
check crates/kernel/adapter/src/lib.rs ReportingRuntimeHandle
check crates/kernel/adapter/src/lib.rs FaultRuntimeHandle
check crates/kernel/adapter/src/lib.rs RuntimeState
check crates/kernel/adapter/src/lib.rs ExecutionContext

# §5 — capture and replay
check crates/kernel/supervisor/src/capture.rs CapturingDecisionLog
check crates/kernel/supervisor/src/capture.rs CapturingSession
check crates/kernel/supervisor/src/capture.rs write_capture_bundle
check crates/kernel/adapter/src/provenance.rs fingerprint
check crates/kernel/runtime/src/provenance.rs compute_runtime_provenance
check crates/kernel/supervisor/src/lib.rs CaptureBundle
check crates/kernel/supervisor/src/lib.rs CAPTURE_FORMAT_VERSION
check crates/kernel/supervisor/src/replay.rs validate_bundle
check crates/kernel/supervisor/src/replay.rs validate_bundle_strict
check crates/kernel/supervisor/src/replay.rs replay
check crates/kernel/supervisor/src/replay.rs replay_checked
check crates/kernel/supervisor/src/replay.rs replay_checked_strict
check crates/kernel/supervisor/src/replay.rs compare_decisions
check crates/kernel/supervisor/src/replay.rs hash_effect
check crates/kernel/supervisor/src/replay.rs ReplayError
check crates/kernel/supervisor/src/replay.rs MemoryDecisionLog
check crates/kernel/supervisor/src/lib.rs Supervisor
check crates/kernel/supervisor/src/lib.rs EpisodeId
check crates/kernel/supervisor/src/lib.rs DecisionLog

# §6 — primitive trait families
check crates/kernel/runtime/src/source/mod.rs SourcePrimitive
check crates/kernel/runtime/src/compute/mod.rs ComputePrimitive
check crates/kernel/runtime/src/trigger/mod.rs TriggerPrimitive
check crates/kernel/runtime/src/action/mod.rs ActionPrimitive

# §7 — enforcement primitives
check crates/kernel/adapter/src/validate.rs validate_adapter
check crates/kernel/adapter/src/registry.rs register
check crates/kernel/adapter/src/composition.rs validate_source_adapter_composition
check crates/kernel/adapter/src/composition.rs validate_action_adapter_composition
check crates/kernel/adapter/src/composition.rs validate_capture_format
check crates/kernel/adapter/src/capture.rs ExternalEventRecord

# §9 — primitive state
check crates/kernel/runtime/src/compute/mod.rs PrimitiveState

# Kernel structural drift checks ---------------------------------------

# §1 — stdlib counts
expected_source=7; expected_compute=27; expected_trigger=2; expected_action=6
for kind in source compute trigger action; do
  dir="crates/kernel/runtime/src/$kind/implementations"
  if [[ -d "$dir" ]]; then
    actual=$(ls "$dir" | grep -v '^mod\.rs$' | grep -v '^tests' | wc -l | tr -d ' ')
    eval "expected=\$expected_$kind"
    if [[ "$actual" != "$expected" ]]; then
      echo "STDLIB COUNT DRIFT: $kind expected=$expected actual=$actual (update kernel CODE_MAP §1)"
      fail=1
    fi
  fi
done

# §8 — trait bound asymmetry: only TriggerPrimitive declares Send + Sync today
if ! grep -qE "^pub trait TriggerPrimitive: Send \+ Sync" crates/kernel/runtime/src/trigger/mod.rs; then
  echo "TRAIT BOUND DRIFT: TriggerPrimitive no longer declares Send + Sync (update kernel CODE_MAP §8)"
  fail=1
fi
for t in Source Compute Action; do
  f="crates/kernel/runtime/src/$(echo $t | tr A-Z a-z)/mod.rs"
  if grep -qE "^pub trait ${t}Primitive: Send \+ Sync" "$f"; then
    echo "TRAIT BOUND DRIFT: ${t}Primitive now declares Send + Sync (update kernel CODE_MAP §8)"
    fail=1
  fi
done

# §8 — suppression site count (expected 9 occurrences across 7 files)
allow_count=$(grep -rn "arc_with_non_send_sync" --include="*.rs" crates/ | wc -l | tr -d ' ')
if [[ "$allow_count" != "9" ]]; then
  echo "ALLOW SITE COUNT DRIFT: arc_with_non_send_sync expected=9 actual=$allow_count (update kernel CODE_MAP §8)"
  fail=1
fi

# §9 — runtime crate must remain free of concurrency primitives
if grep -qrE "spawn|Mutex|RwLock|Atomic|thread::|tokio::|once_cell|OnceLock|OnceCell|lazy_static" \
    --include="*.rs" crates/kernel/runtime/src/; then
  echo "CONCURRENCY DRIFT: runtime crate now contains a concurrency primitive (update kernel CODE_MAP §9)"
  fail=1
fi

# §9 — PrimitiveState reserved-but-unused: executor must pass None,
# nothing in the kernel may allocate one
if ! grep -qE "\.compute\(.*None\)" crates/kernel/runtime/src/runtime/execute.rs; then
  echo "STATELESSNESS DRIFT: executor compute call no longer passes None for PrimitiveState (update kernel CODE_MAP §9)"
  fail=1
fi
if grep -rnE "PrimitiveState\s*\{|PrimitiveState::default|PrimitiveState::new" \
    --include="*.rs" crates/kernel/ | grep -v "pub struct PrimitiveState" >/dev/null; then
  echo "STATELESSNESS DRIFT: kernel now allocates a PrimitiveState (update kernel CODE_MAP §9)"
  fail=1
fi

# §9 — manifest-level statelessness enforcement rule IDs must still exist
for rule in SRC-8 CMP-9 TRG-9 ACT-10; do
  if ! grep -rqE "\"$rule\"" --include="*.rs" crates/kernel/runtime/src/; then
    echo "STATELESSNESS DRIFT: rule ID $rule no longer present in kernel (update kernel CODE_MAP §9)"
    fail=1
  fi
done

# §9 — the rejected structural-enforcement decision doc must still exist
if [[ ! -f docs/ledger/decisions/rejected-structural-enforcement-of-statelessness.md ]]; then
  echo "DOCTRINE DRIFT: rejected-structural-enforcement-of-statelessness.md missing (update kernel CODE_MAP §9)"
  fail=1
fi

# =====================================================================
# Prod CODE_MAP citations
# =====================================================================

# §1 — six prod crates
prod_crates=( \
  "crates/prod/core/loader" \
  "crates/prod/core/host" \
  "crates/prod/clients/sdk-rust" \
  "crates/prod/clients/cli" \
  "crates/prod/clients/sdk-types" \
  "crates/prod/shared/duration" \
)
for c in "${prod_crates[@]}"; do
  if [[ ! -f "$c/Cargo.toml" ]]; then
    echo "MISSING PROD CRATE: $c (update prod CODE_MAP §1)"; fail=1
  fi
done

# §2 — 9-layer call stack
check crates/prod/clients/sdk-rust/src/lib.rs Ergo
check crates/prod/clients/sdk-rust/src/lib.rs ProfileRunner
check crates/prod/core/host/src/usecases/live_run.rs run_graph_from_paths_with_surfaces_and_control
check crates/prod/core/host/src/runner.rs HostedRunner
check crates/prod/core/host/src/runner.rs step
check crates/prod/core/host/src/runner.rs execute_step
check crates/kernel/supervisor/src/capture.rs on_event
check crates/prod/core/host/src/host/buffering_invoker.rs BufferingRuntimeInvoker
check crates/prod/core/host/src/host/buffering_invoker.rs run
check crates/kernel/adapter/src/lib.rs run_reporting
check crates/kernel/adapter/src/lib.rs execute_once

# §3 — surface tiers
check crates/prod/core/host/src/usecases.rs RuntimeSurfaces
check crates/prod/core/host/src/usecases/live_prep.rs prepare_hosted_runner_from_paths_with_surfaces
check crates/prod/core/host/src/usecases/live_prep.rs prepare_hosted_runner_with_surfaces
check crates/prod/core/host/src/usecases/live_run.rs run_graph_from_assets_with_surfaces

# §4 — sealed / open
check crates/prod/core/loader/src/io.rs PreparedGraphAssets
check crates/prod/core/loader/src/io.rs FilesystemGraphBundle
check crates/prod/core/loader/src/io.rs InMemoryGraphBundle
check crates/prod/core/host/src/runner.rs new_validated

# §5 — dual gate
check crates/prod/core/host/src/usecases.rs SessionIntent
check crates/prod/core/host/src/usecases.rs PrepareHostedRunnerFromPathsRequest
check crates/prod/core/host/src/usecases.rs LivePrepOptions
check crates/prod/core/host/src/usecases/live_prep.rs ensure_production_adapter_bound
check crates/prod/core/host/src/usecases/live_prep.rs session_intent_from_driver
check crates/prod/core/host/src/usecases/live_prep.rs ensure_adapter_requirement_satisfied

# §6 — handler coverage
check crates/prod/core/host/src/host/effects.rs EffectHandler
check crates/prod/core/host/src/host/effects.rs SetContextHandler
check crates/prod/core/host/src/host/effects.rs EffectApplyError
check crates/prod/core/host/src/host/effects.rs AppliedWrite
check crates/prod/core/host/src/host/coverage.rs ensure_handler_coverage
check crates/prod/core/host/src/host/coverage.rs HandlerCoverageError
check crates/prod/core/host/src/host/context_store.rs ContextStore

# §7 — finalization state
check crates/prod/core/host/src/runner.rs CaptureFinalizationState
check crates/prod/core/host/src/runner.rs ensure_capture_finalizable
check crates/prod/core/host/src/runner.rs into_capture_bundle

# §8 — vocabulary
check crates/kernel/supervisor/src/lib.rs NO_ADAPTER_PROVENANCE

# §10 — kernel boundary
check crates/kernel/supervisor/src/capture.rs write_capture_bundle

# Prod structural drift checks ----------------------------------------

# §5 — at least 6 call sites of ensure_production_adapter_bound today
prod_gate_sites=$(grep -nE "ensure_production_adapter_bound" \
    crates/prod/core/host/src/usecases/live_run.rs \
    crates/prod/core/host/src/usecases/live_prep.rs | wc -l | tr -d ' ')
if [[ "$prod_gate_sites" -lt 6 ]]; then
  echo "DUAL-GATE CALL SITE DRIFT: expected >= 6 sites, got $prod_gate_sites (update prod CODE_MAP §5)"
  fail=1
fi

# §6 — EffectHandler bound is host-owned
if ! grep -qE "^pub trait EffectHandler: Send \+ Sync" crates/prod/core/host/src/host/effects.rs; then
  echo "TRAIT BOUND DRIFT: EffectHandler no longer requires Send + Sync (update prod CODE_MAP §6)"
  fail=1
fi

# §6 — StopHandle Send + Sync asserted at compile time
if ! grep -qE "assert_send_sync::<StopHandle>" crates/prod/clients/sdk-rust/src/lib.rs; then
  echo "STRUCTURAL DRIFT: StopHandle compile-time Send + Sync assertion missing (update prod CODE_MAP §6)"
  fail=1
fi

# §6 — exactly one built-in EffectHandler implementor in prod/core/host
prod_handler_count=$(grep -rE "^impl EffectHandler for " --include="*.rs" crates/prod/core/host/src/ | wc -l | tr -d ' ')
if [[ "$prod_handler_count" != "1" ]]; then
  echo "BUILT-IN HANDLER COUNT DRIFT: expected 1 impl EffectHandler, got $prod_handler_count (update prod CODE_MAP §6)"
  fail=1
fi

# §4 — PreparedGraphAssets sealed marker
if ! grep -qE "pub\(crate\) _sealed: \(\)" crates/prod/core/loader/src/io.rs; then
  echo "SEAL MARKER DRIFT: PreparedGraphAssets no longer carries pub(crate) _sealed: () (update prod CODE_MAP §4)"
  fail=1
fi

# §5 — SessionIntent default must remain Production
if ! grep -qE "Self::Production" crates/prod/core/host/src/usecases.rs; then
  echo "SAFETY DRIFT: SessionIntent::default() may no longer return Production (update prod CODE_MAP §5)"
  fail=1
fi

# §9 — RuntimeInvoker::run must not carry effects in its signature
if grep -A6 "pub trait RuntimeInvoker" crates/kernel/adapter/src/lib.rs | grep -qE "effects_out|Vec<ActionEffect>"; then
  echo "BOUNDARY DRIFT: RuntimeInvoker::run now references effects (update prod CODE_MAP §9 / kernel CODE_MAP §3)"
  fail=1
fi

# §1 — prod tree must still have exactly six crates
prod_crate_count=$(find crates/prod -name Cargo.toml -not -path "*/target/*" | wc -l | tr -d ' ')
if [[ "$prod_crate_count" != "6" ]]; then
  echo "PROD CRATE COUNT DRIFT: expected 6, got $prod_crate_count (update prod CODE_MAP §1)"
  fail=1
fi

# §7 — four CaptureFinalizationState variants
for v in NoCommittedSteps Eligible FinalizeOnly Fatal; do
  if ! grep -qE "\b$v\b" crates/prod/core/host/src/runner.rs; then
    echo "FINALIZATION STATE DRIFT: variant $v not found (update prod CODE_MAP §7)"
    fail=1
  fi
done

# =====================================================================
# Result
# =====================================================================
if [[ $fail -eq 0 ]]; then
  echo "verify_code_map_drift: ALL $total CITATIONS + STRUCTURAL CHECKS PASS"
  exit 0
else
  echo "verify_code_map_drift: drift detected (see messages above)"
  exit 1
fi
