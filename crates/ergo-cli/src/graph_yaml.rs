use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::adapter_manifest_io::parse_adapter_manifest;
use crate::error_format::{
    cli_error_from_error_info, render_cli_error, render_error_info, CliErrorInfo,
};
use ergo_adapter::{
    bind_semantic_event_with_binder, compile_event_binder, fixture,
    validate_action_adapter_composition, validate_capture_format,
    validate_source_adapter_composition, AdapterProvides, EventBinder, EventId, EventPayload,
    EventTime, ExternalEvent, ExternalEventKind, GraphId, RuntimeHandle,
};
use ergo_runtime::catalog::{
    build_core_catalog, core_registries, CorePrimitiveCatalog, CoreRegistries,
};
use ergo_runtime::cluster::{
    expand, BoundaryKind, Cardinality, ClusterDefinition, ClusterLoader, ClusterVersionIndex, Edge,
    ExpandedGraph, GraphInputPlaceholder, InputPortSpec, InputRef, NodeInstance, NodeKind,
    OutputPortSpec, OutputRef, ParameterBinding, ParameterSpec, ParameterType, ParameterValue,
    PortSpec, PrimitiveCatalog, PrimitiveKind, Signature, ValueType, Version,
};
use ergo_runtime::provenance::{compute_runtime_provenance, RuntimeProvenanceScheme};
use ergo_runtime::runtime::{
    ExecutionContext, Registries, RuntimeError, RuntimeEvent, RuntimeValue,
};
use ergo_supervisor::{
    write_capture_bundle, CaptureJsonStyle, CapturingSession, Constraints, Decision, DecisionLog,
    DecisionLogEntry, NO_ADAPTER_PROVENANCE,
};
use semver::{Version as SemverVersion, VersionReq};
use serde::Deserialize;

struct NullLog;

impl DecisionLog for NullLog {
    fn log(&self, _entry: DecisionLogEntry) {}
}

pub(crate) struct PreparedGraphRuntime {
    pub graph_id: String,
    pub runtime_provenance: String,
    pub expanded: ExpandedGraph,
    pub catalog: CorePrimitiveCatalog,
    pub registries: CoreRegistries,
}

pub(crate) fn prepare_graph_runtime(
    graph_path: &Path,
    cluster_paths: &[PathBuf],
) -> Result<PreparedGraphRuntime, String> {
    let root = parse_graph_file(graph_path)?;
    let clusters = load_cluster_tree(graph_path, &root, cluster_paths)?;
    let loader = PreloadedClusterLoader::new(clusters);

    let catalog = build_core_catalog();
    let registries = core_registries().map_err(|err| format!("core registries: {err:?}"))?;
    let expanded = expand(&root, &loader, &catalog)
        .map_err(|err| format!("graph expansion failed: {}", render_error_info(&err)))?;
    let runtime_provenance =
        compute_runtime_provenance(RuntimeProvenanceScheme::Rpv1, &root.id, &expanded, &catalog)
            .map_err(|err| format!("runtime provenance compute failed: {err}"))?;

    Ok(PreparedGraphRuntime {
        graph_id: root.id,
        runtime_provenance,
        expanded,
        catalog,
        registries,
    })
}

pub fn run_graph_command(graph_path: &Path, args: &[String]) -> Result<(), String> {
    let opts = parse_run_options(args)?;
    let prepared = prepare_graph_runtime(graph_path, &opts.cluster_paths)?;

    if opts.direct {
        return run_direct(&prepared.expanded, &prepared.catalog, &prepared.registries);
    }

    let dependency_summary =
        scan_adapter_dependencies(&prepared.expanded, &prepared.catalog, &prepared.registries)?;
    let PreparedGraphRuntime {
        graph_id,
        runtime_provenance,
        expanded,
        catalog,
        registries,
    } = prepared;
    run_canonical(
        graph_path,
        &graph_id,
        &runtime_provenance,
        opts,
        dependency_summary,
        expanded,
        catalog,
        registries,
    )
}

fn run_direct(
    expanded: &ergo_runtime::cluster::ExpandedGraph,
    catalog: &CorePrimitiveCatalog,
    registries: &CoreRegistries,
) -> Result<(), String> {
    let refs = Registries {
        sources: &registries.sources,
        computes: &registries.computes,
        triggers: &registries.triggers,
        actions: &registries.actions,
    };
    let ctx = ExecutionContext::default();
    let report =
        ergo_runtime::runtime::run(expanded, catalog, &refs, &ctx).map_err(format_runtime_error)?;

    print_outputs(&report.outputs);
    Ok(())
}

fn run_canonical(
    graph_path: &Path,
    graph_id: &str,
    runtime_provenance: &str,
    opts: RunGraphOptions,
    dependency_summary: AdapterDependencySummary,
    expanded: ergo_runtime::cluster::ExpandedGraph,
    catalog: CorePrimitiveCatalog,
    registries: CoreRegistries,
) -> Result<(), String> {
    let fixture_path = opts.fixture_path.as_ref().ok_or_else(|| {
        render_cli_error(
            &CliErrorInfo::new(
                "cli.missing_required_option",
                "canonical run requires --fixture <events.jsonl>",
            )
            .with_rule_id("RUN-CANON-1")
            .with_where("run options")
            .with_fix("provide --fixture <events.jsonl> or use --direct for debug mode"),
        )
    })?;

    let mut adapter_bound = false;
    let mut adapter_provides = AdapterProvides::default();
    let mut event_binder: Option<EventBinder> = None;

    if let Some(adapter_path) = &opts.adapter_path {
        let adapter = parse_adapter_manifest(adapter_path)?;
        ergo_adapter::validate_adapter(&adapter).map_err(|err| {
            cli_error_from_error_info(
                "adapter.invalid_manifest",
                "adapter manifest validation failed",
                format!("path '{}'", adapter_path.display()),
                &err,
            )
        })?;
        adapter_provides = AdapterProvides::from_manifest(&adapter);
        validate_adapter_composition(&expanded, &catalog, &registries, &adapter_provides)?;
        event_binder = Some(compile_event_binder(&adapter_provides).map_err(|err| {
            render_cli_error(
                &CliErrorInfo::new(
                    "adapter.binder_compile_failed",
                    "adapter event binder compilation failed",
                )
                .with_where(format!("path '{}'", adapter_path.display()))
                .with_fix("fix adapter event schema/mapping and retry")
                .with_detail(err.to_string()),
            )
        })?);
        adapter_bound = true;
    } else if dependency_summary.requires_adapter {
        return Err(format_missing_adapter_error(&dependency_summary));
    }

    let adapter_provenance = if adapter_bound {
        adapter_provides.adapter_fingerprint.clone()
    } else {
        NO_ADAPTER_PROVENANCE.to_string()
    };

    let runtime = RuntimeHandle::new(
        Arc::new(expanded),
        Arc::new(catalog),
        Arc::new(registries),
        adapter_provides,
    );

    let mut session = CapturingSession::new_with_provenance(
        GraphId::new(graph_id.to_string()),
        Constraints::default(),
        NullLog,
        runtime,
        adapter_provenance,
        runtime_provenance.to_string(),
    );

    let items = fixture::parse_fixture(fixture_path).map_err(|err| {
        format!(
            "Failed to parse fixture '{}': {err}",
            fixture_path.display()
        )
    })?;

    let mut episodes: Vec<(String, usize)> = Vec::new();
    let mut current_episode: Option<usize> = None;
    let mut event_counter = 0usize;

    for item in items {
        match item {
            fixture::FixtureItem::EpisodeStart { label } => {
                episodes.push((label, 0));
                current_episode = Some(episodes.len() - 1);
            }
            fixture::FixtureItem::Event {
                id,
                kind,
                payload,
                semantic_kind,
            } => {
                if current_episode.is_none() {
                    let label = format!("E{}", episodes.len() + 1);
                    episodes.push((label, 0));
                    current_episode = Some(episodes.len() - 1);
                }

                event_counter += 1;
                let event_id = id.unwrap_or_else(|| format!("fixture_evt_{}", event_counter));
                let event = if adapter_bound {
                    let binder = event_binder
                        .as_ref()
                        .expect("event binder must exist when adapter is bound");
                    let semantic = semantic_kind.ok_or_else(|| {
                        render_cli_error(
                            &CliErrorInfo::new(
                                "fixture.semantic_kind_missing",
                                format!(
                                    "fixture event '{}' is missing semantic_kind in adapter-bound canonical run",
                                    event_id
                                ),
                            )
                            .with_where(format!("fixture event '{}'", event_id))
                            .with_fix(
                                "add semantic_kind to each fixture event when running with --adapter",
                            ),
                        )
                    })?;
                    let payload_value = payload
                        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));

                    bind_semantic_event_with_binder(
                        binder,
                        EventId::new(event_id.clone()),
                        kind,
                        EventTime::default(),
                        &semantic,
                        payload_value,
                    )
                    .map_err(|err| {
                        render_cli_error(
                            &CliErrorInfo::new(
                                "fixture.semantic_binding_failed",
                                format!("fixture event '{}' binding failed", event_id),
                            )
                            .with_where(format!("fixture event '{}'", event_id))
                            .with_fix(
                                "fix fixture payload/semantic_kind to match adapter event schema",
                            )
                            .with_detail(err.to_string()),
                        )
                    })?
                } else {
                    if semantic_kind.is_some() {
                        return Err(render_cli_error(
                            &CliErrorInfo::new(
                                "fixture.unexpected_semantic_kind",
                                format!(
                                    "fixture event '{}' set semantic_kind but canonical run is not adapter-bound",
                                    event_id
                                ),
                            )
                            .with_where(format!("fixture event '{}'", event_id))
                            .with_fix("remove semantic_kind or run with --adapter <adapter.yaml>"),
                        ));
                    }
                    event_from_fixture_payload(&event_id, kind, payload)?
                };

                session.on_event(event);
                let episode_index = current_episode.expect("episode index set");
                episodes[episode_index].1 += 1;
            }
        }
    }

    if episodes.is_empty() {
        return Err("fixture contained no episodes".to_string());
    }

    if event_counter == 0 {
        return Err("fixture contained no events".to_string());
    }

    if let Some((label, _)) = episodes.iter().find(|(_, count)| *count == 0) {
        return Err(format!("episode '{}' has no events", label));
    }

    let bundle = session.into_bundle();
    let capture_path = opts
        .capture_output
        .clone()
        .unwrap_or_else(|| default_capture_output_path(graph_path));
    let capture_style = if opts.pretty_capture {
        CaptureJsonStyle::Pretty
    } else {
        CaptureJsonStyle::Compact
    };
    write_capture_bundle(&capture_path, &bundle, capture_style)?;

    let invoked = bundle
        .decisions
        .iter()
        .filter(|record| record.decision == Decision::Invoke)
        .count();
    let deferred = bundle
        .decisions
        .iter()
        .filter(|record| record.decision == Decision::Defer)
        .count();

    println!(
        "episodes={} events={} invoked={} deferred={}",
        episodes.len(),
        event_counter,
        invoked,
        deferred
    );
    println!("capture artifact: {}", capture_path.display());
    Ok(())
}

