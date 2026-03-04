use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use ergo_adapter::fixture::{parse_fixture, FixtureItem};
use serde_json::Value;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "ergo-cli-fixture-graph-stress-{}-{}-{}",
            std::process::id(),
            name,
            index
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path.join(name);
        fs::write(&path, contents).expect("write temp file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn repo_root() -> PathBuf {
    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for candidate in start.ancestors() {
        if candidate.join("Cargo.toml").exists() && candidate.join("docs").exists() {
            return candidate.to_path_buf();
        }
    }
    panic!(
        "failed to resolve repo root from manifest dir '{}'",
        start.display()
    );
}

fn dual_ma_graph_path() -> PathBuf {
    let root = repo_root();
    let legacy = root.join("dual_ma_crossover.yaml");
    if legacy.exists() {
        return legacy;
    }

    let sandbox = root.join("sandbox/trading_vertical/dual_ma_crossover.yaml");
    if sandbox.exists() {
        return sandbox;
    }

    panic!(
        "dual_ma_crossover.yaml not found at '{}' or '{}'",
        legacy.display(),
        sandbox.display()
    );
}

fn ergo_bin() -> &'static str {
    env!("CARGO_BIN_EXE_ergo")
}

fn run_ergo(args: &[&str], cwd: &Path) -> (i32, String, String) {
    let output = Command::new(ergo_bin())
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("failed to run ergo binary: {err}"));

    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected text to contain {needle:?}\nactual:\n{haystack}"
    );
}

fn assert_parse_ok(path: &Path) -> Vec<FixtureItem> {
    parse_fixture(path).unwrap_or_else(|err| panic!("expected parse success, got: {err}"))
}

fn assert_parse_empty(path: &Path) {
    let items = assert_parse_ok(path);
    assert!(
        items.is_empty(),
        "expected empty parsed fixture, got {items:?}"
    );
}

fn read_capture(path: &Path) -> Value {
    let raw = fs::read_to_string(path).expect("read capture");
    serde_json::from_str(&raw).expect("parse capture json")
}

#[test]
fn boundary_whitespace_only_fixture_run_errors_with_no_episodes() {
    let tmp = TempDir::new("whitespace_no_episodes");
    let fixture = tmp.write("whitespace.jsonl", "   \n\t\n\n");

    assert_parse_empty(&fixture);

    let capture = tmp.path.join("capture.json");
    let fixture_s = fixture.to_string_lossy().to_string();
    let capture_s = capture.to_string_lossy().to_string();
    let (code, _stdout, stderr) = run_ergo(
        &["fixture", "run", &fixture_s, "--capture-output", &capture_s],
        &repo_root(),
    );
    assert_eq!(code, 1, "stderr: {stderr}");
    assert_contains(&stderr, "fixture contained no episodes");
}

#[test]
fn boundary_single_event_without_episode_start_auto_creates_episode_in_runner() {
    let tmp = TempDir::new("single_event_auto_episode");
    let fixture = tmp.write(
        "single_event.jsonl",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    );

    let items = assert_parse_ok(&fixture);
    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], FixtureItem::Event { .. }));

    let capture = tmp.path.join("capture.json");
    let fixture_s = fixture.to_string_lossy().to_string();
    let capture_s = capture.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ergo(
        &["fixture", "run", &fixture_s, "--capture-output", &capture_s],
        &repo_root(),
    );
    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert_contains(&stdout, "episode E1:");
    assert_contains(&stdout, "capture artifact:");
    assert!(capture.exists(), "capture artifact should exist");
}

#[test]
fn boundary_episode_start_with_no_following_events_errors_in_runner() {
    let tmp = TempDir::new("episode_start_no_events");
    let fixture = tmp.write(
        "episode_start_no_events.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n",
    );

    let items = assert_parse_ok(&fixture);
    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], FixtureItem::EpisodeStart { .. }));

    let capture = tmp.path.join("capture.json");
    let fixture_s = fixture.to_string_lossy().to_string();
    let capture_s = capture.to_string_lossy().to_string();
    let (code, _stdout, stderr) = run_ergo(
        &["fixture", "run", &fixture_s, "--capture-output", &capture_s],
        &repo_root(),
    );
    assert_eq!(code, 1, "stderr: {stderr}");
    assert_contains(&stderr, "fixture contained no events");
}

