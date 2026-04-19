//! runner
//!
//! Purpose:
//! - Implement the core hosted-runner step engine that turns `HostedEvent` or replayed
//!   `ExternalEvent` inputs into supervisor decisions, host-owned effect application, optional
//!   egress dispatch, and capture finalization truth.
//!
//! Owns:
//! - `HostedRunner`, `HostedEvent`, `HostedStepOutcome`, and `HostedAdapterConfig`.
//! - Hosted-runner configuration validation, effect ownership routing, and the
//!   `CaptureFinalizationState` gate for step/finalize legality.
//! - Host-local decision logging and capture enrichment inputs for applied effects, intent acks,
//!   and interruptions.
//!
//! Does not own:
//! - Loader/prep orchestration, adapter manifest parsing, or canonical run request shaping, which
//!   live in `usecases/live_prep.rs` and `usecases/live_run.rs`.
//! - Ingress process protocol behavior, which lives in `usecases/process_driver.rs`.
//! - Replay comparison semantics, which live in `replay.rs` and `ergo_supervisor`.
//! - `set_context` key/writable/type enforcement, which belongs to the host
//!   effect layer.
//!
//! Connects to:
//! - `capture_enrichment.rs` for persisted host capture sidecars.
//! - `egress/process.rs` through `EgressRuntime` for live external intent dispatch.
//! - CLI and SDK manual-runner surfaces via `lib.rs` re-exports of `HostedRunner` and
//!   `HostedEvent`.
//!
//! Safety notes:
//! - `HostDecisionLog` propagates mutex poison via `expect(...)`; a panic while the lock is held is
//!   treated as unrecoverable for the runner lifetime.
//! - `CaptureFinalizationState` is load-bearing: `FinalizeOnly` permits capture finalization but
//!   blocks further stepping, while `Fatal` blocks both. This is the runner-owned
//!   gate in the capture finalization pipeline:
//!     runner.rs: `CaptureFinalizationState` (gate), `ensure_capture_finalizable()`,
//!       `into_capture_bundle()` (extraction)
//!     live_prep.rs: `HostedRunnerFinalizeFailure` (staged error),
//!       `finalize_hosted_runner_capture_with_stage()` (3-step orchestration)
//!     live_run.rs: `FinalizedRunCapture` (summary DTO),
//!       `finalize_run_capture()` (driver-level validation)
//! - `HostedEvent` intentionally stays in this file as the public wire DTO
//!   consumed by CLI/SDK callers. The typing boundary is the private
//!   `ValidatedHostedEvent` phase, not the wire shape. `semantic_kind` remains
//!   `Option<String>` for wire compatibility; validation of semantic-kind
//!   coherence happens during adapter binding, not at the DTO level.
//! - `host_internal_handler_kinds()` is the host-owned authority for effect
//!   kinds that stay inside the runner/handler path rather than replay-owned
//!   external effects.
//! - `validate_hosted_runner_configuration()` is a pure validation function
//!   that returns warnings as data. `HostedRunner::new()` emits warnings to
//!   stderr to preserve behavior for direct callers; orchestration callers
//!   (e.g. `live_prep.rs`) handle warnings at their own layer.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::{Arc, Mutex};

use ergo_adapter::{
    bind_semantic_event_with_binder, compile_event_binder, AdapterProvides, EventId, EventTime,
    ExternalEvent, ExternalEventKind, GraphId, RunTermination,
};
use ergo_runtime::common::ActionEffect;
use ergo_supervisor::{
    CaptureBundle, CapturedIntentAck, CapturingSession, Constraints, Decision, DecisionLog,
    DecisionLogEntry, NO_ADAPTER_PROVENANCE,
};
use serde::{Deserialize, Serialize};

