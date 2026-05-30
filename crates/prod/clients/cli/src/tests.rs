use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

#[test]
fn replay_graph_replays_yaml_capture() -> Result<(), String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-cli-replay-graph-test-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

    let graph_path = temp_dir.join("graph.yaml");
    let fixture_path = temp_dir.join("fixture.jsonl");
    let capture_path = temp_dir.join("capture.json");

    let graph = r#"
kind: cluster
id: replay_basic
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
    let fixture = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";

    fs::write(&graph_path, graph).map_err(|err| format!("write graph: {err}"))?;
    fs::write(&fixture_path, fixture).map_err(|err| format!("write fixture: {err}"))?;

    let run_args = vec![
        "--fixture".to_string(),
        fixture_path.to_string_lossy().to_string(),
        "--capture-output".to_string(),
        capture_path.to_string_lossy().to_string(),
    ];
    graph_yaml::run_graph_command(&graph_path, &run_args)?;

    replay_graph(&capture_path, &graph_path, &[], None)?;

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn replay_graph_uses_host_rehydrate_path() -> Result<(), String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-cli-replay-host-rehydrate-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

    let graph_path = temp_dir.join("graph.yaml");
    let fixture_path = temp_dir.join("fixture.jsonl");
    let capture_path = temp_dir.join("capture.json");

    let graph = r#"