#[test]
fn boundary_two_episode_starts_back_to_back_first_episode_empty_errors_in_runner() {
    let tmp = TempDir::new("two_episode_starts_back_to_back");
    let fixture = tmp.write(
        "two_starts.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
         {\"kind\":\"episode_start\",\"id\":\"E2\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    );

    let items = assert_parse_ok(&fixture);
    assert_eq!(items.len(), 3);
    assert!(matches!(items[0], FixtureItem::EpisodeStart { .. }));
    assert!(matches!(items[1], FixtureItem::EpisodeStart { .. }));
    assert!(matches!(items[2], FixtureItem::Event { .. }));

    let capture = tmp.path.join("capture.json");
    let fixture_s = fixture.to_string_lossy().to_string();
    let capture_s = capture.to_string_lossy().to_string();
    let (code, _stdout, stderr) = run_ergo(
        &["fixture", "run", &fixture_s, "--capture-output", &capture_s],
        &repo_root(),
    );
    assert_eq!(code, 1, "stderr: {stderr}");
    assert_contains(&stderr, "episode 'E1' has no events");
}

#[test]
fn graph_capture_payload_is_present_in_events_payload_bytes() {
    let tmp = TempDir::new("graph_payload_capture");
    let fixture = tmp.write(
        "payload_fixture.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"x\":2.5}}}\n",
    );

    let items = assert_parse_ok(&fixture);
    assert_eq!(items.len(), 2);
    assert!(matches!(items[0], FixtureItem::EpisodeStart { .. }));
    assert!(matches!(items[1], FixtureItem::Event { .. }));

    let capture = tmp.path.join("payload_capture.json");
    let graph_s = dual_ma_graph_path().to_string_lossy().to_string();
    let fixture_s = fixture.to_string_lossy().to_string();
    let capture_s = capture.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ergo(
        &[
            "run",
            &graph_s,
            "--fixture",
            &fixture_s,
            "--capture-output",
            &capture_s,
        ],
        &repo_root(),
    );
    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert_contains(&stdout, "capture artifact:");

    let bundle = read_capture(&capture);
    let events = bundle["events"].as_array().expect("capture events array");
    assert_eq!(events.len(), 1);
    let payload = events[0]["payload"]
        .as_array()
        .expect("payload bytes array");
    assert!(
        !payload.is_empty(),
        "expected non-empty payload bytes in capture event: {:?}",
        events[0]
    );
}

#[test]
fn graph_run_then_replay_confirms_match_for_payload_fixture() {
    let tmp = TempDir::new("graph_run_replay_match");
    let fixture = tmp.write(
        "run_replay_fixture.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"x\":2.5}}}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"x\":-1.0}}}\n",
    );

    let items = assert_parse_ok(&fixture);
    assert_eq!(items.len(), 3);
    assert!(matches!(items[0], FixtureItem::EpisodeStart { .. }));
    assert!(matches!(items[1], FixtureItem::Event { .. }));
    assert!(matches!(items[2], FixtureItem::Event { .. }));

    let capture = tmp.path.join("run_replay_capture.json");
    let graph_s = dual_ma_graph_path().to_string_lossy().to_string();
    let fixture_s = fixture.to_string_lossy().to_string();
    let capture_s = capture.to_string_lossy().to_string();
    let (run_code, run_stdout, run_stderr) = run_ergo(
        &[
            "run",
            &graph_s,
            "--fixture",
            &fixture_s,
            "--capture-output",
            &capture_s,
        ],
        &repo_root(),
    );
    assert_eq!(run_code, 0, "stdout:\n{run_stdout}\nstderr:\n{run_stderr}");
    assert_contains(&run_stdout, "episodes=1 events=2");

    let (replay_code, replay_stdout, replay_stderr) =
        run_ergo(&["replay", &capture_s, "-g", &graph_s], &repo_root());
    assert_eq!(
        replay_code, 0,
        "stdout:\n{replay_stdout}\nstderr:\n{replay_stderr}"
    );
    assert_contains(&replay_stdout, "replay identity: match");
}

#[test]
fn graph_capture_distinct_payload_hashes_for_varying_payloads() {
    let tmp = TempDir::new("graph_distinct_payload_hashes");
    let fixture = tmp.write(
        "varying_payloads.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"x\":1.0}}}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"x\":2.0}}}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"x\":3.0}}}\n",
    );

    let items = assert_parse_ok(&fixture);
    assert_eq!(items.len(), 4);
    assert!(matches!(items[0], FixtureItem::EpisodeStart { .. }));
    assert!(matches!(items[1], FixtureItem::Event { .. }));
    assert!(matches!(items[2], FixtureItem::Event { .. }));
    assert!(matches!(items[3], FixtureItem::Event { .. }));

    let capture = tmp.path.join("varying_payloads_capture.json");
    let graph_s = dual_ma_graph_path().to_string_lossy().to_string();
    let fixture_s = fixture.to_string_lossy().to_string();
    let capture_s = capture.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ergo(
        &[
            "run",
            &graph_s,
            "--fixture",
            &fixture_s,
            "--capture-output",
            &capture_s,
        ],
        &repo_root(),
    );
    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");

    let bundle = read_capture(&capture);
    let events = bundle["events"].as_array().expect("capture events array");
    assert_eq!(events.len(), 3);
    let hashes: Vec<&str> = events
        .iter()
        .map(|event| event["payload_hash"].as_str().expect("payload_hash string"))
        .collect();
    assert_eq!(hashes.len(), 3);
    assert_ne!(hashes[0], hashes[1]);
    assert_ne!(hashes[0], hashes[2]);
    assert_ne!(hashes[1], hashes[2]);
}

#[test]
fn graph_capture_payload_vs_no_payload_produces_different_hashes() {
    let tmp = TempDir::new("graph_payload_vs_no_payload_hashes");

    let fixture_with_payload = tmp.write(
        "with_payload.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\",\"payload\":{\"x\":7.0}}}\n",
    );
    let fixture_without_payload = tmp.write(
        "without_payload.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    );

    let with_items = assert_parse_ok(&fixture_with_payload);
    let without_items = assert_parse_ok(&fixture_without_payload);
    assert_eq!(with_items.len(), 2);
    assert_eq!(without_items.len(), 2);
    assert!(matches!(with_items[0], FixtureItem::EpisodeStart { .. }));
    assert!(matches!(with_items[1], FixtureItem::Event { .. }));
    assert!(matches!(without_items[0], FixtureItem::EpisodeStart { .. }));
    assert!(matches!(without_items[1], FixtureItem::Event { .. }));

    let graph_s = dual_ma_graph_path().to_string_lossy().to_string();
    let with_fixture_s = fixture_with_payload.to_string_lossy().to_string();
    let without_fixture_s = fixture_without_payload.to_string_lossy().to_string();

    let with_capture = tmp.path.join("with_payload_capture.json");
    let with_capture_s = with_capture.to_string_lossy().to_string();
    let (with_code, with_stdout, with_stderr) = run_ergo(
        &[
            "run",
            &graph_s,
            "--fixture",
            &with_fixture_s,
            "--capture-output",
            &with_capture_s,
        ],
        &repo_root(),
    );
    assert_eq!(
        with_code, 0,
        "stdout:\n{with_stdout}\nstderr:\n{with_stderr}"
    );

    let without_capture = tmp.path.join("without_payload_capture.json");
    let without_capture_s = without_capture.to_string_lossy().to_string();
    let (without_code, without_stdout, without_stderr) = run_ergo(
        &[
            "run",
            &graph_s,
            "--fixture",
            &without_fixture_s,
            "--capture-output",
            &without_capture_s,
        ],
        &repo_root(),
    );
    assert_eq!(
        without_code, 0,
        "stdout:\n{without_stdout}\nstderr:\n{without_stderr}"
    );

    let with_bundle = read_capture(&with_capture);
    let without_bundle = read_capture(&without_capture);
    let with_event = &with_bundle["events"].as_array().expect("events")[0];
    let without_event = &without_bundle["events"].as_array().expect("events")[0];

    let with_hash = with_event["payload_hash"]
        .as_str()
        .expect("payload_hash string");
    let without_hash = without_event["payload_hash"]
        .as_str()
        .expect("payload_hash string");
    assert_ne!(with_hash, without_hash, "payload hashes should differ");

    let with_payload = with_event["payload"]
        .as_array()
        .expect("payload bytes array");
    let without_payload = without_event["payload"]
        .as_array()
        .expect("payload bytes array");
    assert!(
        !with_payload.is_empty(),
        "with-payload capture should be non-empty"
    );
    assert!(
        without_payload.is_empty(),
        "no-payload capture should be empty bytes"
    );
}