fn event_from_fixture_payload(
    event_id: &str,
    kind: ExternalEventKind,
    payload: Option<serde_json::Value>,
) -> Result<ExternalEvent, String> {
    let event_id_value = EventId::new(event_id.to_string());
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

fn default_capture_output_path(graph_path: &Path) -> PathBuf {
    let stem = graph_path
        .file_stem()
        .map(|part| part.to_string_lossy().to_string())
        .filter(|part| !part.is_empty())
        .unwrap_or_else(|| "graph".to_string());
    let sanitized = sanitize_filename_component(&stem);
    PathBuf::from("target").join(format!("{sanitized}-capture.json"))
}

fn sanitize_filename_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        "graph".to_string()
    } else {
        out
    }
}

#[derive(Debug, Default)]
struct RunGraphOptions {
    adapter_path: Option<PathBuf>,
    fixture_path: Option<PathBuf>,
    capture_output: Option<PathBuf>,
    pretty_capture: bool,
    cluster_paths: Vec<PathBuf>,
    direct: bool,
}

fn parse_run_options(args: &[String]) -> Result<RunGraphOptions, String> {
    let mut options = RunGraphOptions::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-a" | "--adapter" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| {
                        render_cli_error(
                            &CliErrorInfo::new(
                                "cli.missing_option_value",
                                format!("{} requires a path", args[i]),
                            )
                            .with_where(format!("arg '{}'", args[i]))
                            .with_fix("provide -a <adapter.yaml>"),
                        )
                    })?;
                options.adapter_path = Some(PathBuf::from(value));
                i += 2;
            }
            "-f" | "--fixture" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| {
                        render_cli_error(
                            &CliErrorInfo::new(
                                "cli.missing_option_value",
                                format!("{} requires a path", args[i]),
                            )
                            .with_where(format!("arg '{}'", args[i]))
                            .with_fix("provide -f <events.jsonl>"),
                        )
                    })?;
                options.fixture_path = Some(PathBuf::from(value));
                i += 2;
            }
            "-o" | "--capture" | "--capture-output" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| {
                        render_cli_error(
                            &CliErrorInfo::new(
                                "cli.missing_option_value",
                                format!("{} requires a path", args[i]),
                            )
                            .with_where(format!("arg '{}'", args[i]))
                            .with_fix("provide -o <path>"),
                        )
                    })?;
                options.capture_output = Some(PathBuf::from(value));
                i += 2;
            }
            "-p" | "--pretty-capture" => {
                options.pretty_capture = true;
                i += 1;
            }
            "-d" | "--direct" => {
                options.direct = true;
                i += 1;
            }
            "--cluster-path" | "--search-path" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| {
                        render_cli_error(
                            &CliErrorInfo::new(
                                "cli.missing_option_value",
                                format!("{} requires a path", args[i]),
                            )
                            .with_where(format!("arg '{}'", args[i]))
                            .with_fix("provide a directory path"),
                        )
                    })?;
                options.cluster_paths.push(PathBuf::from(value));
                i += 2;
            }
            other => {
                return Err(render_cli_error(
                    &CliErrorInfo::new("cli.invalid_option", format!("unknown run option '{other}'"))
                        .with_where(format!("arg '{other}'"))
                        .with_fix("use -a|--adapter, -f|--fixture, -o|--capture|--capture-output, -p|--pretty-capture, -d|--direct, --cluster-path, or --search-path"),
                ))
            }
        }
    }

    if options.direct {
        if options.fixture_path.is_some() {
            return Err(render_cli_error(
                &CliErrorInfo::new(
                    "cli.conflicting_options",
                    "--direct cannot be combined with --fixture",
                )
                .with_where("arg '--direct' and arg '--fixture'")
                .with_fix("remove --direct for canonical mode or remove --fixture for direct mode"),
            ));
        }
        if options.adapter_path.is_some() {
            return Err(render_cli_error(
                &CliErrorInfo::new(
                    "cli.conflicting_options",
                    "--direct cannot be combined with --adapter",
                )
                .with_where("arg '--direct' and arg '--adapter'")
                .with_fix("remove --direct for canonical mode or remove --adapter for direct mode"),
            ));
        }
        if options.capture_output.is_some() {
            return Err(render_cli_error(
                &CliErrorInfo::new(
                    "cli.conflicting_options",
                    "--direct cannot be combined with --capture-output",
                )
                .with_where("arg '--direct' and arg '--capture-output'")
                .with_fix("remove --direct or remove --capture-output"),
            ));
        }
        if options.pretty_capture {
            return Err(render_cli_error(
                &CliErrorInfo::new(
                    "cli.conflicting_options",
                    "--direct cannot be combined with --pretty-capture",
                )
                .with_where("arg '--direct' and arg '--pretty-capture'")
                .with_fix("remove --direct or remove --pretty-capture"),
            ));
        }
        return Ok(options);
    }

    if options.fixture_path.is_none() {
        return Err(render_cli_error(
            &CliErrorInfo::new(
                "cli.missing_required_option",
                "canonical run requires --fixture <events.jsonl>",
            )
            .with_rule_id("RUN-CANON-1")
            .with_where("run options")
            .with_fix(
                "provide --fixture <events.jsonl> or use --direct for one-shot debug execution",
            ),
        ));
    }

    Ok(options)
}

#[derive(Debug, Default)]
struct AdapterDependencySummary {
    requires_adapter: bool,
    required_context_nodes: Vec<String>,
    write_nodes: Vec<String>,
}