kind: cluster
id: replay_basic
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
    let fixture = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";

    fs::write(&graph_path, graph).map_err(|err| format!("write graph: {err}"))?;
    fs::write(&fixture_path, fixture).map_err(|err| format!("write fixture: {err}"))?;

    let run_args = vec![
        "--fixture".to_string(),
        fixture_path.to_string_lossy().to_string(),
        "--capture-output".to_string(),
        capture_path.to_string_lossy().to_string(),
    ];
    graph_yaml::run_graph_command(&graph_path, &run_args)?;

    let mut bundle = load_bundle(&capture_path)?;
    bundle.events[0].payload.data = br#""not-an-object""#.to_vec();
    bundle.events[0].payload_hash = ergo_adapter::capture::hash_payload(&bundle.events[0].payload);
    fs::write(
        &capture_path,
        serde_json::to_vec_pretty(&bundle).map_err(|err| format!("serialize capture: {err}"))?,
    )
    .map_err(|err| format!("rewrite capture: {err}"))?;

    let err = replay_graph(&capture_path, &graph_path, &[], None)
        .expect_err("host replay should reject invalid rehydrated event payload");
    assert!(
        err.contains("code: replay.event_rehydrate_failed"),
        "unexpected err: {err}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn replay_graph_rejects_duplicate_event_ids_in_strict_preflight() -> Result<(), String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-cli-replay-duplicate-event-id-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

    let graph_path = temp_dir.join("graph.yaml");
    let fixture_path = temp_dir.join("fixture.jsonl");
    let capture_path = temp_dir.join("capture.json");

    let graph = r#"
kind: cluster
id: replay_basic
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
    let fixture = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";

    fs::write(&graph_path, graph).map_err(|err| format!("write graph: {err}"))?;
    fs::write(&fixture_path, fixture).map_err(|err| format!("write fixture: {err}"))?;

    let run_args = vec![
        "--fixture".to_string(),
        fixture_path.to_string_lossy().to_string(),
        "--capture-output".to_string(),
        capture_path.to_string_lossy().to_string(),
    ];
    graph_yaml::run_graph_command(&graph_path, &run_args)?;

    let mut bundle = load_bundle(&capture_path)?;
    let duplicate = bundle.events[0].event_id.clone();
    bundle.events[1].event_id = duplicate;
    fs::write(
        &capture_path,
        serde_json::to_vec_pretty(&bundle).map_err(|err| format!("serialize capture: {err}"))?,
    )
    .map_err(|err| format!("rewrite capture: {err}"))?;

    let err = replay_graph(&capture_path, &graph_path, &[], None)
        .expect_err("strict replay preflight must reject duplicate event ids");
    assert!(
        err.contains("code: replay.duplicate_event_id"),
        "unexpected err: {err}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn replay_graph_detects_effect_drift() -> Result<(), String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-cli-replay-effect-drift-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

    let graph_path = temp_dir.join("graph.yaml");
    let fixture_path = temp_dir.join("fixture.jsonl");
    let capture_path = temp_dir.join("capture.json");

    let graph = r#"
kind: cluster
id: replay_basic
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
    let fixture = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";

    fs::write(&graph_path, graph).map_err(|err| format!("write graph: {err}"))?;
    fs::write(&fixture_path, fixture).map_err(|err| format!("write fixture: {err}"))?;

    let run_args = vec![
        "--fixture".to_string(),
        fixture_path.to_string_lossy().to_string(),
        "--capture-output".to_string(),
        capture_path.to_string_lossy().to_string(),
    ];
    graph_yaml::run_graph_command(&graph_path, &run_args)?;

    let mut bundle = load_bundle(&capture_path)?;
    let fake_effect = ergo_runtime::common::ActionEffect {
        kind: "set_context".to_string(),
        writes: vec![ergo_runtime::common::EffectWrite {
            key: "drifted".to_string(),
            value: ergo_runtime::common::Value::Number(42.0),
        }],
        intents: vec![],
    };
    bundle.decisions[0]
        .effects
        .push(ergo_supervisor::CapturedActionEffect {
            effect_hash: ergo_supervisor::replay::hash_effect(&fake_effect),
            effect: fake_effect,
        });
    fs::write(
        &capture_path,
        serde_json::to_vec_pretty(&bundle).map_err(|err| format!("serialize capture: {err}"))?,
    )
    .map_err(|err| format!("rewrite capture: {err}"))?;

    let err = replay_graph(&capture_path, &graph_path, &[], None)
        .expect_err("effect drift should fail canonical replay");
    assert!(
        err.contains("code: replay.effect_mismatch"),
        "unexpected err: {err}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn replay_graph_rejects_graph_id_mismatch() -> Result<(), String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-cli-replay-mismatch-test-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

    let graph_path = temp_dir.join("graph.yaml");
    let other_graph_path = temp_dir.join("other_graph.yaml");
    let fixture_path = temp_dir.join("fixture.jsonl");
    let capture_path = temp_dir.join("capture.json");

    let graph = r#"
kind: cluster
id: replay_basic
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

    let other_graph = r#"
kind: cluster
id: replay_other
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 8.0
edges: []
outputs:
  value_out: src.value
"#;

    let fixture = "\
{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n";

    fs::write(&graph_path, graph).map_err(|err| format!("write graph: {err}"))?;
    fs::write(&other_graph_path, other_graph).map_err(|err| format!("write other graph: {err}"))?;
    fs::write(&fixture_path, fixture).map_err(|err| format!("write fixture: {err}"))?;

    let run_args = vec![
        "--fixture".to_string(),
        fixture_path.to_string_lossy().to_string(),
        "--capture-output".to_string(),
        capture_path.to_string_lossy().to_string(),
    ];
    graph_yaml::run_graph_command(&graph_path, &run_args)?;

    let err = replay_graph(&capture_path, &other_graph_path, &[], None)
        .expect_err("graph id mismatch should fail");
    assert!(
        err.contains("error: graph_id mismatch"),
        "unexpected err: {err}"
    );
    assert!(
        err.contains("where: capture graph_id"),
        "unexpected err: {err}"
    );
    assert!(
        err.contains("fix: replay with --graph"),
        "unexpected err: {err}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn parse_replay_options_requires_graph() {
    let err = parse_replay_options(&["--adapter".to_string(), "adapter.yaml".to_string()])
        .expect_err("missing graph should fail");
    assert!(
        err.contains("replay requires -g|--graph"),
        "unexpected err: {err}"
    );
    assert!(
        err.contains("code: cli.missing_required_option")
            && err.contains("where: replay command options")
            && err.contains("fix: rerun with -g <graph.yaml>"),
        "unexpected err: {err}"
    );
}

#[test]
fn parse_replay_options_accepts_short_graph_and_adapter_flags() {
    let opts = parse_replay_options(&[
        "-g".to_string(),
        "graph.yaml".to_string(),
        "-a".to_string(),
        "adapter.yaml".to_string(),
    ])
    .expect("short replay flags should parse");
    assert_eq!(opts.graph_path.as_deref(), Some(Path::new("graph.yaml")));
    assert_eq!(
        opts.adapter_path.as_deref(),
        Some(Path::new("adapter.yaml"))
    );
}

#[test]
fn parse_replay_options_keeps_long_flag_compatibility() {
    let opts = parse_replay_options(&[
        "--graph".to_string(),
        "graph.yaml".to_string(),
        "--adapter".to_string(),
        "adapter.yaml".to_string(),
    ])
    .expect("long replay flags should parse");
    assert_eq!(opts.graph_path.as_deref(), Some(Path::new("graph.yaml")));
    assert_eq!(
        opts.adapter_path.as_deref(),
        Some(Path::new("adapter.yaml"))
    );
}

#[test]
fn parse_replay_options_rejects_pretty_capture_flag() {
    let err = parse_replay_options(&[
        "--graph".to_string(),
        "graph.yaml".to_string(),
        "--pretty-capture".to_string(),
    ])
    .expect_err("unknown replay flag should fail");
    assert!(
        err.contains("unknown replay option '--pretty-capture'"),
        "unexpected err: {err}"
    );
    assert!(
        err.contains("code: cli.invalid_option")
            && err.contains("where: arg '--pretty-capture'")
            && err.contains("fix: use -g|--graph, -a|--adapter"),
        "unexpected err: {err}"
    );
}

#[test]
fn usage_moves_fixture_to_top_level_subcommand() {
    let help = usage();
    assert!(
        help.contains("ergo fixture inspect <events.jsonl>"),
        "expected fixture inspect in top-level help: {help}"
    );
    assert!(
        help.contains("ergo fixture validate <events.jsonl>"),
        "expected fixture validate in top-level help: {help}"
    );
    assert!(
        !help.contains("ergo fixture run"),
        "fixture run should be removed in v1 help: {help}"
    );
    assert!(
        !help.contains("ergo run fixture"),
        "run fixture should be removed in v1 help: {help}"
    );
}

#[test]
fn help_topic_fixture_matches_fixture_usage() {
    let topic = help_topic("fixture").expect("fixture help should exist");
    assert_eq!(topic, fixture_ops::fixture_usage());
}

#[test]
fn help_topic_unknown_returns_none() {
    assert!(help_topic("does-not-exist").is_none());
}

#[test]
fn format_replay_error_includes_rule_like_fields() {
    let err = format_replay_error(&ReplayError::AdapterRequiredForProvenancedCapture);
    assert!(
        err.contains("error:")
            && err.contains("code: replay.adapter_required")
            && err.contains("where:")
            && err.contains("fix:"),
        "unexpected err: {err}"
    );
}

#[test]
fn format_replay_error_effect_mismatch_includes_code() {
    let err = format_replay_error(&ReplayError::EffectMismatch {
        event_id: EventId::new("e1"),
        effect_index: 0,
        expected: None,
        actual: None,
        detail: "hash differs".to_string(),
    });
    assert!(
        err.contains("code: replay.effect_mismatch"),
        "expected replay.effect_mismatch code: {err}"
    );
    assert!(
        err.contains("error:") && err.contains("where:") && err.contains("fix:"),
        "unexpected format: {err}"
    );
}

#[test]
fn format_replay_error_effect_mismatch_surfaces_expected_actual() {
    use ergo_runtime::common::{ActionEffect, EffectWrite, Value};
    use ergo_supervisor::replay::hash_effect;
    use ergo_supervisor::CapturedActionEffect;

    let expected_effect = ActionEffect {
        kind: "set_context".to_string(),
        writes: vec![EffectWrite {
            key: "price".to_string(),
            value: Value::Number(42.0),
        }],
        intents: vec![],
    };
    let actual_effect = ActionEffect {
        kind: "set_context".to_string(),
        writes: vec![EffectWrite {
            key: "volume".to_string(),
            value: Value::Number(99.0),
        }],
        intents: vec![],
    };
    let err = format_replay_error(&ReplayError::EffectMismatch {
        event_id: EventId::new("e1"),
        effect_index: 0,
        expected: Some(CapturedActionEffect {
            effect: expected_effect,
            effect_hash: hash_effect(&ActionEffect {
                kind: "set_context".to_string(),
                writes: vec![EffectWrite {
                    key: "price".to_string(),
                    value: Value::Number(42.0),
                }],
                intents: vec![],
            }),
        }),
        actual: Some(CapturedActionEffect {
            effect: actual_effect,
            effect_hash: hash_effect(&ActionEffect {
                kind: "set_context".to_string(),
                writes: vec![EffectWrite {
                    key: "volume".to_string(),
                    value: Value::Number(99.0),
                }],
                intents: vec![],
            }),
        }),
        detail: "content mismatch".to_string(),
    });
    assert!(
        err.contains("code: replay.effect_mismatch"),
        "expected code: {err}"
    );
    assert!(
        err.contains("detail: expected:") && err.contains("\"price\""),
        "expected effect detail with key 'price': {err}"
    );
    assert!(
        err.contains("detail: actual:") && err.contains("\"volume\""),
        "actual effect detail with key 'volume': {err}"
    );
}
