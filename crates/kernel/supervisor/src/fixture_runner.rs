use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use ergo_adapter::fixture::FixtureItem;
use ergo_adapter::{
    ensure_demo_sources_have_no_required_context, AdapterProvides, EventId, EventPayload,
    EventTime, ExternalEvent, ExternalEventKind, GraphId, RuntimeHandle,
};
use ergo_runtime::action::ActionOutcome;
use ergo_runtime::catalog::{CorePrimitiveCatalog, CoreRegistries};
use ergo_runtime::cluster::ExpandedGraph;
use ergo_runtime::provenance::{compute_runtime_provenance, RuntimeProvenanceScheme};

use crate::demo::demo_1;
use crate::{
    write_capture_bundle, CaptureBundle, CaptureJsonStyle, CapturingSession, Constraints, Decision,
    DecisionLog, DecisionLogEntry, NO_ADAPTER_PROVENANCE,
};

const DEFAULT_GRAPH_ID: &str = "demo_1";
const DEFAULT_ARTIFACT_NAME: &str = "fixture-capture.json";

#[derive(Debug, Clone)]
pub struct EpisodeSummary {
    pub label: String,
    pub decision: String,
    pub trigger_a: String,
    pub trigger_b: String,
    pub action_a: String,
    pub action_b: String,
}

#[derive(Debug)]
pub struct FixtureRunResult {
    pub artifact_path: PathBuf,
    pub episodes: Vec<EpisodeSummary>,
}

struct NullLog;

impl DecisionLog for NullLog {
    fn log(&self, _entry: DecisionLogEntry) {}
}

#[derive(Debug)]
struct EpisodeInfo {
    label: String,
    event_ids: Vec<String>,
}

pub fn run_fixture(
    items: Vec<FixtureItem>,
    graph: Arc<ExpandedGraph>,
    catalog: Arc<CorePrimitiveCatalog>,
    registries: Arc<CoreRegistries>,
    output_path: Option<PathBuf>,
    capture_style: CaptureJsonStyle,
) -> Result<FixtureRunResult, String> {
    ensure_demo_sources_have_no_required_context(&graph, &catalog, &registries)?;
    let runtime_provenance = compute_runtime_provenance(
        RuntimeProvenanceScheme::Rpv1,
        DEFAULT_GRAPH_ID,
        graph.as_ref(),
        catalog.as_ref(),
    )
    .map_err(|err| format!("runtime provenance compute failed: {err}"))?;
    let runtime = RuntimeHandle::new(graph, catalog, registries, AdapterProvides::default());
    let mut session = CapturingSession::new_with_provenance(
        GraphId::new(DEFAULT_GRAPH_ID),
        Constraints::default(),
        NullLog,
        runtime,
        NO_ADAPTER_PROVENANCE.to_string(),
        runtime_provenance,
    );

    let mut episodes: Vec<EpisodeInfo> = Vec::new();
    let mut current_episode: Option<usize> = None;
    let mut event_counter = 0usize;

    for item in items {
        match item {
            FixtureItem::EpisodeStart { label } => {
                episodes.push(EpisodeInfo {
                    label,
                    event_ids: Vec::new(),
                });
                current_episode = Some(episodes.len() - 1);
            }
            FixtureItem::Event {
                id, kind, payload, ..
            } => {
                if current_episode.is_none() {
                    let label = format!("E{}", episodes.len() + 1);
                    episodes.push(EpisodeInfo {
                        label,
                        event_ids: Vec::new(),
                    });
                    current_episode = Some(episodes.len() - 1);
                }

                event_counter += 1;
                let event_id = id.unwrap_or_else(|| format!("fixture_evt_{}", event_counter));
                let external = event_from_payload(&event_id, kind, payload)?;
                session.on_event(external);

                let episode_index = current_episode.expect("episode index set");
                episodes[episode_index].event_ids.push(event_id);
            }
        }
    }

    if episodes.is_empty() {
        return Err("fixture contained no episodes".to_string());
    }

    if episodes.iter().all(|episode| episode.event_ids.is_empty()) {
        return Err("fixture contained no events".to_string());
    }

    if let Some(episode) = episodes.iter().find(|episode| episode.event_ids.is_empty()) {
        return Err(format!("episode '{}' has no events", episode.label));
    }

    let bundle = session.into_bundle();
    let summaries = summarize_episodes(&episodes, &bundle)?;

    let artifact_path =
        output_path.unwrap_or_else(|| PathBuf::from("target").join(DEFAULT_ARTIFACT_NAME));
    write_capture_bundle(&artifact_path, &bundle, capture_style)?;

    Ok(FixtureRunResult {
        artifact_path,
        episodes: summaries,
    })
}

