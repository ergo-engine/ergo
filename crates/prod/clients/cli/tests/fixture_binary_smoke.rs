mod support;

use support::{assert_contains, read_capture, run_ergo, TempDir};

#[test]
fn fixture_run_empty_fixture_returns_cli_error() {
    let tmp = TempDir::new("fixture_empty");
    tmp.write("whitespace.jsonl", "   \n\t\n\n");

    let (code, _stdout, stderr) = run_ergo(
        &[
            "fixture",
            "run",
            "whitespace.jsonl",
            "--capture-output",
            "capture.json",
        ],
        tmp.path(),
    );

    assert_eq!(code, 1, "stderr: {stderr}");
    assert_contains(&stderr, "fixture contained no episodes");
}

#[test]
fn fixture_run_single_event_auto_creates_episode_and_writes_capture() {
    let tmp = TempDir::new("fixture_auto_episode");
    tmp.write(
        "single_event.jsonl",
        "{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    );

    let (code, stdout, stderr) = run_ergo(
        &[
            "fixture",
            "run",
            "single_event.jsonl",
            "--capture-output",
            "capture.json",
        ],
        tmp.path(),
    );

    assert_eq!(code, 0, "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert_contains(&stdout, "episode E1: events=1");
    assert_contains(&stdout, "capture artifact: capture.json");

    let capture = tmp.path().join("capture.json");
    assert!(capture.exists(), "capture artifact should exist");
    let bundle = read_capture(&capture);
    assert_eq!(
        bundle["events"].as_array().expect("events array").len(),
        1,
        "one captured event expected"
    );
}

#[test]
fn fixture_run_episode_start_without_events_returns_cli_error() {
    let tmp = TempDir::new("fixture_no_events");
    tmp.write(
        "episode_start_no_events.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n",
    );

    let (code, _stdout, stderr) = run_ergo(
        &[
            "fixture",
            "run",
            "episode_start_no_events.jsonl",
            "--capture-output",
            "capture.json",
        ],
        tmp.path(),
    );

    assert_eq!(code, 1, "stderr: {stderr}");
    assert_contains(&stderr, "fixture contained no events");
}

#[test]
fn fixture_run_back_to_back_episode_starts_returns_cli_error() {
    let tmp = TempDir::new("fixture_back_to_back_starts");
    tmp.write(
        "two_starts.jsonl",
        "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n\
         {\"kind\":\"episode_start\",\"id\":\"E2\"}\n\
         {\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
    );

    let (code, _stdout, stderr) = run_ergo(
        &[
            "fixture",
            "run",
            "two_starts.jsonl",
            "--capture-output",
            "capture.json",
        ],
        tmp.path(),
    );

    assert_eq!(code, 1, "stderr: {stderr}");
    assert_contains(&stderr, "episode 'E1' has no events");
}
