use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use ergo_adapter::{
    capture::ExternalEventRecord, ErrKind, EventId, EventTime, ExecutionContext, ExternalEvent,
    ExternalEventKind, GraphId, RunTermination, RuntimeHandle, RuntimeInvoker,
};
use ergo_runtime::catalog::{CorePrimitiveCatalog, CoreRegistries};
use ergo_runtime::cluster::ExpandedGraph;
use serde::{Deserialize, Serialize};

/// Capture bundle format version. Used for forward-compatibility checks.
pub(crate) const CAPTURE_FORMAT_VERSION: &str = "v0";

mod capture;
#[cfg(any(test, feature = "demo"))]
pub mod fixture_runner;
pub mod replay;

#[cfg(any(test, feature = "demo"))]
pub mod demo;

pub use capture::{CapturingDecisionLog, CapturingSession};

/// SUP-7: DecisionLog is write-only. No read/query surface is ever exposed.
pub trait DecisionLog {
    fn log(&self, entry: DecisionLogEntry);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct DeterministicClock {
    now: EventTime,
}

impl DeterministicClock {
    fn new() -> Self {
        Self {
            now: EventTime::default(),
        }
    }

    fn advance_to(&mut self, at: EventTime) {
        if at > self.now {
            self.now = at;
        }
    }

