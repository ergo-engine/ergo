//! project::tests
//!
//! Purpose:
//! - Keep loader project discovery and `ergo.toml` profile-parse regression
//!   coverage out of the production file while locking the current path-backed
//!   contract.

use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn make_temp_dir(label: &str) -> PathBuf {
    let index = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ergo_loader_project_{label}_{}_{}_{}",
        std::process::id(),
        index,
        nanos
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_file(base: &Path, rel: &str, contents: &str) -> PathBuf {
    let path = base.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(&path, contents).expect("write file");
    path
}

#[test]
fn discovers_project_root_from_nested_path() {
    let root = make_temp_dir("discover");
    write_file(
        &root,
        "ergo.toml",
        "name = \"sdk-project\"\nversion = \"0.1.0\"\n",
    );
    let nested = root.join("graphs/deep/path");
    fs::create_dir_all(&nested).expect("create nested");

    let discovered = discover_project_root(&nested).expect("discover project");
    assert_eq!(discovered, root);

    let _ = fs::remove_dir_all(discovered);
}

#[test]
fn resolve_run_profile_adds_clusters_path_and_fixture_ingress() {
    let root = make_temp_dir("resolve");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
capture_output = "captures/historical.capture.json"
max_duration = "15m"
max_events = 42
"#,
    );

    let project = load_project(&root).expect("load project");
    let resolved = project
        .resolve_run_profile("historical")
        .expect("resolve profile");

    assert_eq!(resolved.graph_path, root.join("graphs/strategy.yaml"));
    assert_eq!(resolved.cluster_paths, vec![root.join("clusters")]);
    assert_eq!(
        resolved.capture_output,
        Some(root.join("captures/historical.capture.json"))
    );
    assert_eq!(resolved.max_duration, Some(Duration::from_secs(15 * 60)));
    assert_eq!(resolved.max_events, Some(42));
    assert_eq!(
        resolved.ingress,
        ResolvedProjectIngress::Fixture {
            path: root.join("fixtures/historical.jsonl")
        }
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_profile_durations_accept_the_shared_supported_units() {
    let manifest = toml::from_str::<ProjectManifest>(
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.ms]
graph = "graphs/ms.yaml"
fixture = "fixtures/ms.jsonl"
max_duration = "1500ms"

[profiles.s]
graph = "graphs/s.yaml"
fixture = "fixtures/s.jsonl"
max_duration = "5s"

[profiles.m]
graph = "graphs/m.yaml"
fixture = "fixtures/m.jsonl"
max_duration = "3m"

[profiles.h]
graph = "graphs/h.yaml"
fixture = "fixtures/h.jsonl"
max_duration = "2h"
"#,
    )
    .expect("manifest should parse");

    assert_eq!(
        manifest
            .profiles
            .get("ms")
            .expect("ms profile")
            .max_duration,
        Some(Duration::from_millis(1500))
    );
    assert_eq!(
        manifest.profiles.get("s").expect("s profile").max_duration,
        Some(Duration::from_secs(5))
    );
    assert_eq!(
        manifest.profiles.get("m").expect("m profile").max_duration,
        Some(Duration::from_secs(180))
    );
    assert_eq!(
        manifest.profiles.get("h").expect("h profile").max_duration,
        Some(Duration::from_secs(7200))
    );
}

#[test]
fn load_project_rejects_non_numeric_profile_duration_literal_with_current_wording() {
    let root = make_temp_dir("unsupported_duration");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
max_duration = "15fortnights"
"#,
    );

    let err = load_project(&root).expect_err("invalid duration must fail project load");
    match err {
        ProjectError::ProjectParse { detail, .. } => assert!(
            detail.contains("invalid duration '15fortnights': invalid digit found in string"),
            "unexpected parse detail: {detail}"
        ),
        other => panic!("expected project parse error, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_project_rejects_invalid_numeric_profile_duration_with_current_wording() {
    let root = make_temp_dir("invalid_duration_number");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.historical]
graph = "graphs/strategy.yaml"
fixture = "fixtures/historical.jsonl"
max_duration = "xs"
"#,
    );

    let err = load_project(&root).expect_err("invalid duration must fail project load");
    match err {
        ProjectError::ProjectParse { detail, .. } => assert!(
            detail.contains("invalid duration 'xs': invalid digit found in string"),
            "unexpected parse detail: {detail}"
        ),
        other => panic!("expected project parse error, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_run_profile_rejects_mutually_exclusive_ingress_sources() {
    let root = make_temp_dir("invalid_ingress");
    write_file(
        &root,
        "ergo.toml",
        r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
fixture = "fixtures/live.jsonl"

[profiles.live.ingress]
type = "process"
command = ["python3", "channels/ingress/live.py"]
"#,
    );

    let project = load_project(&root).expect("load project");
    let err = project
        .resolve_run_profile("live")
        .expect_err("mutually exclusive ingress must fail");
    assert!(matches!(err, ProjectError::ProfileInvalid { .. }));

    let _ = fs::remove_dir_all(root);
}