fn scan_adapter_dependencies(
    expanded: &ergo_runtime::cluster::ExpandedGraph,
    catalog: &CorePrimitiveCatalog,
    registries: &CoreRegistries,
) -> Result<AdapterDependencySummary, String> {
    let mut summary = AdapterDependencySummary::default();

    for (runtime_id, node) in &expanded.nodes {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| {
                format!(
                    "missing catalog metadata for primitive '{}@{}'",
                    node.implementation.impl_id, node.implementation.version
                )
            })?;

        match meta.kind {
            PrimitiveKind::Source => {
                let source = registries
                    .sources
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| {
                        format!(
                            "source '{}' missing in core registry",
                            node.implementation.impl_id
                        )
                    })?;
                if source
                    .manifest()
                    .requires
                    .context
                    .iter()
                    .any(|req| req.required)
                {
                    summary.required_context_nodes.push(runtime_id.clone());
                }
            }
            PrimitiveKind::Action => {
                let action = registries
                    .actions
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| {
                        format!(
                            "action '{}' missing in core registry",
                            node.implementation.impl_id
                        )
                    })?;
                if !action.manifest().effects.writes.is_empty() {
                    summary.write_nodes.push(runtime_id.clone());
                }
            }
            _ => {}
        }
    }

    summary.requires_adapter =
        !summary.required_context_nodes.is_empty() || !summary.write_nodes.is_empty();
    Ok(summary)
}

fn format_missing_adapter_error(summary: &AdapterDependencySummary) -> String {
    let where_field = if let Some(node) = summary
        .required_context_nodes
        .first()
        .or_else(|| summary.write_nodes.first())
    {
        format!("node '{}'", node)
    } else {
        "graph dependency scan".to_string()
    };

    let mut info = CliErrorInfo::new(
        "adapter.required_for_graph",
        "graph requires adapter capabilities but no --adapter was provided",
    )
    .with_rule_id("RUN-CANON-2")
    .with_where(where_field)
    .with_fix("provide --adapter <adapter.yaml> for canonical run");

    if !summary.required_context_nodes.is_empty() {
        info = info.with_detail(format!(
            "required source context at node(s): {}",
            summary.required_context_nodes.join(", ")
        ));
    }

    if !summary.write_nodes.is_empty() {
        info = info.with_detail(format!(
            "action writes at node(s): {}",
            summary.write_nodes.join(", ")
        ));
    }

    render_cli_error(&info)
}

pub(crate) fn validate_adapter_composition(
    expanded: &ergo_runtime::cluster::ExpandedGraph,
    catalog: &CorePrimitiveCatalog,
    registries: &CoreRegistries,
    provides: &AdapterProvides,
) -> Result<(), String> {
    validate_capture_format(&provides.capture_format_version)
        .map_err(|err| format!("adapter composition failed: {}", render_error_info(&err)))?;

    for (runtime_id, node) in &expanded.nodes {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| {
                format!(
                    "missing catalog metadata for primitive '{}@{}'",
                    node.implementation.impl_id, node.implementation.version
                )
            })?;
        match meta.kind {
            PrimitiveKind::Source => {
                let source = registries
                    .sources
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| {
                        format!(
                            "source '{}' missing in core registry",
                            node.implementation.impl_id
                        )
                    })?;
                validate_source_adapter_composition(&source.manifest().requires, provides)
                    .map_err(|err| {
                        format!(
                            "source composition failed for node '{}': {}",
                            runtime_id,
                            render_error_info(&err)
                        )
                    })?;
            }
            PrimitiveKind::Action => {
                let action = registries
                    .actions
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| {
                        format!(
                            "action '{}' missing in core registry",
                            node.implementation.impl_id
                        )
                    })?;
                validate_action_adapter_composition(&action.manifest().effects, provides).map_err(
                    |err| {
                        format!(
                            "action composition failed for node '{}': {}",
                            runtime_id,
                            render_error_info(&err)
                        )
                    },
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}
fn print_outputs(outputs: &HashMap<String, RuntimeValue>) {
    if outputs.is_empty() {
        println!("outputs: {{}}");
        return;
    }

    let mut keys: Vec<&String> = outputs.keys().collect();
    keys.sort();
    println!("outputs:");
    for key in keys {
        if let Some(value) = outputs.get(key) {
            println!("  {key}: {}", format_runtime_value(value));
        }
    }
}

fn format_runtime_value(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Number(n) => n.to_string(),
        RuntimeValue::Series(items) => format!("{items:?}"),
        RuntimeValue::Bool(b) => b.to_string(),
        RuntimeValue::String(s) => s.clone(),
        RuntimeValue::Event(RuntimeEvent::Trigger(event)) => format!("Trigger::{event:?}"),
        RuntimeValue::Event(RuntimeEvent::Action(outcome)) => format!("Action::{outcome:?}"),
    }
}

fn format_runtime_error(err: RuntimeError) -> String {
    match err {
        RuntimeError::Validation(err) => {
            format!("runtime validation failed: {}", render_error_info(&err))
        }
        RuntimeError::Execution(err) => {
            format!("runtime execution failed: {}", render_error_info(&err))
        }
    }
}

#[derive(Clone)]
struct PreloadedClusterLoader {
    clusters: HashMap<(String, Version), ClusterDefinition>,
}

impl PreloadedClusterLoader {
    fn new(clusters: HashMap<(String, Version), ClusterDefinition>) -> Self {
        Self { clusters }
    }
}

impl ClusterLoader for PreloadedClusterLoader {
    fn load(&self, id: &str, version: &Version) -> Option<ClusterDefinition> {
        self.clusters
            .get(&(id.to_string(), version.clone()))
            .cloned()
    }
}