use crate::capture_enrichment::{
    enrich_bundle_with_host_artifacts, AppliedEffectsByDecision, AppliedIntentAcksByDecision,
    StepInterruptionsByDecision,
};
use crate::diagnostics::emit_warnings_to_stderr;
use crate::egress::{
    validate_egress_config, EgressConfig, EgressProcessError, EgressRuntime,
    EgressValidationWarning,
};
use crate::error::{
    EgressDispatchFailure, HostedEgressValidationError, HostedEventBuildError, HostedStepError,
};
use crate::host::{
    ensure_handler_coverage, AppliedWrite, BufferingRuntimeInvoker, ContextStore, EffectApplyError,
    EffectHandler, SetContextHandler,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostedEvent {
    pub event_id: String,
    pub kind: ExternalEventKind,
    pub at: EventTime,
    pub semantic_kind: Option<String>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct HostedStepOutcome {
    pub decision: Decision,
    pub termination: Option<RunTermination>,
    pub retry_count: usize,
    pub effects: Vec<ActionEffect>,
    pub applied_writes: Vec<AppliedWrite>,
}

#[derive(Debug)]
pub struct HostedAdapterConfig {
    provides: AdapterProvides,
    binder: ergo_adapter::EventBinder,
    adapter_provenance: String,
}

impl HostedAdapterConfig {
    pub fn new(
        provides: AdapterProvides,
        adapter_provenance: impl Into<String>,
    ) -> Result<Self, ergo_adapter::EventBindingError> {
        let binder = compile_event_binder(&provides)?;
        Ok(Self {
            provides,
            binder,
            adapter_provenance: adapter_provenance.into(),
        })
    }

    pub fn provides(&self) -> &AdapterProvides {
        &self.provides
    }

    pub fn adapter_provenance(&self) -> &str {
        &self.adapter_provenance
    }
}

#[derive(Clone, Default)]
struct HostDecisionLog {
    entries: Arc<Mutex<Vec<DecisionLogEntry>>>,
}

impl DecisionLog for HostDecisionLog {
    fn log(&self, entry: DecisionLogEntry) {
        let mut guard = self.entries.lock().expect("host decision log poisoned");
        guard.push(entry);
    }
}

impl HostDecisionLog {
    fn len(&self) -> usize {
        let guard = self.entries.lock().expect("host decision log poisoned");
        guard.len()
    }

    fn get(&self, index: usize) -> Option<DecisionLogEntry> {
        let guard = self.entries.lock().expect("host decision log poisoned");
        guard.get(index).cloned()
    }
}

struct AdapterMode {
    provides: AdapterProvides,
    binder: ergo_adapter::EventBinder,
}

struct ValidatedHostedEvent {
    event_id: EventId,
    kind: ExternalEventKind,
    at: EventTime,
    semantic_kind: Option<String>,
    payload: Option<serde_json::Map<String, serde_json::Value>>,
}

impl ValidatedHostedEvent {
    fn new(event: HostedEvent) -> Result<Self, HostedStepError> {
        let payload = match event.payload {
            Some(payload) => Some(
                payload
                    .as_object()
                    .cloned()
                    .ok_or(HostedStepError::PayloadMustBeObject)?,
            ),
            None => None,
        };

        Ok(Self {
            event_id: EventId::new(event.event_id),
            kind: event.kind,
            at: event.at,
            semantic_kind: event.semantic_kind,
            payload,
        })
    }

    fn semantic_kind(&self) -> Result<&str, HostedStepError> {
        self.semantic_kind
            .as_deref()
            .ok_or(HostedStepError::MissingSemanticKind)
    }

    fn payload_object(
        &self,
    ) -> Result<&serde_json::Map<String, serde_json::Value>, HostedStepError> {
        self.payload.as_ref().ok_or(HostedStepError::MissingPayload)
    }

    fn into_external_event(self) -> Result<ExternalEvent, HostedStepError> {
        match self.payload {
            Some(payload) => {
                let bytes = serde_json::to_vec(&payload).map_err(|err| {
                    HostedStepError::EventBuild(HostedEventBuildError::SerializePayload(err))
                })?;
                ExternalEvent::with_payload(
                    self.event_id,
                    self.kind,
                    self.at,
                    ergo_adapter::EventPayload { data: bytes },
                )
                .map_err(|err| {
                    HostedStepError::EventBuild(HostedEventBuildError::InvalidPayload(err))
                })
            }
            None => Ok(ExternalEvent::mechanical_at(
                self.event_id,
                self.kind,
                self.at,
            )),
        }
    }
}

#[derive(Default)]
struct InvokedEffectDispatch {
    applied_writes: Vec<AppliedWrite>,
    intent_acks: Vec<CapturedIntentAck>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepMode {
    Live,
    Replay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureFinalizationState {
    NoCommittedSteps,
    Eligible,
    FinalizeOnly,
    Fatal,
}

/// Domain-shaped transition methods. Each method represents a specific
/// lifecycle event rather than a generic state target, so call sites
/// cannot request meaningless moves.
///
/// Legal transitions:
/// - `NoCommittedSteps → Eligible`   (on_step_success)
/// - `NoCommittedSteps → FinalizeOnly` (on_dispatch_failure)
/// - `NoCommittedSteps → Fatal`      (on_fatal_error)
/// - `Eligible → FinalizeOnly`       (on_dispatch_failure)
/// - `Eligible → Fatal`              (on_fatal_error)
/// - `FinalizeOnly → Fatal`          (on_fatal_error)
impl CaptureFinalizationState {
    /// A step executed successfully. `NoCommittedSteps → Eligible` on first
    /// call; idempotent once `Eligible`. `FinalizeOnly`/`Fatal` are unreachable
    /// here (guarded by `ensure_step_allowed`) but preserved defensively.
    fn on_step_success(self) -> Self {
        match self {
            Self::NoCommittedSteps => Self::Eligible,
            other => other,
        }
    }

    /// An egress dispatch failure occurred. Moves to `FinalizeOnly`.
    /// Legal from `NoCommittedSteps` and `Eligible`. Idempotent from
    /// `FinalizeOnly`. `Fatal` is preserved (already terminal).
    fn on_dispatch_failure(self) -> Self {
        match self {
            Self::Fatal => Self::Fatal,
            _ => Self::FinalizeOnly,
        }
    }

    /// A non-recoverable error occurred. Always moves to `Fatal`.
    fn on_fatal_error(self) -> Self {
        Self::Fatal
    }
}

pub(crate) const HOST_INTERNAL_SET_CONTEXT_KIND: &str = "set_context";

fn default_handlers() -> BTreeMap<String, Arc<dyn EffectHandler>> {
    let mut handlers: BTreeMap<String, Arc<dyn EffectHandler>> = BTreeMap::new();
    handlers.insert(
        HOST_INTERNAL_SET_CONTEXT_KIND.to_string(),
        Arc::new(SetContextHandler),
    );
    handlers
}

pub(crate) fn host_internal_handler_kinds() -> BTreeSet<String> {
    BTreeSet::from([HOST_INTERNAL_SET_CONTEXT_KIND.to_string()])
}

pub(crate) fn validate_hosted_runner_configuration(
    adapter: Option<&HostedAdapterConfig>,
    egress_config: Option<&EgressConfig>,
    egress_provenance: Option<&str>,
    replay_external_kinds: &HashSet<String>,
    graph_emittable_effect_kinds: &HashSet<String>,
) -> Result<Vec<EgressValidationWarning>, HostedStepError> {
    let handler_kinds = host_internal_handler_kinds();

    if egress_config.is_some() && !replay_external_kinds.is_empty() {
        return Err(HostedStepError::EgressValidation(
            HostedEgressValidationError::ReplayOwnershipWithLiveEgress,
        ));
    }

    if let Some(conflict) = replay_external_kinds
        .iter()
        .find(|kind| handler_kinds.contains(*kind))
    {
        return Err(HostedStepError::EgressValidation(
            HostedEgressValidationError::ReplayOwnedKindConflictsWithHandler {
                kind: conflict.clone(),
            },
        ));
    }

    let mut warnings = Vec::new();

    if let Some(config) = adapter {
        if let Some(egress_config) = egress_config {
            warnings = validate_egress_config(
                egress_config,
                &config.provides,
                graph_emittable_effect_kinds,
                &handler_kinds,
            )?;
        } else {
            ensure_handler_coverage(
                &config.provides,
                graph_emittable_effect_kinds,
                &handler_kinds,
                replay_external_kinds,
            )?;
        }
    } else if egress_config.is_some() {
        return Err(HostedStepError::EgressValidation(
            HostedEgressValidationError::EgressConfigRequiresAdapterBoundMode,
        ));
    } else if !replay_external_kinds.is_empty() {
        return Err(HostedStepError::EgressValidation(
            HostedEgressValidationError::ReplayOwnershipRequiresAdapterBoundMode,
        ));
    }

    if egress_config.is_none() && egress_provenance.is_some() {
        return Err(HostedStepError::EgressValidation(
            HostedEgressValidationError::EgressProvenanceRequiresConfig,
        ));
    }
    if egress_config.is_some() && egress_provenance.is_none() {
        return Err(HostedStepError::EgressValidation(
            HostedEgressValidationError::MissingEgressProvenance,
        ));
    }

    Ok(warnings)
}

pub struct HostedRunner {
    session: CapturingSession<HostDecisionLog, BufferingRuntimeInvoker>,
    decision_log: HostDecisionLog,
    runtime: BufferingRuntimeInvoker,
    context_store: ContextStore,
    seen_event_ids: HashSet<String>,
    adapter: Option<AdapterMode>,
    handlers: BTreeMap<String, Arc<dyn EffectHandler>>,
    applied_effects: AppliedEffectsByDecision,
    applied_intent_acks: AppliedIntentAcksByDecision,
    interruptions: StepInterruptionsByDecision,
    egress: Option<EgressRuntime>,
    egress_provenance: Option<String>,
    replay_external_kinds: HashSet<String>,
    capture_finalization_state: CaptureFinalizationState,
    #[cfg(test)]
    last_step_mode: Option<StepMode>,
}

impl HostedRunner {
    /// Construct a `HostedRunner` from pre-assembled components.
    ///
    /// # Trust boundary
    ///
    /// This is a **low-level constructor** that does not enforce the
    /// production adapter closure gate (`ensure_production_adapter_bound`).
    /// Callers using this constructor directly are responsible for ensuring
    /// that production sessions always provide an `adapter`.
    ///
    /// **Preferred API paths** (which enforce the gate automatically):
    /// - [`prepare_hosted_runner_from_paths`] / [`prepare_hosted_runner`]
    ///   (via the host usecases layer)
    /// - [`LivePrepOptions::for_production`] /
    ///   [`PrepareHostedRunnerFromPathsRequest::for_production`]
    ///   (structurally require a non-optional adapter)
    ///
    /// Passing `adapter: None` is valid only for fixture/replay sessions
    /// where external data is pre-authored and trusted.  Using
    /// `adapter: None` with live production data bypasses schema
    /// validation on every event payload.
    pub fn new(
        graph_id: GraphId,
        constraints: Constraints,
        runtime: BufferingRuntimeInvoker,
        runtime_provenance: String,
        adapter: Option<HostedAdapterConfig>,
        egress_config: Option<EgressConfig>,
        egress_provenance: Option<String>,
        replay_external_kinds: Option<HashSet<String>>,
    ) -> Result<Self, HostedStepError> {
        let graph_emittable_effect_kinds = runtime.graph_emittable_effect_kinds().clone();
        let replay_external_kinds = replay_external_kinds.unwrap_or_default();
        let warnings = validate_hosted_runner_configuration(
            adapter.as_ref(),
            egress_config.as_ref(),
            egress_provenance.as_deref(),
            &replay_external_kinds,
            &graph_emittable_effect_kinds,
        )?;
        emit_warnings_to_stderr(&warnings);

        Ok(Self::new_validated(
            graph_id,
            constraints,
            runtime,
            runtime_provenance,
            adapter,
            egress_config,
            egress_provenance,
            replay_external_kinds,
        ))
    }

    pub(crate) fn new_validated(
        graph_id: GraphId,
        constraints: Constraints,
        runtime: BufferingRuntimeInvoker,
        runtime_provenance: String,
        adapter: Option<HostedAdapterConfig>,
        egress_config: Option<EgressConfig>,
        egress_provenance: Option<String>,
        replay_external_kinds: HashSet<String>,
    ) -> Self {
        let handlers = default_handlers();
        let egress = egress_config.map(EgressRuntime::new);
        let decision_log = HostDecisionLog::default();

        let adapter_provenance = adapter
            .as_ref()
            .map(|config| config.adapter_provenance.clone())
            .unwrap_or_else(|| NO_ADAPTER_PROVENANCE.to_string());

        let session = CapturingSession::new_with_provenance(
            graph_id,
            constraints,
            decision_log.clone(),
            runtime.clone(),
            adapter_provenance,
            runtime_provenance,
        );

        let adapter = adapter.map(|config| AdapterMode {
            provides: config.provides,
            binder: config.binder,
        });

        Self {
            session,
            decision_log,
            runtime,
            context_store: ContextStore::new(),
            seen_event_ids: HashSet::new(),
            adapter,
            handlers,
            applied_effects: AppliedEffectsByDecision::default(),
            applied_intent_acks: AppliedIntentAcksByDecision::default(),
            interruptions: StepInterruptionsByDecision::default(),
            egress,
            egress_provenance,
            replay_external_kinds,
            capture_finalization_state: CaptureFinalizationState::NoCommittedSteps,
            #[cfg(test)]
            last_step_mode: None,
        }
    }

    pub fn step(&mut self, event: HostedEvent) -> Result<HostedStepOutcome, HostedStepError> {
        self.ensure_step_allowed()?;
        let external_event = match self.build_external_event(event) {
            Ok(external_event) => external_event,
            Err(err) => {
                self.record_step_error(&err);
                return Err(err);
            }
        };
        self.execute_public_step(external_event, StepMode::Live)
    }

    pub fn replay_step(
        &mut self,
        external_event: ExternalEvent,
    ) -> Result<HostedStepOutcome, HostedStepError> {
        self.ensure_step_allowed()?;
        self.execute_public_step(external_event, StepMode::Replay)
    }

    fn execute_public_step(
        &mut self,
        external_event: ExternalEvent,
        mode: StepMode,
    ) -> Result<HostedStepOutcome, HostedStepError> {
        let outcome = self.execute_step(external_event, mode);
        match &outcome {
            Ok(_) => {
                self.capture_finalization_state = self.capture_finalization_state.on_step_success();
            }
            Err(err) => self.record_step_error(err),
        }
        outcome
    }

    fn execute_step(
        &mut self,
        external_event: ExternalEvent,
        mode: StepMode,
    ) -> Result<HostedStepOutcome, HostedStepError> {
        if mode == StepMode::Live {
            self.start_egress_channels()?;
        }
        #[cfg(test)]
        {
            self.last_step_mode = Some(mode);
        }

        let event_id = external_event.event_id().as_str().to_string();
        if !self.seen_event_ids.insert(event_id.clone()) {
            return Err(HostedStepError::DuplicateEventId { event_id });
        }

        if self.runtime.pending_effect_count() != 0 {
            return Err(HostedStepError::LifecycleViolation {
                detail: "pending effect buffer must be drained before next on_event".to_string(),
            });
        }

        let pre_entry_len = self.decision_log.len();
        let pre_run_calls = self.runtime.run_call_count();

        self.session.on_event(external_event);

        let post_entry_len = self.decision_log.len();
        if post_entry_len != pre_entry_len + 1 {
            return Err(HostedStepError::LifecycleViolation {
                detail: format!(
                    "expected exactly one decision entry for step, got {} new entries",
                    post_entry_len.saturating_sub(pre_entry_len)
                ),
            });
        }

        let decision_index = post_entry_len - 1;
        let entry = self
            .decision_log
            .get(decision_index)
            .ok_or(HostedStepError::MissingDecisionEntry)?;

        let run_calls = self.runtime.run_call_count().saturating_sub(pre_run_calls);

        let drained_effects = self.runtime.drain_pending_effects();

        let dispatch = if entry.decision == Decision::Invoke {
            let expected_calls = (entry.retry_count as u64).saturating_add(1);
            if run_calls != expected_calls {
                return Err(HostedStepError::LifecycleViolation {
                    detail: format!(
                        "run call count mismatch: expected {expected_calls}, got {run_calls}"
                    ),
                });
            }

            self.dispatch_invoked_effects(decision_index, mode, &drained_effects)?
        } else {
            if run_calls != 0 {
                return Err(HostedStepError::LifecycleViolation {
                    detail: format!(
                        "non-invoke decision {:?} unexpectedly performed {run_calls} run calls",
                        entry.decision
                    ),
                });
            }

            if !drained_effects.is_empty() {
                return Err(HostedStepError::LifecycleViolation {
                    detail: "non-invoke decision produced pending effects".to_string(),
                });
            }
            InvokedEffectDispatch::default()
        };

        Ok(HostedStepOutcome {
            decision: entry.decision,
            termination: entry.termination,
            retry_count: entry.retry_count,
            effects: drained_effects,
            applied_writes: dispatch.applied_writes,
        })
    }

    pub fn start_egress_channels(&mut self) -> Result<(), HostedStepError> {
        let Some(egress) = self.egress.as_mut() else {
            return Ok(());
        };
        egress.start_channels()?;
        Ok(())
    }

    pub fn ensure_no_pending_egress_acks(
        &mut self,
        host_stop_requested: bool,
    ) -> Result<(), HostedStepError> {
        let Some(egress) = self.egress.as_mut() else {
            return Ok(());
        };
        egress.assert_no_pending_acks(host_stop_requested)?;
        Ok(())
    }

    pub fn stop_egress_channels(&mut self) -> Result<(), HostedStepError> {
        let Some(egress) = self.egress.as_mut() else {
            return Ok(());
        };
        egress.shutdown_channels()?;
        Ok(())
    }

    pub fn into_capture_bundle(self) -> CaptureBundle {
        self.into_capture_bundle_and_egress().0
    }

    pub fn into_capture_bundle_and_egress(mut self) -> (CaptureBundle, Option<EgressRuntime>) {
        let mut bundle = self.session.into_bundle();
        bundle.egress_provenance = self.egress_provenance.clone();
        enrich_bundle_with_host_artifacts(
            &mut bundle,
            self.applied_effects.effects(),
            self.applied_intent_acks.intent_acks(),
            self.interruptions.interruptions(),
        );
        let egress = self.egress.take();
        (bundle, egress)
    }

    pub fn context_snapshot(&self) -> &BTreeMap<String, serde_json::Value> {
        self.context_store.snapshot()
    }

    pub(crate) fn ensure_capture_finalizable(&self) -> Result<(), HostedStepError> {
        match self.capture_finalization_state {
            CaptureFinalizationState::NoCommittedSteps => {
                Err(HostedStepError::LifecycleViolation {
                    detail: "hosted runner cannot finalize before the first committed step"
                        .to_string(),
                })
            }
            CaptureFinalizationState::Fatal => Err(HostedStepError::LifecycleViolation {
                detail: "hosted runner cannot finalize after a non-finalizable step error"
                    .to_string(),
            }),
            CaptureFinalizationState::Eligible | CaptureFinalizationState::FinalizeOnly => Ok(()),
        }
    }

    fn ensure_step_allowed(&self) -> Result<(), HostedStepError> {
        match self.capture_finalization_state {
            CaptureFinalizationState::FinalizeOnly => {
                Err(HostedStepError::LifecycleViolation {
                    detail:
                        "hosted runner must be finalized after egress dispatch failure before stepping again"
                            .to_string(),
                })
            }
            CaptureFinalizationState::Fatal => Err(HostedStepError::LifecycleViolation {
                detail: "hosted runner cannot continue after a non-finalizable step error"
                    .to_string(),
            }),
            CaptureFinalizationState::NoCommittedSteps | CaptureFinalizationState::Eligible => {
                Ok(())
            }
        }
    }

    #[cfg(test)]
    fn last_step_mode(&self) -> Option<StepMode> {
        self.last_step_mode
    }

    fn record_step_error(&mut self, err: &HostedStepError) {
        self.capture_finalization_state = match err {
            HostedStepError::EgressDispatchFailure(_) => {
                self.capture_finalization_state.on_dispatch_failure()
            }
            err if is_recoverable_hosted_step_error(err) => self.capture_finalization_state,
            _ => self.capture_finalization_state.on_fatal_error(),
        };
    }

    fn build_external_event(&self, event: HostedEvent) -> Result<ExternalEvent, HostedStepError> {
        let event = ValidatedHostedEvent::new(event)?;
        if let Some(adapter) = &self.adapter {
            let semantic_kind = event.semantic_kind()?.to_string();
            let incoming_object = event.payload_object()?.clone();
            let allowed_store_keys = allowed_schema_keys(adapter, &semantic_kind)?;

            let mut merged = serde_json::Map::new();
            for (key, value) in self.context_store.snapshot() {
                if adapter.provides.context.contains_key(key) && allowed_store_keys.contains(key) {
                    merged.insert(key.clone(), value.clone());
                }
            }

            for (key, value) in incoming_object {
                merged.insert(key, value);
            }

            return bind_semantic_event_with_binder(
                &adapter.binder,
                event.event_id,
                event.kind,
                event.at,
                &semantic_kind,
                serde_json::Value::Object(merged),
            )
            .map_err(HostedStepError::Binding);
        }

        event.into_external_event()
    }

    fn dispatch_invoked_effects(
        &mut self,
        decision_index: usize,
        mode: StepMode,
        drained_effects: &[ActionEffect],
    ) -> Result<InvokedEffectDispatch, HostedStepError> {
        let Some(adapter) = &self.adapter else {
            if !drained_effects.is_empty() {
                return Err(HostedStepError::EffectsWithoutAdapter);
            }
            return Ok(InvokedEffectDispatch::default());
        };

        if !drained_effects.is_empty() {
            self.applied_effects
                .record(decision_index, drained_effects.to_vec());
        }

        let egress_owned_kinds = self
            .egress
            .as_ref()
            .map(EgressRuntime::route_kind_set)
            .unwrap_or_else(|| self.replay_external_kinds.clone());
        let mut dispatch = InvokedEffectDispatch::default();

        for effect in drained_effects {
            let handler = self.handlers.get(&effect.kind);
            let egress_owned = egress_owned_kinds.contains(&effect.kind);

            match (handler, egress_owned) {
                (Some(_), true) => {
                    return Err(HostedStepError::LifecycleViolation {
                        detail: format!(
                            "effect kind '{}' is ambiguously owned by both handler and egress",
                            effect.kind
                        ),
                    });
                }
                (Some(handler), false) => {
                    if !effect.intents.is_empty() {
                        return Err(HostedStepError::LifecycleViolation {
                            detail: format!(
                                "handler-owned effect '{}' must not carry intents",
                                effect.kind
                            ),
                        });
                    }
                    // SUP-6 alignment: no rollback on handler failure.
                    let writes =
                        handler.apply(effect, &mut self.context_store, &adapter.provides)?;
                    dispatch.applied_writes.extend(writes);
                }
                (None, true) => {
                    if !effect.writes.is_empty() {
                        return Err(HostedStepError::LifecycleViolation {
                            detail: format!(
                                "egress-owned effect '{}' must not carry writes",
                                effect.kind
                            ),
                        });
                    }
                    if effect.intents.is_empty() {
                        return Err(HostedStepError::LifecycleViolation {
                            detail: format!(
                                "egress-owned effect '{}' must carry at least one intent",
                                effect.kind
                            ),
                        });
                    }
                    if effect
                        .intents
                        .iter()
                        .any(|intent| intent.kind != effect.kind)
                    {
                        return Err(HostedStepError::LifecycleViolation {
                            detail: format!(
                                "egress-owned effect '{}' contains intent with mismatched kind",
                                effect.kind
                            ),
                        });
                    }

                    if mode == StepMode::Live {
                        let Some(egress) = self.egress.as_mut() else {
                            return Err(HostedStepError::LifecycleViolation {
                                detail: "intent dispatch required but no egress runtime configured"
                                    .to_string(),
                            });
                        };

                        for intent in &effect.intents {
                            match egress.dispatch_intent(intent) {
                                Ok(ack) => dispatch.intent_acks.push(ack),
                                Err(err) => {
                                    let dispatch_failure = map_egress_dispatch_error(err)?;
                                    if !dispatch.intent_acks.is_empty() {
                                        self.applied_intent_acks
                                            .record(decision_index, dispatch.intent_acks.clone());
                                    }
                                    self.interruptions.record(
                                        decision_index,
                                        format!("egress dispatch failed: {}", dispatch_failure),
                                    );
                                    return Err(HostedStepError::EgressDispatchFailure(
                                        dispatch_failure,
                                    ));
                                }
                            }
                        }
                    }
                }
                (None, false) => {
                    return Err(HostedStepError::from(
                        EffectApplyError::UnhandledEffectKind {
                            kind: effect.kind.clone(),
                        },
                    ));
                }
            }
        }

        if !dispatch.intent_acks.is_empty() {
            self.applied_intent_acks
                .record(decision_index, dispatch.intent_acks.clone());
        }

        Ok(dispatch)
    }
}

pub fn is_recoverable_hosted_step_error(err: &HostedStepError) -> bool {
    matches!(
        err,
        HostedStepError::DuplicateEventId { .. }
            | HostedStepError::MissingSemanticKind
            | HostedStepError::MissingPayload
            | HostedStepError::PayloadMustBeObject
            | HostedStepError::UnknownSemanticKind { .. }
            | HostedStepError::Binding(_)
            | HostedStepError::EventBuild(_)
    )
}

fn map_egress_dispatch_error(
    err: EgressProcessError,
) -> Result<EgressDispatchFailure, HostedStepError> {
    match err {
        EgressProcessError::Timeout {
            channel, intent_id, ..
        } => Ok(EgressDispatchFailure::AckTimeout { channel, intent_id }),
        EgressProcessError::Protocol { channel, detail } => {
            Ok(EgressDispatchFailure::ProtocolViolation { channel, detail })
        }
        EgressProcessError::Io { channel, detail } => {
            Ok(EgressDispatchFailure::Io { channel, detail })
        }
        EgressProcessError::Startup { .. }
        | EgressProcessError::InvalidConfig(..)
        | EgressProcessError::PendingAcks { .. } => Err(HostedStepError::EgressProcess(err)),
    }
}

fn allowed_schema_keys(
    adapter: &AdapterMode,
    semantic_kind: &str,
) -> Result<HashSet<String>, HostedStepError> {
    let Some(schema) = adapter.provides.event_schemas.get(semantic_kind) else {
        return Err(HostedStepError::UnknownSemanticKind {
            kind: semantic_kind.to_string(),
        });
    };

    let mut keys = HashSet::new();
    if let Some(properties) = schema.get("properties").and_then(|value| value.as_object()) {
        for key in properties.keys() {
            keys.insert(key.clone());
        }
    }

    Ok(keys)
}

#[cfg(test)]
mod tests;