    fn now(&self) -> EventTime {
        self.now
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EpisodeId(u64);

impl EpisodeId {
    pub fn new(id: u64) -> Self {
        EpisodeId(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone)]
struct DeferredEpisode {
    origin_event_id: EventId,
    ctx: ExecutionContext,
    defer_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Invoke,
    Skip,
    Defer,
}

#[derive(Debug, Clone)]
pub struct DecisionLogEntry {
    pub graph_id: GraphId,
    pub event_id: EventId,
    pub event: ExternalEvent,
    pub decision: Decision,
    pub schedule_at: Option<EventTime>,
    pub episode_id: EpisodeId,
    pub deadline: Option<Duration>,
    pub termination: Option<RunTermination>,
    pub retry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EpisodeInvocationRecord {
    pub event_id: EventId,
    pub decision: Decision,
    pub schedule_at: Option<EventTime>,
    pub episode_id: EpisodeId,
    pub deadline: Option<Duration>,
    #[serde(default)]
    pub termination: Option<RunTermination>,
    pub retry_count: usize,
}

impl From<&DecisionLogEntry> for EpisodeInvocationRecord {
    fn from(entry: &DecisionLogEntry) -> Self {
        Self {
            event_id: entry.event_id.clone(),
            decision: entry.decision,
            schedule_at: entry.schedule_at,
            episode_id: entry.episode_id,
            deadline: entry.deadline,
            termination: entry.termination.clone(),
            retry_count: entry.retry_count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureBundle {
    pub capture_version: String,
    pub graph_id: GraphId,
    pub config: Constraints,
    pub events: Vec<ExternalEventRecord>,
    pub decisions: Vec<EpisodeInvocationRecord>,
    pub adapter_version: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Constraints {
    pub max_in_flight: Option<usize>,
    pub max_per_window: Option<usize>,
    pub rate_window: Option<Duration>,
    pub deadline: Option<Duration>,
    pub max_retries: usize,
}

pub struct Supervisor<L: DecisionLog, R: RuntimeInvoker> {
    graph_id: GraphId,
    constraints: Constraints,
    decision_log: L,
    runtime: R,
    next_episode_id: u64,
    in_flight: usize,
    recent_invocations: VecDeque<EventTime>,
    clock: DeterministicClock,
    deferred_queue: BTreeMap<(EventTime, EpisodeId), DeferredEpisode>,
}

impl<L: DecisionLog> Supervisor<L, RuntimeHandle> {
    pub fn new(
        graph_id: GraphId,
        constraints: Constraints,
        decision_log: L,
        graph: Arc<ExpandedGraph>,
        catalog: Arc<CorePrimitiveCatalog>,
        registries: Arc<CoreRegistries>,
    ) -> Self {
        Self {
            graph_id,
            constraints,
            decision_log,
            runtime: RuntimeHandle::new(graph, catalog, registries),
            next_episode_id: 0,
            in_flight: 0,
            recent_invocations: VecDeque::new(),
            clock: DeterministicClock::new(),
            deferred_queue: BTreeMap::new(),
        }
    }
}

impl<L: DecisionLog, R: RuntimeInvoker> Supervisor<L, R> {
    pub fn with_runtime(
        graph_id: GraphId,
        constraints: Constraints,
        decision_log: L,
        runtime: R,
    ) -> Self {
        Self {
            graph_id,
            constraints,
            decision_log,
            runtime,
            next_episode_id: 0,
            in_flight: 0,
            recent_invocations: VecDeque::new(),
            clock: DeterministicClock::new(),
            deferred_queue: BTreeMap::new(),
        }
    }

    pub fn on_event(&mut self, event: ExternalEvent) {
        self.clock.advance_to(event.at());
        let now = self.clock.now();

        if event.kind() == ExternalEventKind::Pump {
            self.process_tick(&event, now);
            return;
        }

        let episode_id = self.next_episode_id();

        if self.is_concurrency_saturated() {
            self.enqueue_deferred(now, episode_id, &event);
            self.log_decision(&event, Decision::Defer, Some(now), episode_id, None, 0);
            return;
        }

        if let Some(delay) = self.rate_limit_delay(now) {
            let schedule_at = now.saturating_add(delay);
            self.enqueue_deferred(schedule_at, episode_id, &event);
            self.log_decision(
                &event,
                Decision::Defer,
                Some(schedule_at),
                episode_id,
                None,
                0,
            );
            return;
        }

        self.in_flight = self.in_flight.saturating_add(1);
        if self.constraints.max_per_window.is_some() && self.constraints.rate_window.is_some() {
            self.recent_invocations.push_back(now);
        }

        let (termination, retry_count) =
            self.invoke_with_retries(event.event_id(), event.context());

        self.in_flight = self.in_flight.saturating_sub(1);

        self.log_decision(
            &event,
            Decision::Invoke,
            None,
            episode_id,
            Some(termination),
            retry_count,
        );
    }

    fn next_episode_id(&mut self) -> EpisodeId {
        let id = EpisodeId::new(self.next_episode_id);
        self.next_episode_id = self.next_episode_id.saturating_add(1);
        id
    }

    fn is_concurrency_saturated(&self) -> bool {
        matches!(self.constraints.max_in_flight, Some(max) if self.in_flight >= max)
    }

    fn enqueue_deferred(
        &mut self,
        schedule_at: EventTime,
        episode_id: EpisodeId,
        event: &ExternalEvent,
    ) {
        self.deferred_queue.insert(
            (schedule_at, episode_id),
            DeferredEpisode {
                origin_event_id: event.event_id().clone(),
                ctx: event.context().clone(),
                defer_count: 0,
            },
        );
    }

    fn process_tick(&mut self, tick_event: &ExternalEvent, now: EventTime) {
        // Find first due episode: schedule_at <= now
        let due_key = self
            .deferred_queue
            .keys()
            .find(|(schedule_at, _)| *schedule_at <= now)
            .cloned();

        // CASE 1: Nothing due — log no-op
        let Some(key) = due_key else {
            let episode_id = self.next_episode_id();
            self.log_decision(tick_event, Decision::Defer, None, episode_id, None, 0);
            return;
        };

        let mut item = self.deferred_queue.remove(&key).unwrap();
        let episode_id = key.1;

        // CASE 2: Still saturated — re-defer
        if self.is_concurrency_saturated() {
            item.defer_count += 1;
            self.deferred_queue.insert((now, episode_id), item);
            self.log_decision(tick_event, Decision::Defer, Some(now), episode_id, None, 0);
            return;
        }

        // CASE 3: Rate limited — re-defer with delay
        if let Some(delay) = self.rate_limit_delay(now) {
            item.defer_count += 1;
            let schedule_at = now.saturating_add(delay);
            self.deferred_queue.insert((schedule_at, episode_id), item);
            self.log_decision(
                tick_event,
                Decision::Defer,
                Some(schedule_at),
                episode_id,
                None,
                0,
            );
            return;
        }

        // CASE 4: Can run — invoke with SUP-4 retries
        self.in_flight = self.in_flight.saturating_add(1);
        if self.constraints.max_per_window.is_some() && self.constraints.rate_window.is_some() {
            self.recent_invocations.push_back(now);
        }

        let (termination, retry_count) = self.invoke_with_retries(&item.origin_event_id, &item.ctx);

        self.in_flight = self.in_flight.saturating_sub(1);

        self.log_decision(
            tick_event,
            Decision::Invoke,
            None,
            episode_id,
            Some(termination),
            retry_count,
        );
    }

    fn rate_limit_delay(&mut self, now: EventTime) -> Option<Duration> {
        let Some(max_per_window) = self.constraints.max_per_window else {
            return None;
        };
        let Some(window) = self.constraints.rate_window else {
            return None;
        };

        while let Some(front) = self.recent_invocations.front() {
            if now.as_duration().saturating_sub(front.as_duration()) >= window {
                self.recent_invocations.pop_front();
            } else {
                break;
            }
        }

        if self.recent_invocations.len() >= max_per_window {
            if let Some(front) = self.recent_invocations.front() {
                let elapsed = now.as_duration().saturating_sub(front.as_duration());
                let delay = window.saturating_sub(elapsed);
                return Some(delay);
            }
        }

        None
    }

    fn invoke_with_retries(
        &self,
        event_id: &EventId,
        ctx: &ergo_adapter::ExecutionContext,
    ) -> (RunTermination, usize) {
        let mut attempts = 0_usize;
        let mut termination =
            self.runtime
                .run(&self.graph_id, event_id, ctx, self.constraints.deadline);

        while attempts < self.constraints.max_retries && Self::should_retry(&termination) {
            attempts = attempts.saturating_add(1);
            termination =
                self.runtime
                    .run(&self.graph_id, event_id, ctx, self.constraints.deadline);
        }

        (termination, attempts)
    }

    fn should_retry(termination: &RunTermination) -> bool {
        match termination {
            RunTermination::Failed(err) => match err {
                ErrKind::SemanticError => false,
                ErrKind::NetworkTimeout | ErrKind::AdapterUnavailable | ErrKind::RuntimeError => {
                    true
                }
                _ => false,
            },
            RunTermination::TimedOut => true,
            _ => false,
        }
    }

    fn log_decision(
        &self,
        event: &ExternalEvent,
        decision: Decision,
        schedule_at: Option<EventTime>,
        episode_id: EpisodeId,
        termination: Option<RunTermination>,
        retry_count: usize,
    ) {
        let entry = DecisionLogEntry {
            graph_id: self.graph_id.clone(),
            event_id: event.event_id().clone(),
            event: event.clone(),
            decision,
            schedule_at,
            episode_id,
            deadline: self.constraints.deadline,
            termination,
            retry_count,
        };
        self.decision_log.log(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DecisionLog, DecisionLogEntry, ErrKind, RunTermination, RuntimeInvoker, Supervisor,
    };

    struct TestLog;

    impl DecisionLog for TestLog {
        fn log(&self, _entry: DecisionLogEntry) {}
    }

    struct TestRuntime;

    impl RuntimeInvoker for TestRuntime {
        fn run(
            &self,
            _graph_id: &ergo_adapter::GraphId,
            _event_id: &ergo_adapter::EventId,
            _ctx: &ergo_adapter::ExecutionContext,
            _deadline: Option<std::time::Duration>,
        ) -> RunTermination {
            RunTermination::Completed
        }
    }

    #[test]
    fn semantic_error_not_retryable() {
        let termination = RunTermination::Failed(ErrKind::SemanticError);
        assert!(!Supervisor::<TestLog, TestRuntime>::should_retry(
            &termination
        ));
    }

    #[test]
    fn runtime_error_is_retryable() {
        let termination = RunTermination::Failed(ErrKind::RuntimeError);
        assert!(Supervisor::<TestLog, TestRuntime>::should_retry(
            &termination
        ));
    }
}