impl ClusterVersionIndex for PreloadedClusterLoader {
    fn available_versions(&self, id: &str) -> Vec<Version> {
        let mut versions = self
            .clusters
            .keys()
            .filter_map(|(candidate_id, version)| {
                if candidate_id == id {
                    Some(version.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        versions.sort();
        versions
    }
}

fn load_cluster_tree(
    root_path: &Path,
    root: &ClusterDefinition,
    search_paths: &[PathBuf],
) -> Result<HashMap<(String, Version), ClusterDefinition>, String> {
    let mut builder = ClusterTreeBuilder {
        clusters: HashMap::new(),
        cluster_sources: HashMap::new(),
        visiting_paths: HashSet::new(),
        visiting_keys: HashSet::new(),
        search_paths: search_paths.to_vec(),
    };
    builder.visit(root_path, root.clone())?;
    Ok(builder.clusters)
}

struct ClusterTreeBuilder {
    clusters: HashMap<(String, Version), ClusterDefinition>,
    cluster_sources: HashMap<(String, Version), PathBuf>,
    visiting_paths: HashSet<PathBuf>,
    visiting_keys: HashSet<(String, Version)>,
    search_paths: Vec<PathBuf>,
}

impl ClusterTreeBuilder {
    fn visit(&mut self, path: &Path, def: ClusterDefinition) -> Result<(), String> {
        let canonical = canonicalize_or_self(path);
        let cluster_key = (def.id.clone(), def.version.clone());

        if let Some(existing_path) = self.cluster_sources.get(&cluster_key) {
            if existing_path != &canonical {
                return Err(format!(
                    "cluster '{}@{}' is defined by multiple files: '{}' and '{}'",
                    def.id,
                    def.version,
                    existing_path.display(),
                    canonical.display()
                ));
            }
        } else {
            self.cluster_sources
                .insert(cluster_key.clone(), canonical.clone());
        }

        if !self.visiting_paths.insert(canonical.clone()) {
            return Err(format!(
                "circular cluster reference detected at '{}'",
                path.display()
            ));
        }
        if !self.visiting_keys.insert(cluster_key.clone()) {
            return Err(format!(
                "circular cluster reference detected for '{}@{}' at '{}'",
                def.id,
                def.version,
                path.display()
            ));
        }

        self.clusters
            .entry(cluster_key.clone())
            .or_insert_with(|| def.clone());

        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        for node in def.nodes.values() {
            let NodeKind::Cluster {
                cluster_id,
                version,
            } = &node.kind
            else {
                continue;
            };
            let cluster_paths = resolve_cluster_paths(base_dir, cluster_id, &self.search_paths);
            if cluster_paths.is_empty() {
                return Err(format!(
                    "missing cluster file for '{}@{}' referenced by node '{}' in '{}'",
                    cluster_id,
                    version,
                    node.id,
                    path.display()
                ));
            }

            for cluster_path in cluster_paths {
                let nested = parse_graph_file(&cluster_path).map_err(|err| {
                    format!(
                        "failed parsing nested cluster '{}@{}' at '{}': {}",
                        cluster_id,
                        version,
                        cluster_path.display(),
                        err
                    )
                })?;

                if nested.id != *cluster_id {
                    return Err(format!(
                        "cluster id mismatch in '{}': expected '{}', found '{}'",
                        cluster_path.display(),
                        cluster_id,
                        nested.id
                    ));
                }

                if !selector_matches_version(version, &nested.version)? {
                    continue;
                }

                self.visit(&cluster_path, nested)?;
            }
        }

        self.visiting_paths.remove(&canonical);
        self.visiting_keys.remove(&cluster_key);
        Ok(())
    }
}

fn resolve_cluster_paths(
    base_dir: &Path,
    cluster_id: &str,
    search_paths: &[PathBuf],
) -> Vec<PathBuf> {
    let filename = format!("{cluster_id}.yaml");

    let mut candidates = vec![
        base_dir.join(&filename),
        base_dir.join("clusters").join(&filename),
    ];

    for path in search_paths {
        candidates.push(path.join(&filename));
        candidates.push(path.join("clusters").join(&filename));
    }

    let mut seen = HashSet::new();
    let mut resolved = Vec::new();
    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        let canonical = canonicalize_or_self(&candidate);
        if seen.insert(canonical) {
            resolved.push(candidate);
        }
    }
    resolved
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn parse_graph_file(path: &Path) -> Result<ClusterDefinition, String> {
    let data = fs::read_to_string(path)
        .map_err(|err| format!("read graph '{}': {err}", path.display()))?;
    parse_graph_str(&data, path)
}

fn parse_graph_str(input: &str, source: &Path) -> Result<ClusterDefinition, String> {
    let raw: RawClusterDefinition = serde_yaml::from_str(input)
        .map_err(|err| format!("parse YAML '{}': {err}", source.display()))?;
    raw.into_cluster_definition()
        .map_err(|err| format!("graph '{}': {err}", source.display()))
}

#[derive(Debug, Deserialize)]
struct RawClusterDefinition {
    kind: String,
    id: String,
    version: serde_yaml::Value,
    nodes: HashMap<String, RawNodeSpec>,
    edges: Vec<RawEdge>,
    #[serde(default)]
    outputs: HashMap<String, String>,
    #[serde(default)]
    inputs: Vec<RawInputPortSpec>,
    #[serde(default)]
    parameters: Vec<RawParameterSpec>,
    #[serde(default, alias = "signature")]
    declared_signature: Option<RawSignature>,
}

impl RawClusterDefinition {
    fn into_cluster_definition(self) -> Result<ClusterDefinition, String> {
        if self.kind.to_ascii_lowercase() != "cluster" {
            return Err(format!("kind must be 'cluster', got '{}'", self.kind));
        }
        validate_general_identifier(&self.id, "cluster id", true)?;
        let version = parse_version_value(&self.version)?;

        let input_ports = self
            .inputs
            .into_iter()
            .map(RawInputPortSpec::into_input_port)
            .collect::<Result<Vec<_>, _>>()?;

        let input_names: HashSet<String> = input_ports
            .iter()
            .map(|input| input.maps_to.name.clone())
            .collect();

        let mut nodes = HashMap::new();
        for (node_id, raw_node) in self.nodes {
            validate_node_identifier(&node_id, "node id")?;
            let node = raw_node.into_node_instance(node_id.clone())?;
            nodes.insert(node_id, node);
        }

        let mut edges = Vec::with_capacity(self.edges.len());
        for raw_edge in self.edges {
            let parsed = raw_edge.into_parsed_edge()?;
            if let Some(external_name) = parsed.external_source.as_deref() {
                if !input_names.contains(external_name) {
                    return Err(format!(
                        "edge references external input '${}' that is not declared in inputs",
                        external_name
                    ));
                }
            } else if !nodes.contains_key(&parsed.edge.from.node_id) {
                if input_names.contains(&parsed.edge.from.node_id) {
                    return Err(format!(
                        "edge source '{}.{}' must use '${}' syntax for external inputs",
                        parsed.edge.from.node_id,
                        parsed.edge.from.port_name,
                        parsed.edge.from.node_id
                    ));
                }
                return Err(format!(
                    "edge source node '{}' is not declared in nodes",
                    parsed.edge.from.node_id
                ));
            }

            if !nodes.contains_key(&parsed.edge.to.node_id) {
                return Err(format!(
                    "edge target node '{}' is not declared in nodes",
                    parsed.edge.to.node_id
                ));
            }
            edges.push(parsed.edge);
        }

        let mut output_ports = Vec::with_capacity(self.outputs.len());
        for (name, target) in self.outputs {
            validate_general_identifier(&name, "output name", false)?;
            let (node_id, port_name) = parse_node_port_ref(&target, "output target")?;
            output_ports.push(OutputPortSpec {
                name,
                maps_to: OutputRef { node_id, port_name },
            });
        }

        let parameters = self
            .parameters
            .into_iter()
            .map(RawParameterSpec::into_parameter_spec)
            .collect::<Result<Vec<_>, _>>()?;

        let declared_signature = self
            .declared_signature
            .map(RawSignature::into_signature)
            .transpose()?;

        Ok(ClusterDefinition {
            id: self.id,
            version,
            nodes,
            edges,
            input_ports,
            output_ports,
            parameters,
            declared_signature,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawNodeSpec {
    #[serde(rename = "impl")]
    impl_ref: Option<String>,
    cluster: Option<String>,
    #[serde(default)]
    params: HashMap<String, RawParamBinding>,
}

impl RawNodeSpec {
    fn into_node_instance(self, node_id: String) -> Result<NodeInstance, String> {
        let kind = match (self.impl_ref, self.cluster) {
            (Some(impl_ref), None) => {
                let (impl_id, version) = parse_packed_id_version(&impl_ref, "impl")?;
                NodeKind::Impl { impl_id, version }
            }
            (None, Some(cluster_ref)) => {
                let (cluster_id, version) = parse_packed_id_version(&cluster_ref, "cluster")?;
                NodeKind::Cluster {
                    cluster_id,
                    version,
                }
            }
            (Some(_), Some(_)) => {
                return Err(format!(
                    "node '{}' must define exactly one of 'impl' or 'cluster'",
                    node_id
                ))
            }
            (None, None) => {
                return Err(format!(
                    "node '{}' must define exactly one of 'impl' or 'cluster'",
                    node_id
                ))
            }
        };

        let mut parameter_bindings = HashMap::new();
        for (name, raw_binding) in self.params {
            validate_general_identifier(&name, "parameter binding name", false)?;
            let binding = raw_binding.into_binding()?;
            parameter_bindings.insert(name, binding);
        }

        Ok(NodeInstance {
            id: node_id,
            kind,
            parameter_bindings,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawParamBinding {
    Exposed { exposed: String },
    Scalar(serde_yaml::Value),
}

impl RawParamBinding {
    fn into_binding(self) -> Result<ParameterBinding, String> {
        match self {
            Self::Exposed { exposed } => {
                validate_general_identifier(&exposed, "exposed parameter name", false)?;
                Ok(ParameterBinding::Exposed {
                    parent_param: exposed,
                })
            }
            Self::Scalar(value) => Ok(ParameterBinding::Literal {
                value: parse_untyped_parameter_value(&value)?,
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawEdge {
    Shorthand(String),
    Structured {
        from: RawEndpointRef,
        to: RawEndpointRef,
    },
}

#[derive(Debug)]
struct ParsedEdge {
    edge: Edge,
    external_source: Option<String>,
}

impl RawEdge {
    fn into_parsed_edge(self) -> Result<ParsedEdge, String> {
        match self {
            Self::Shorthand(text) => parse_shorthand_edge(&text),
            Self::Structured { from, to } => {
                let (from, external_source) = parse_output_endpoint_ref(from)?;
                let to = parse_input_endpoint_ref(to)?;
                Ok(ParsedEdge {
                    edge: Edge { from, to },
                    external_source,
                })
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawEndpointRef {
    NodePort { node: String, port: String },
    External { external: String },
}

fn parse_output_endpoint_ref(raw: RawEndpointRef) -> Result<(OutputRef, Option<String>), String> {
    match raw {
        RawEndpointRef::NodePort { node, port } => {
            validate_node_identifier(&node, "edge source node")?;
            validate_port_identifier(&port, "edge source port")?;
            Ok((
                OutputRef {
                    node_id: node,
                    port_name: port,
                },
                None,
            ))
        }
        RawEndpointRef::External { external } => {
            validate_general_identifier(&external, "external input name", true)?;
            Ok((
                OutputRef {
                    node_id: external.clone(),
                    port_name: external.clone(),
                },
                Some(external),
            ))
        }
    }
}

fn parse_input_endpoint_ref(raw: RawEndpointRef) -> Result<InputRef, String> {
    match raw {
        RawEndpointRef::NodePort { node, port } => {
            validate_node_identifier(&node, "edge target node")?;
            validate_port_identifier(&port, "edge target port")?;
            Ok(InputRef {
                node_id: node,
                port_name: port,
            })
        }
        RawEndpointRef::External { external } => Err(format!(
            "external input '{}' cannot appear as an edge target",
            external
        )),
    }
}

fn parse_shorthand_edge(text: &str) -> Result<ParsedEdge, String> {
    let (from_raw, to_raw) = text
        .split_once("->")
        .ok_or_else(|| format!("invalid edge '{}': expected 'from -> to'", text))?;
    let from_raw = from_raw.trim();
    let to_raw = to_raw.trim();
    if from_raw.is_empty() || to_raw.is_empty() {
        return Err(format!("invalid edge '{}': expected 'from -> to'", text));
    }

    let (from, external_source) = if let Some(external) = from_raw.strip_prefix('$') {
        let name = external.trim();
        validate_general_identifier(name, "external input name", true)?;
        (
            OutputRef {
                node_id: name.to_string(),
                port_name: name.to_string(),
            },
            Some(name.to_string()),
        )
    } else {
        let (node_id, port_name) = parse_node_port_ref(from_raw, "edge source")?;
        (OutputRef { node_id, port_name }, None)
    };

    if to_raw.starts_with('$') {
        return Err(format!(
            "invalid edge '{}': external inputs can only appear on the source side",
            text
        ));
    }

    let (node_id, port_name) = parse_node_port_ref(to_raw, "edge target")?;
    Ok(ParsedEdge {
        edge: Edge {
            from,
            to: InputRef { node_id, port_name },
        },
        external_source,
    })
}

#[derive(Debug, Deserialize)]
struct RawInputPortSpec {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    required: Option<bool>,
}

impl RawInputPortSpec {
    fn into_input_port(self) -> Result<InputPortSpec, String> {
        validate_general_identifier(&self.name, "input name", true)?;
        let ty = parse_value_type(&self.ty)?;
        let required = self.required.unwrap_or(true);
        Ok(InputPortSpec {
            name: self.name.clone(),
            maps_to: GraphInputPlaceholder {
                name: self.name,
                ty,
                required,
            },
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawParameterSpec {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    default: Option<serde_yaml::Value>,
    required: Option<bool>,
}

impl RawParameterSpec {
    fn into_parameter_spec(self) -> Result<ParameterSpec, String> {
        validate_general_identifier(&self.name, "parameter name", false)?;
        let ty = parse_parameter_type(&self.ty)?;
        let default = self
            .default
            .as_ref()
            .map(|value| parse_typed_parameter_value(value, &ty))
            .transpose()?;
        let required = self.required.unwrap_or(default.is_none());
        Ok(ParameterSpec {
            name: self.name,
            ty,
            default,
            required,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawSignature {
    kind: String,
    inputs: Vec<RawPortSpec>,
    outputs: Vec<RawPortSpec>,
    has_side_effects: bool,
    is_origin: bool,
}

impl RawSignature {
    fn into_signature(self) -> Result<Signature, String> {
        let kind = parse_boundary_kind(&self.kind)?;
        let inputs = self
            .inputs
            .into_iter()
            .map(RawPortSpec::into_port_spec)
            .collect::<Result<Vec<_>, _>>()?;
        let outputs = self
            .outputs
            .into_iter()
            .map(RawPortSpec::into_port_spec)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Signature {
            kind,
            inputs,
            outputs,
            has_side_effects: self.has_side_effects,
            is_origin: self.is_origin,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawPortSpec {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    cardinality: String,
    wireable: bool,
}

impl RawPortSpec {
    fn into_port_spec(self) -> Result<PortSpec, String> {
        validate_general_identifier(&self.name, "declared signature port name", false)?;
        let ty = parse_value_type(&self.ty)?;
        let cardinality = parse_cardinality(&self.cardinality)?;
        Ok(PortSpec {
            name: self.name,
            ty,
            cardinality,
            wireable: self.wireable,
        })
    }
}

fn version_migration_guidance() -> &'static str {
    "use strict semver (e.g. '1.2.3') or a semver constraint (e.g. '^1.2'); migrate legacy tags with tools/migrate_graph_versions.py --check"
}

fn normalize_numeric_semver(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.split('.').collect();
    if parts.is_empty() || parts.len() > 2 {
        return None;
    }
    if !parts
        .iter()
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    match parts.len() {
        1 => Some(format!("{}.0.0", parts[0])),
        2 => Some(format!("{}.{}.0", parts[0], parts[1])),
        _ => None,
    }
}

fn parse_exact_semver(value: &str, context: &str) -> Result<String, String> {
    SemverVersion::parse(value)
        .map(|version| version.to_string())
        .map_err(|_| {
            format!(
                "invalid {context} '{value}': expected strict semver (x.y.z); {}",
                version_migration_guidance()
            )
        })
}

fn parse_version_selector(value: &str, context: &str) -> Result<String, String> {
    if let Ok(version) = SemverVersion::parse(value) {
        return Ok(version.to_string());
    }
    VersionReq::parse(value)
        .map(|_| value.to_string())
        .map_err(|_| {
            format!(
                "invalid {context} '{value}': expected strict semver or semver constraint; {}",
                version_migration_guidance()
            )
        })
}

fn selector_matches_version(selector: &str, version: &str) -> Result<bool, String> {
    let candidate = SemverVersion::parse(version).map_err(|_| {
        format!("cluster discovery found non-semver version '{version}' in nested cluster file")
    })?;
    if let Ok(exact) = SemverVersion::parse(selector) {
        return Ok(candidate == exact);
    }
    let req = VersionReq::parse(selector)
        .map_err(|_| format!("invalid cluster selector '{selector}' during discovery"))?;
    Ok(req.matches(&candidate))
}

fn parse_packed_id_version(value: &str, field: &str) -> Result<(String, String), String> {
    let (id, version) = value.rsplit_once('@').ok_or_else(|| {
        format!(
            "invalid {} reference '{}': expected '<id>@<version>'",
            field, value
        )
    })?;
    if id.is_empty() || version.is_empty() {
        return Err(format!(
            "invalid {} reference '{}': expected non-empty id and version",
            field, value
        ));
    }
    validate_general_identifier(id, &format!("{field} id"), false)?;
    let version = parse_version_selector(version, &format!("{field} version selector"))?;
    Ok((id.to_string(), version))
}

fn parse_node_port_ref(value: &str, label: &str) -> Result<(String, String), String> {
    let (node, port) = value
        .split_once('.')
        .ok_or_else(|| format!("invalid {label} '{}': expected 'node.port'", value))?;
    if node.is_empty() || port.is_empty() || port.contains('.') {
        return Err(format!("invalid {label} '{}': expected 'node.port'", value));
    }
    validate_node_identifier(node, &format!("{label} node"))?;
    validate_port_identifier(port, &format!("{label} port"))?;
    Ok((node.to_string(), port.to_string()))
}

fn parse_version_value(value: &serde_yaml::Value) -> Result<String, String> {
    match value {
        serde_yaml::Value::String(s) => parse_exact_semver(s, "cluster version"),
        serde_yaml::Value::Number(n) => {
            let raw = n.to_string();
            let normalized = normalize_numeric_semver(&raw).ok_or_else(|| {
                format!(
                    "invalid cluster version number '{raw}': expected semver-like number (e.g. 1 or 1.2) or use a quoted semver string"
                )
            })?;
            parse_exact_semver(&normalized, "cluster version")
        }
        _ => Err(format!(
            "version must be a string or number, got '{value:?}'"
        )),
    }
}

fn parse_untyped_parameter_value(value: &serde_yaml::Value) -> Result<ParameterValue, String> {
    match value {
        serde_yaml::Value::Bool(b) => Ok(ParameterValue::Bool(*b)),
        serde_yaml::Value::Number(n) => {
            if let Some(int) = n.as_i64() {
                return Ok(ParameterValue::Int(int));
            }
            if let Some(number) = n.as_f64() {
                return Ok(ParameterValue::Number(number));
            }
            Err(format!("unsupported numeric parameter value '{n}'"))
        }
        serde_yaml::Value::String(s) => Ok(ParameterValue::String(s.clone())),
        _ => Err(format!(
            "parameter values must be scalar (number, bool, string) or {{exposed: ...}}, got '{value:?}'"
        )),
    }
}

fn parse_typed_parameter_value(
    value: &serde_yaml::Value,
    ty: &ParameterType,
) -> Result<ParameterValue, String> {
    match ty {
        ParameterType::Int => value
            .as_i64()
            .map(ParameterValue::Int)
            .ok_or_else(|| format!("expected Int default, got '{value:?}'")),
        ParameterType::Number => value
            .as_f64()
            .map(ParameterValue::Number)
            .ok_or_else(|| format!("expected Number default, got '{value:?}'")),
        ParameterType::Bool => value
            .as_bool()
            .map(ParameterValue::Bool)
            .ok_or_else(|| format!("expected Bool default, got '{value:?}'")),
        ParameterType::String => value
            .as_str()
            .map(|s| ParameterValue::String(s.to_string()))
            .ok_or_else(|| format!("expected String default, got '{value:?}'")),
        ParameterType::Enum => value
            .as_str()
            .map(|s| ParameterValue::Enum(s.to_string()))
            .ok_or_else(|| format!("expected Enum default (string), got '{value:?}'")),
    }
}

fn parse_value_type(raw: &str) -> Result<ValueType, String> {
    match raw.to_ascii_lowercase().as_str() {
        "number" => Ok(ValueType::Number),
        "series" => Ok(ValueType::Series),
        "bool" | "boolean" => Ok(ValueType::Bool),
        "event" => Ok(ValueType::Event),
        "string" => Ok(ValueType::String),
        _ => Err(format!("unknown value type '{}'", raw)),
    }
}

fn parse_parameter_type(raw: &str) -> Result<ParameterType, String> {
    match raw.to_ascii_lowercase().as_str() {
        "int" => Ok(ParameterType::Int),
        "number" => Ok(ParameterType::Number),
        "bool" | "boolean" => Ok(ParameterType::Bool),
        "string" => Ok(ParameterType::String),
        "enum" => Ok(ParameterType::Enum),
        _ => Err(format!("unknown parameter type '{}'", raw)),
    }
}

fn parse_cardinality(raw: &str) -> Result<Cardinality, String> {
    match raw.to_ascii_lowercase().as_str() {
        "single" => Ok(Cardinality::Single),
        "multiple" => Ok(Cardinality::Multiple),
        _ => Err(format!("unknown cardinality '{}'", raw)),
    }
}

fn parse_boundary_kind(raw: &str) -> Result<BoundaryKind, String> {
    match raw.to_ascii_lowercase().as_str() {
        "source_like" | "sourcelike" => Ok(BoundaryKind::SourceLike),
        "compute_like" | "computelike" => Ok(BoundaryKind::ComputeLike),
        "trigger_like" | "triggerlike" => Ok(BoundaryKind::TriggerLike),
        "action_like" | "actionlike" => Ok(BoundaryKind::ActionLike),
        _ => Err(format!("unknown boundary kind '{}'", raw)),
    }
}

fn validate_node_identifier(value: &str, label: &str) -> Result<(), String> {
    validate_general_identifier(value, label, true)
}

fn validate_port_identifier(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.contains('.') {
        return Err(format!("{label} must not contain '.'"));
    }
    if value.contains(' ') {
        return Err(format!("{label} must not contain spaces"));
    }
    Ok(())
}

fn validate_general_identifier(
    value: &str,
    label: &str,
    forbid_dollar: bool,
) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.contains('.') {
        return Err(format!("{label} must not contain '.'"));
    }
    if value.contains('@') {
        return Err(format!("{label} must not contain '@'"));
    }
    if value.contains(' ') {
        return Err(format!("{label} must not contain spaces"));
    }
    if forbid_dollar && value.contains('$') {
        return Err(format!("{label} must not contain '$'"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ergo_runtime::action::ActionOutcome;
    use ergo_supervisor::CaptureBundle;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn write_temp_yaml(name: &str, contents: &str) -> PathBuf {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ergo-graph-yaml-test-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join(name);
        fs::write(&path, contents).expect("write temp graph yaml");
        path
    }

    fn write_temp_tree(entries: &[(&str, &str)]) -> PathBuf {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "ergo-graph-yaml-tree-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&root).expect("create temp tree root");
        for (relative_path, contents) in entries {
            let full_path = root.join(relative_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).expect("create temp tree parent");
            }
            fs::write(&full_path, contents).expect("write temp tree file");
        }
        root
    }

    fn write_temp_fixture(name: &str, contents: &str) -> PathBuf {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ergo-graph-fixture-test-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&dir).expect("create temp fixture dir");
        let path = dir.join(name);
        fs::write(&path, contents).expect("write temp fixture");
        path
    }

    #[test]
    fn parse_run_options_supports_short_flags_and_capture_alias() -> Result<(), String> {
        let opts = parse_run_options(&[
            "-f".to_string(),
            "fixture.jsonl".to_string(),
            "-a".to_string(),
            "adapter.yaml".to_string(),
            "-o".to_string(),
            "capture-short.json".to_string(),
            "-p".to_string(),
            "--cluster-path".to_string(),
            "clusters".to_string(),
        ])?;
        assert_eq!(
            opts.fixture_path.as_deref(),
            Some(Path::new("fixture.jsonl"))
        );
        assert_eq!(
            opts.adapter_path.as_deref(),
            Some(Path::new("adapter.yaml"))
        );
        assert_eq!(
            opts.capture_output.as_deref(),
            Some(Path::new("capture-short.json"))
        );
        assert!(opts.pretty_capture);
        assert_eq!(opts.cluster_paths, vec![PathBuf::from("clusters")]);
        assert!(!opts.direct);

        let alias_opts = parse_run_options(&[
            "-f".to_string(),
            "fixture.jsonl".to_string(),
            "--capture".to_string(),
            "capture-alias.json".to_string(),
        ])?;
        assert_eq!(
            alias_opts.capture_output.as_deref(),
            Some(Path::new("capture-alias.json"))
        );

        let direct_opts = parse_run_options(&["-d".to_string()])?;
        assert!(direct_opts.direct);
        Ok(())
    }

    #[test]
    fn parse_run_options_keeps_long_flag_compatibility() -> Result<(), String> {
        let opts = parse_run_options(&[
            "--fixture".to_string(),
            "fixture.jsonl".to_string(),
            "--adapter".to_string(),
            "adapter.yaml".to_string(),
            "--capture-output".to_string(),
            "capture-long.json".to_string(),
            "--pretty-capture".to_string(),
        ])?;
        assert_eq!(
            opts.fixture_path.as_deref(),
            Some(Path::new("fixture.jsonl"))
        );
        assert_eq!(
            opts.adapter_path.as_deref(),
            Some(Path::new("adapter.yaml"))
        );
        assert_eq!(
            opts.capture_output.as_deref(),
            Some(Path::new("capture-long.json"))
        );
        assert!(opts.pretty_capture);
        Ok(())
    }

    #[test]
    fn parse_run_options_unknown_flag_is_actionable() {
        let err =
            parse_run_options(&["--bogus".to_string()]).expect_err("unknown run flag should fail");
        assert!(
            err.contains("code: cli.invalid_option")
                && err.contains("where: arg '--bogus'")
                && err.contains("fix: use -a|--adapter, -f|--fixture"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parses_graph_with_external_input_edge() {
        let yaml = r#"
kind: cluster
id: threshold_gate
version: "0.1.0"

nodes:
  gt:
    impl: gt@0.1.0

edges:
  - $threshold -> gt.b

inputs:
  - name: threshold
    type: number
    required: true

outputs:
  result: gt.result
"#;
        let parsed = parse_graph_str(yaml, Path::new("inline.yaml")).expect("parse graph");
        assert_eq!(parsed.input_ports.len(), 1);
        assert_eq!(parsed.edges.len(), 1);
        assert_eq!(parsed.edges[0].from.node_id, "threshold");
        assert_eq!(parsed.edges[0].from.port_name, "threshold");
    }

    #[test]
    fn rejects_undeclared_external_input_reference() {
        let yaml = r#"
kind: cluster
id: bad
version: "0.1.0"
nodes:
  gt:
    impl: gt@0.1.0
edges:
  - $threshold -> gt.b
outputs: {}
"#;
        let err = parse_graph_str(yaml, Path::new("inline.yaml")).expect_err("parse should fail");
        assert!(err.contains("not declared in inputs"), "err: {err}");
    }

    #[test]
    fn rejects_external_input_source_without_dollar_prefix() {
        let yaml = r#"
kind: cluster
id: bad
version: "0.1.0"
nodes:
  gt:
    impl: gt@0.1.0
edges:
  - threshold.value -> gt.b
inputs:
  - name: threshold
    type: number
outputs:
  result: gt.result
"#;
        let err = parse_graph_str(yaml, Path::new("inline.yaml")).expect_err("parse should fail");
        assert!(
            err.contains("must use '$threshold' syntax"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_unknown_edge_source_node() {
        let yaml = r#"
kind: cluster
id: bad
version: "0.1.0"
nodes:
  gt:
    impl: gt@0.1.0
edges:
  - nope.value -> gt.b
outputs:
  result: gt.result
"#;
        let err = parse_graph_str(yaml, Path::new("inline.yaml")).expect_err("parse should fail");
        assert!(
            err.contains("source node 'nope' is not declared"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_unknown_edge_target_node() {
        let yaml = r#"
kind: cluster
id: bad
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 1
edges:
  - src.value -> missing.a
outputs:
  out: src.value
"#;
        let err = parse_graph_str(yaml, Path::new("inline.yaml")).expect_err("parse should fail");
        assert!(
            err.contains("target node 'missing' is not declared"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn canonical_run_requires_fixture() {
        let graph = r#"
kind: cluster
id: basic
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 1
edges: []
outputs:
  value_out: src.value
"#;
        let graph_path = write_temp_yaml("basic_no_fixture.yaml", graph);
        let err = run_graph_command(&graph_path, &[]).expect_err("fixture should be required");
        assert!(
            err.contains("canonical run requires --fixture"),
            "unexpected error: {err}"
        );
        assert!(
            err.contains("code: cli.missing_required_option")
                && err.contains("rule: RUN-CANON-1")
                && err.contains("where: run options")
                && err.contains("fix: provide --fixture"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn direct_mode_rejects_fixture_adapter_and_pretty_flags() {
        let graph = r#"
kind: cluster
id: basic
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 1
edges: []
outputs:
  value_out: src.value
"#;
        let graph_path = write_temp_yaml("basic_direct_flags.yaml", graph);

        let args_with_fixture = vec![
            "--direct".to_string(),
            "--fixture".to_string(),
            "fixture.jsonl".to_string(),
        ];
        let err = run_graph_command(&graph_path, &args_with_fixture)
            .expect_err("fixture must be rejected");
        assert!(
            err.contains("--direct cannot be combined with --fixture"),
            "unexpected error: {err}"
        );
        assert!(
            err.contains("code: cli.conflicting_options")
                && err.contains("where: arg '--direct' and arg '--fixture'")
                && err.contains("fix: remove --direct"),
            "unexpected error: {err}"
        );

        let args_with_adapter = vec![
            "--direct".to_string(),
            "--adapter".to_string(),
            "adapter.yaml".to_string(),
        ];
        let err = run_graph_command(&graph_path, &args_with_adapter)
            .expect_err("adapter must be rejected");
        assert!(
            err.contains("--direct cannot be combined with --adapter"),
            "unexpected error: {err}"
        );

        let args_with_pretty = vec!["--direct".to_string(), "--pretty-capture".to_string()];
        let err = run_graph_command(&graph_path, &args_with_pretty)
            .expect_err("pretty-capture must be rejected");
        assert!(
            err.contains("--direct cannot be combined with --pretty-capture"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn canonical_adapter_independent_graph_runs_without_adapter() {
        let graph = r#"
kind: cluster
id: basic
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#;
        let graph_path = write_temp_yaml("basic_canonical.yaml", graph);
        let fixture_path = write_temp_fixture(
            "basic.fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );
        let args = vec![
            "--fixture".to_string(),
            fixture_path.to_string_lossy().to_string(),
        ];
        run_graph_command(&graph_path, &args)
            .expect("adapter-independent canonical graph should run");
    }

    #[test]
    fn canonical_no_adapter_rejects_semantic_kind_in_fixture() {
        let graph = r#"
kind: cluster
id: basic
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#;
        let graph_path = write_temp_yaml("basic_semantic_kind_reject.yaml", graph);
        let fixture_path = write_temp_fixture(
            "basic_semantic_kind_reject.fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"semantic_kind\":\"PriceTick\"}}\n",
        );
        let args = vec![
            "--fixture".to_string(),
            fixture_path.to_string_lossy().to_string(),
        ];

        let err = run_graph_command(&graph_path, &args)
            .expect_err("semantic_kind should be rejected when no adapter is bound");
        assert!(
            err.contains("semantic_kind") && err.contains("run with --adapter"),
            "unexpected error: {err}"
        );
        assert!(
            err.contains("code: fixture.unexpected_semantic_kind")
                && err.contains("where: fixture event"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn adapter_dependent_graph_without_adapter_errors() {
        let graph = r#"
kind: cluster
id: basic
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#;
        let graph_path = write_temp_yaml("basic_requires_adapter.yaml", graph);
        let root = parse_graph_file(&graph_path).expect("parse graph");
        let loader = PreloadedClusterLoader::new(HashMap::new());
        let catalog = build_core_catalog();
        let registries = core_registries().expect("core registries");
        let expanded = expand(&root, &loader, &catalog).expect("expand graph");

        let opts = RunGraphOptions {
            fixture_path: Some(PathBuf::from("unused.fixture.jsonl")),
            ..RunGraphOptions::default()
        };
        let dependency = AdapterDependencySummary {
            requires_adapter: true,
            required_context_nodes: vec!["src".to_string()],
            write_nodes: Vec::new(),
        };

        let err = run_canonical(
            &graph_path,
            &root.id,
            "rpv1:sha256:test",
            opts,
            dependency,
            expanded,
            catalog,
            registries,
        )
        .expect_err("missing adapter should be rejected");
        assert!(
            err.contains("no --adapter was provided"),
            "unexpected error: {err}"
        );
        assert!(
            err.contains("rule: RUN-CANON-2")
                && err.contains("fix: provide --adapter <adapter.yaml>")
                && err.contains("detail: required source context"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn canonical_run_writes_capture_with_none_provenance() {
        let graph = r#"
kind: cluster
id: basic
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#;
        let graph_path = write_temp_yaml("basic_capture.yaml", graph);
        let fixture_path = write_temp_fixture(
            "basic_capture.fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );
        let output_path = write_temp_yaml("capture_output.json", "{}");

        let args = vec![
            "--fixture".to_string(),
            fixture_path.to_string_lossy().to_string(),
            "--capture-output".to_string(),
            output_path.to_string_lossy().to_string(),
        ];
        run_graph_command(&graph_path, &args).expect("canonical run should produce capture");

        let raw = fs::read_to_string(&output_path).expect("capture file should be readable");
        assert!(
            raw.matches('\n').count() == 1,
            "default capture should be compact single-line JSON"
        );
        let bundle: CaptureBundle = serde_json::from_str(&raw).expect("capture bundle parses");
        assert_eq!(bundle.adapter_provenance, NO_ADAPTER_PROVENANCE);
    }

    #[test]
    fn canonical_run_pretty_capture_writes_multiline_output() {
        let graph = r#"
kind: cluster
id: basic
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#;
        let graph_path = write_temp_yaml("basic_capture_pretty.yaml", graph);
        let fixture_path = write_temp_fixture(
            "basic_capture_pretty.fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        );
        let output_path = write_temp_yaml("capture_output_pretty.json", "{}");

        let args = vec![
            "--fixture".to_string(),
            fixture_path.to_string_lossy().to_string(),
            "--capture-output".to_string(),
            output_path.to_string_lossy().to_string(),
            "--pretty-capture".to_string(),
        ];
        run_graph_command(&graph_path, &args).expect("canonical run should produce capture");

        let raw = fs::read_to_string(&output_path).expect("capture file should be readable");
        assert!(
            raw.matches('\n').count() > 1,
            "pretty capture should be multiline JSON"
        );
        let bundle: CaptureBundle = serde_json::from_str(&raw).expect("capture bundle parses");
        assert_eq!(bundle.adapter_provenance, NO_ADAPTER_PROVENANCE);
    }

    #[test]
    fn run_graph_command_executes_simple_graph() {
        let graph = r#"
kind: cluster
id: basic
version: "0.1.0"

nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5

edges: []

outputs:
  value_out: src.value
"#;
        let graph_path = write_temp_yaml("basic.yaml", graph);
        let args = vec!["--direct".to_string()];
        run_graph_command(&graph_path, &args).expect("graph should run");
    }

    #[test]
    fn run_graph_command_allows_boundary_output_from_cluster_node() {
        let root = write_temp_tree(&[
            (
                "child.yaml",
                r#"
kind: cluster
id: child
version: "0.1.0"

nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 7

edges: []
outputs:
  out: src.value
"#,
            ),
            (
                "root.yaml",
                r#"
kind: cluster
id: root
version: "0.1.0"

nodes:
  c:
    cluster: child@0.1.0

edges: []
outputs:
  result: c.out
"#,
            ),
        ]);
        let root_graph = root.join("root.yaml");
        let args = vec!["--direct".to_string()];
        run_graph_command(&root_graph, &args).expect("root boundary output through cluster node");
    }

    #[test]
    fn demo_1_yaml_executes_end_to_end() {
        let yaml = r#"
kind: cluster
id: demo_1
version: "0.1.0"

nodes:
  src_left_a:
    impl: number_source@0.1.0
    params:
      value: 4.0

  src_left_b:
    impl: number_source@0.1.0
    params:
      value: 2.0

  src_right_a:
    impl: number_source@0.1.0
    params:
      value: 1.0

  src_right_b:
    impl: number_source@0.1.0
    params:
      value: 1.0

  src_ctx_x:
    impl: context_number_source@0.1.0

  add_left:
    impl: add@0.1.0

  add_right:
    impl: add@0.1.0

  add_right_ctx:
    impl: add@0.1.0

  add_total:
    impl: add@0.1.0

  gt_a:
    impl: gt@0.1.0

  gt_b:
    impl: gt@0.1.0

  emit_a:
    impl: emit_if_true@0.1.0

  emit_b:
    impl: emit_if_true@0.1.0

  act_a:
    impl: ack_action@0.1.0
    params:
      accept: true

  act_b:
    impl: ack_action@0.1.0
    params:
      accept: true

edges:
  - src_left_a.value -> add_left.a
  - src_left_b.value -> add_left.b
  - src_right_a.value -> add_right.a
  - src_right_b.value -> add_right.b
  - add_left.result -> add_total.a
  - add_right.result -> add_total.b
  - add_right.result -> add_right_ctx.a
  - src_ctx_x.value -> add_right_ctx.b
  - add_left.result -> gt_a.a
  - add_right_ctx.result -> gt_a.b
  - add_right_ctx.result -> gt_b.a
  - add_left.result -> gt_b.b
  - gt_a.result -> emit_a.input
  - gt_b.result -> emit_b.input
  - emit_a.event -> act_a.event
  - emit_b.event -> act_b.event

outputs:
  sum_left: add_left.result
  sum_total: add_total.result
  action_a_outcome: act_a.outcome
  action_b_outcome: act_b.outcome
"#;

        let cluster = parse_graph_str(yaml, Path::new("demo_1.yaml")).expect("parse demo_1");
        let loader = PreloadedClusterLoader::new(HashMap::new());
        let catalog = build_core_catalog();
        let registries = core_registries().expect("core registries");
        let expanded = expand(&cluster, &loader, &catalog).expect("expand demo_1");
        let refs = Registries {
            sources: &registries.sources,
            computes: &registries.computes,
            triggers: &registries.triggers,
            actions: &registries.actions,
        };

        let report =
            ergo_runtime::runtime::run(&expanded, &catalog, &refs, &ExecutionContext::default())
                .expect("run demo_1");

        assert_eq!(
            report.outputs.get("sum_left"),
            Some(&RuntimeValue::Number(6.0))
        );
        assert_eq!(
            report.outputs.get("sum_total"),
            Some(&RuntimeValue::Number(8.0))
        );
        assert_eq!(
            report.outputs.get("action_a_outcome"),
            Some(&RuntimeValue::Event(RuntimeEvent::Action(
                ActionOutcome::Completed
            )))
        );
        assert_eq!(
            report.outputs.get("action_b_outcome"),
            Some(&RuntimeValue::Event(RuntimeEvent::Action(
                ActionOutcome::Skipped
            )))
        );
    }

    #[test]
    fn version_accepts_number_and_string() {
        let yaml = r#"
kind: cluster
id: vcheck
version: 1.0
nodes: {}
edges: []
outputs: {}
"#;
        let parsed = parse_graph_str(yaml, Path::new("inline.yaml")).expect("parse graph");
        assert_eq!(parsed.version, "1.0.0");
    }

    #[test]
    fn legacy_v_prefix_versions_are_rejected_with_migration_guidance() {
        let yaml = r#"
kind: cluster
id: legacy
version: "v1"
nodes: {}
edges: []
outputs: {}
"#;
        let err =
            parse_graph_str(yaml, Path::new("legacy.yaml")).expect_err("legacy v-prefix version");
        assert!(err.contains("strict semver"), "unexpected error: {err}");
        assert!(
            err.contains("migrate_graph_versions.py"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn semver_constraint_selector_parses_for_node_refs() {
        let yaml = r#"
kind: cluster
id: root
version: "1.0.0"
nodes:
  child:
    impl: add@^0.1
edges: []
outputs: {}
"#;
        let parsed = parse_graph_str(yaml, Path::new("constraint.yaml")).expect("parse graph");
        let node = parsed.nodes.get("child").expect("child node");
        match &node.kind {
            NodeKind::Impl { impl_id, version } => {
                assert_eq!(impl_id, "add");
                assert_eq!(version, "^0.1");
            }
            other => panic!("expected impl node, got {other:?}"),
        }
    }

    #[test]
    fn run_graph_command_rejects_circular_cluster_references() {
        let root = write_temp_tree(&[(
            "root.yaml",
            r#"
kind: cluster
id: root
version: "0.1.0"

nodes:
  self_ref:
    cluster: root@0.1.0

edges: []
outputs: {}
"#,
        )]);
        let graph = root.join("root.yaml");
        let args = vec!["--direct".to_string()];
        let err = run_graph_command(&graph, &args).expect_err("cycle should be rejected");
        assert!(
            err.contains("circular cluster reference"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn run_graph_command_rejects_duplicate_cluster_id_version_from_different_files() {
        let root = write_temp_tree(&[
            (
                "root/root.yaml",
                r#"
kind: cluster
id: root
version: "0.1.0"

nodes:
  a_node:
    cluster: a@0.1.0
  b_node:
    cluster: b@0.1.0
  add:
    impl: add@0.1.0

edges:
  - a_node.out -> add.a
  - b_node.out -> add.b

outputs:
  result: add.result
"#,
            ),
            (
                "a/a.yaml",
                r#"
kind: cluster
id: a
version: "0.1.0"

nodes:
  c:
    cluster: common@0.1.0
  z:
    impl: number_source@0.1.0
    params:
      value: 0
  pass:
    impl: add@0.1.0

edges:
  - c.out -> pass.a
  - z.value -> pass.b

outputs:
  out: pass.result
"#,
            ),
            (
                "b/b.yaml",
                r#"
kind: cluster
id: b
version: "0.1.0"

nodes:
  c:
    cluster: common@0.1.0
  z:
    impl: number_source@0.1.0
    params:
      value: 0
  pass:
    impl: add@0.1.0

edges:
  - c.out -> pass.a
  - z.value -> pass.b

outputs:
  out: pass.result
"#,
            ),
            (
                "a/common.yaml",
                r#"
kind: cluster
id: common
version: "0.1.0"

nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 10

edges: []
outputs:
  out: src.value
"#,
            ),
            (
                "b/common.yaml",
                r#"
kind: cluster
id: common
version: "0.1.0"

nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 100

edges: []
outputs:
  out: src.value
"#,
            ),
        ]);
        let graph = root.join("root/root.yaml");
        let search_a = root.join("a").to_string_lossy().to_string();
        let search_b = root.join("b").to_string_lossy().to_string();
        let args = vec![
            "--direct".to_string(),
            "--cluster-path".to_string(),
            search_a,
            "--cluster-path".to_string(),
            search_b,
        ];
        let err = run_graph_command(&graph, &args).expect_err("duplicate definitions must fail");
        assert!(
            err.contains("defined by multiple files"),
            "unexpected error: {err}"
        );
    }
}