fn event_from_payload(
    event_id: &str,
    kind: ExternalEventKind,
    payload: Option<serde_json::Value>,
) -> Result<ExternalEvent, String> {
    let event_id_value = EventId::new(event_id);
    if let Some(payload) = payload {
        let data = serde_json::to_vec(&payload)
            .map_err(|err| format!("fixture payload encode error for event '{event_id}': {err}"))?;
        ExternalEvent::with_payload(
            event_id_value,
            kind,
            EventTime::default(),
            EventPayload { data },
        )
        .map_err(|err| format!("fixture payload invalid for event '{event_id}': {err}"))
    } else {
        Ok(ExternalEvent::mechanical(event_id_value, kind))
    }
}

fn summarize_episodes(
    episodes: &[EpisodeInfo],
    bundle: &CaptureBundle,
) -> Result<Vec<EpisodeSummary>, String> {
    let mut decisions_by_event: HashMap<String, Vec<Decision>> = HashMap::new();
    for record in &bundle.decisions {
        decisions_by_event
            .entry(record.event_id.as_str().to_string())
            .or_default()
            .push(record.decision);
    }

    let mut context_by_event: HashMap<String, Option<f64>> = HashMap::new();
    for record in &bundle.events {
        context_by_event.insert(
            record.event_id.as_str().to_string(),
            context_value_from_payload(&record.payload),
        );
    }

    let mut summaries = Vec::new();

    for episode in episodes {
        let mut invoked = false;
        let mut deferred = false;
        let mut invoked_event: Option<&String> = None;

        for event_id in &episode.event_ids {
            let entries = decisions_by_event
                .get(event_id)
                .ok_or_else(|| format!("no decision for event '{event_id}'"))?;
            if entries.iter().any(|decision| *decision == Decision::Invoke) {
                invoked = true;
                if invoked_event.is_none() {
                    invoked_event = Some(event_id);
                }
            }
            if entries.iter().any(|decision| *decision == Decision::Defer) {
                deferred = true;
            }
        }

        let decision = if invoked {
            "invoke"
        } else if deferred {
            "defer"
        } else {
            "none"
        };

        let (trigger_a, trigger_b, action_a, action_b) = if invoked {
            let context_value = invoked_event
                .and_then(|event_id| context_by_event.get(event_id))
                .copied()
                .flatten();
            let summary = demo_1::summary_for_context_value(context_value);
            (
                trigger_status(summary.action_a_outcome.clone()),
                trigger_status(summary.action_b_outcome.clone()),
                action_status(summary.action_a_outcome),
                action_status(summary.action_b_outcome),
            )
        } else {
            ("deferred", "deferred", "deferred", "deferred")
        };

        summaries.push(EpisodeSummary {
            label: episode.label.clone(),
            decision: decision.to_string(),
            trigger_a: trigger_a.to_string(),
            trigger_b: trigger_b.to_string(),
            action_a: action_a.to_string(),
            action_b: action_b.to_string(),
        });
    }

    Ok(summaries)
}

fn action_status(outcome: ActionOutcome) -> &'static str {
    if outcome == ActionOutcome::Skipped {
        "skipped"
    } else {
        "executed"
    }
}

fn trigger_status(outcome: ActionOutcome) -> &'static str {
    if outcome == ActionOutcome::Skipped {
        "not_emitted"
    } else {
        "emitted"
    }
}

fn context_value_from_payload(payload: &EventPayload) -> Option<f64> {
    if payload.data.is_empty() {
        return None;
    }

    let parsed: serde_json::Value = serde_json::from_slice(&payload.data).ok()?;
    parsed
        .as_object()
        .and_then(|object| object.get(demo_1::CONTEXT_NUMBER_KEY))
        .and_then(|value| value.as_f64())
}
