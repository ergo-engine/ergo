mod support;

use support::{assert_contains, read_capture, run_ergo, TempDir};

fn write_smoke_graph(tmp: &TempDir) {
    tmp.write(
        "graph.yaml",
        r#"
kind: cluster
id: cli_binary_smoke
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 2.5
edges: []
outputs:
  value_out: src.value
"#,
    );
}

fn write_smoke_fixture(tmp: &TempDir) {
    tmp.write(
        "fixture.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"x\":2.5}}}\n",
    );
}

#[test]
fn graph_run_writes_capture_for_temp_graph() {
    let tmp = TempDir::new("graph_run_capture");
    write_smoke_graph(&tmp);
    write_smoke_fixture(&tmp);

    let (code, stdout, stderr) = run_ergo(
        &[
            "run",
            "graph.yaml",
            "--fixture",
            "fixture.jsonl",
            "--capture-output",
            "capture.json",
        ],
        tmp.path(),
    );

    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert_contains(
        &stdout,
        "status=completed episodes=1 events=1 invoked=1 deferred=0",
    );
    assert_contains(&stdout, "capture artifact: capture.json");

    let capture = tmp.path().join("capture.json");
    assert!(capture.exists(), "capture artifact should exist");
    let bundle = read_capture(&capture);
    assert_eq!(
        bundle["graph_id"].as_str().expect("graph id"),
        "cli_binary_smoke"
    );
    assert_eq!(
        bundle["events"].as_array().expect("events array").len(),
        1,
        "one captured event expected"
    );
}

#[test]
fn graph_run_then_replay_reports_match_for_temp_graph() {
    let tmp = TempDir::new("graph_run_replay");
    write_smoke_graph(&tmp);
    write_smoke_fixture(&tmp);

    let (run_code, run_stdout, run_stderr) = run_ergo(
        &[
            "run",
            "graph.yaml",
            "--fixture",
            "fixture.jsonl",
            "--capture-output",
            "capture.json",
        ],
        tmp.path(),
    );
    assert_eq!(run_code, 0, "stdout:\n{run_stdout}\nstderr:\n{run_stderr}");

    let (replay_code, replay_stdout, replay_stderr) =
        run_ergo(&["replay", "capture.json", "-g", "graph.yaml"], tmp.path());
    assert_eq!(
        replay_code, 0,
        "stdout:\n{replay_stdout}\nstderr:\n{replay_stderr}"
    );
    assert_contains(
        &replay_stdout,
        "replay graph_id=cli_binary_smoke events=1 invoked=1 deferred=0 skipped=0",
    );
    assert_contains(&replay_stdout, "replay identity: match");
}
