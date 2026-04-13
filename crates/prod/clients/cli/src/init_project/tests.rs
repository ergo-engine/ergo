use super::*;
use std::collections::BTreeSet;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn make_temp_dir_under(base: &Path, label: &str) -> PathBuf {
    let index = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let dir = base.join(format!(
        "ergo_cli_init_{label}_{}_{}_{}",
        std::process::id(),
        index,
        nanos
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn make_temp_dir(label: &str) -> PathBuf {
    make_temp_dir_under(&std::env::temp_dir(), label)
}

fn make_workspace_temp_dir(label: &str) -> PathBuf {
    let target_root = workspace_root().expect("workspace root").join("target");
    fs::create_dir_all(&target_root).expect("create target root");
    make_temp_dir_under(&target_root, label)
}

fn scaffold_test_cargo_target_dir() -> PathBuf {
    let dir = workspace_root()
        .expect("workspace root")
        .join("target/ergo_cli_init_scaffold_tests");
    fs::create_dir_all(&dir).expect("create shared scaffold cargo target dir");
    dir
}

fn cli_sdk_path() -> PathBuf {
    workspace_root()
        .expect("workspace root")
        .join("crates/prod/clients/sdk-rust")
}

fn run_cargo_project(project_root: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new("cargo")
        .args(args)
        .current_dir(project_root)
        .env("CARGO_TARGET_DIR", scaffold_test_cargo_target_dir())
        .output()
        .map_err(|err| format!("spawn cargo {:?}: {err}", args))
}

fn collect_project_files(project_root: &Path) -> Result<BTreeSet<String>, String> {
    let mut files = BTreeSet::new();
    let mut dirs = vec![project_root.to_path_buf()];

    while let Some(dir) = dirs.pop() {
        for entry in
            fs::read_dir(&dir).map_err(|err| format!("read dir '{}': {err}", dir.display()))?
        {
            let entry = entry.map_err(|err| format!("read dir entry: {err}"))?;
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                let relative = path
                    .strip_prefix(project_root)
                    .map_err(|err| format!("strip prefix '{}': {err}", project_root.display()))?;
                files.insert(normalize_path(relative));
            }
        }
    }

    Ok(files)
}

fn expected_scaffold_files() -> BTreeSet<String> {
    BTreeSet::from([
        ".gitignore".to_string(),
        "README.md".to_string(),
        "Cargo.toml".to_string(),
        "ergo.toml".to_string(),
        "src/main.rs".to_string(),
        "src/implementations/mod.rs".to_string(),
        "src/implementations/sources.rs".to_string(),
        "src/implementations/actions.rs".to_string(),
        "graphs/strategy.yaml".to_string(),
        "clusters/sample_message.yaml".to_string(),
        "adapters/sample.yaml".to_string(),
        "channels/ingress/live_feed.py".to_string(),
        "channels/egress/sample_outbox.py".to_string(),
        "egress/live.toml".to_string(),
        "fixtures/historical.jsonl".to_string(),
        "captures/.gitkeep".to_string(),
    ])
}

#[test]
fn init_command_creates_sdk_first_scaffold() -> Result<(), String> {
    let root = make_workspace_temp_dir("basic");
    let project_root = root.join("sample-app");
    let message = init_command(&[project_root.display().to_string()])?;

    assert!(message.contains("initialized Ergo SDK project"));
    assert!(message.contains("channel scripts: sample ingress/egress scripts target Python 3"));
    assert!(message.contains("cargo run -- profiles"));
    assert!(message.contains("cargo run -- replay historical captures/historical.capture.json"));
    assert!(project_root.join("README.md").exists());
    assert!(project_root.join("Cargo.toml").exists());
    assert!(project_root.join("ergo.toml").exists());
    assert!(project_root.join("graphs/strategy.yaml").exists());
    assert!(project_root.join("clusters/sample_message.yaml").exists());
    assert!(project_root.join("src/implementations/actions.rs").exists());
    assert!(project_root
        .join("channels/egress/sample_outbox.py")
        .exists());
    assert!(project_root.join("channels/ingress/live_feed.py").exists());

    let cargo_toml = fs::read_to_string(project_root.join("Cargo.toml"))
        .map_err(|err| format!("read Cargo.toml: {err}"))?;
    let expected_sdk_path = render_dependency_path(&project_root, &cli_sdk_path());
    assert!(
        cargo_toml.contains(&format!("path = \"{expected_sdk_path}\"")),
        "expected repo-relative sdk path, got:\n{cargo_toml}"
    );
    assert!(cargo_toml.contains("ctrlc = \"3.4\""));

    let main_rs = fs::read_to_string(project_root.join("src/main.rs"))
        .map_err(|err| format!("read main.rs: {err}"))?;
    assert!(main_rs.contains("Ergo::from_project"));
    assert!(main_rs.contains("StopHandle"));
    assert!(main_rs.contains("run_profile_with_stop"));
    assert!(main_rs.contains("ctrlc::set_handler"));
    assert!(main_rs.contains("\"profiles\""));

    let graph_yaml = fs::read_to_string(project_root.join("graphs/strategy.yaml"))
        .map_err(|err| format!("read graph: {err}"))?;
    assert!(graph_yaml.contains("cluster: sample_message@0.1.0"));

    let readme = fs::read_to_string(project_root.join("README.md"))
        .map_err(|err| format!("read README.md: {err}"))?;
    assert!(readme.contains("cargo run -- profiles"));
    assert!(readme.contains("src/implementations/actions.rs"));
    assert!(readme.contains("Ctrl-C"));

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn scaffold_matches_expected_tree_and_templates() -> Result<(), String> {
    let root = make_workspace_temp_dir("snapshot");
    let project_root = root.join("sample-app");
    init_command(&[project_root.display().to_string()])?;

    let generated_files = collect_project_files(&project_root)?;
    assert_eq!(generated_files, expected_scaffold_files());

    let names = derive_project_names(&project_root)?;
    let expected_sdk_path = render_dependency_path(&project_root, &cli_sdk_path());

    let cargo_toml = fs::read_to_string(project_root.join("Cargo.toml"))
        .map_err(|err| format!("read Cargo.toml: {err}"))?;
    assert_eq!(
        cargo_toml,
        cargo_toml_contents(&names, &expected_sdk_path),
        "Cargo.toml drifted from the scaffold template"
    );

    let ergo_toml = fs::read_to_string(project_root.join("ergo.toml"))
        .map_err(|err| format!("read ergo.toml: {err}"))?;
    assert_eq!(
        ergo_toml,
        ergo_toml_contents(&names),
        "ergo.toml drifted from the scaffold template"
    );

    let readme = fs::read_to_string(project_root.join("README.md"))
        .map_err(|err| format!("read README.md: {err}"))?;
    assert_eq!(
        readme,
        readme_contents(&names),
        "README.md drifted from the scaffold template"
    );

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn scaffolded_project_builds_runs_and_validates() -> Result<(), String> {
    let root = make_workspace_temp_dir("e2e");
    let project_root = root.join("sample-app");
    init_command(&[project_root.display().to_string()])?;

    let run = run_cargo_project(&project_root, &["run", "--quiet"])?;
    if !run.status.success() {
        return Err(format!(
            "cargo run failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        ));
    }

    let capture_path = project_root.join("captures/historical.capture.json");
    let raw_capture =
        fs::read_to_string(&capture_path).map_err(|err| format!("read capture: {err}"))?;
    let capture_json: serde_json::Value =
        serde_json::from_str(&raw_capture).map_err(|err| format!("parse capture: {err}"))?;
    assert!(
        capture_json.get("egress_provenance").is_some(),
        "capture should include egress provenance"
    );
    let decisions = capture_json
        .get("decisions")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "capture decisions missing".to_string())?;
    let first_decision = decisions
        .first()
        .ok_or_else(|| "expected at least one decision".to_string())?;
    let intent_acks = first_decision
        .get("intent_acks")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "expected intent_acks array".to_string())?;
    assert!(!intent_acks.is_empty(), "expected at least one intent ack");

    let validate = run_cargo_project(&project_root, &["run", "--quiet", "--", "validate"])?;
    if !validate.status.success() {
        return Err(format!(
            "cargo run -- validate failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&validate.stdout),
            String::from_utf8_lossy(&validate.stderr)
        ));
    }
    let validate_stdout = String::from_utf8_lossy(&validate.stdout);
    assert!(validate_stdout.contains("validate ok project"));
    assert!(validate_stdout.contains("profiles:"));

    let profiles = run_cargo_project(&project_root, &["run", "--quiet", "--", "profiles"])?;
    if !profiles.status.success() {
        return Err(format!(
            "cargo run -- profiles failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&profiles.stdout),
            String::from_utf8_lossy(&profiles.stderr)
        ));
    }
    let profiles_stdout = String::from_utf8_lossy(&profiles.stdout);
    assert!(profiles_stdout.contains("profiles:"));
    assert!(profiles_stdout.contains("historical"));
    assert!(profiles_stdout.contains("live"));

    let doctor = run_cargo_project(&project_root, &["run", "--quiet", "--", "doctor"])?;
    if !doctor.status.success() {
        return Err(format!(
            "cargo run -- doctor failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&doctor.stdout),
            String::from_utf8_lossy(&doctor.stderr)
        ));
    }

    let live = run_cargo_project(&project_root, &["run", "--quiet", "--", "run", "live"])?;
    if !live.status.success() {
        return Err(format!(
            "cargo run -- run live failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&live.stdout),
            String::from_utf8_lossy(&live.stderr)
        ));
    }
    assert!(
        project_root.join("captures/live.capture.json").exists(),
        "expected live capture to be written"
    );

    let adapter_path = project_root.join("adapters/sample.yaml");
    let broken_adapter =
        fs::read_to_string(&adapter_path).map_err(|err| format!("read adapter: {err}"))?;
    fs::write(
        &adapter_path,
        broken_adapter.replace("publish_sample", "publish_mismatch"),
    )
    .map_err(|err| format!("write broken adapter: {err}"))?;

    let invalid = run_cargo_project(&project_root, &["run", "--quiet", "--", "validate"])?;
    assert!(
        !invalid.status.success(),
        "broken adapter validation should fail"
    );
    let invalid_stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(
        invalid_stderr.contains("COMP-") || invalid_stderr.contains("ACT-"),
        "expected rule id in validation failure stderr, got: {invalid_stderr}"
    );

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn init_requires_sdk_path_outside_workspace_checkout() {
    let root = make_temp_dir("sdk_path_required");
    let project_root = root.join("sample-app");

    let err = init_command(&[project_root.display().to_string()])
        .expect_err("outside-workspace init should require --sdk-path");
    assert!(err.contains("--sdk-path"), "unexpected error: {err}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_accepts_explicit_sdk_path_outside_workspace_checkout() -> Result<(), String> {
    let root = make_temp_dir("sdk_path_explicit");
    let project_root = root.join("sample-app");
    let sdk_path = cli_sdk_path();

    let message = init_command(&[
        project_root.display().to_string(),
        "--sdk-path".to_string(),
        sdk_path.display().to_string(),
    ])?;
    assert!(message.contains("initialized Ergo SDK project"));

    let cargo_toml = fs::read_to_string(project_root.join("Cargo.toml"))
        .map_err(|err| format!("read Cargo.toml: {err}"))?;
    assert!(
        !cargo_toml.contains(&format!("path = \"{}\"", normalize_path(&sdk_path))),
        "Cargo.toml should not write an absolute SDK path, got:\n{cargo_toml}"
    );
    assert!(
        cargo_toml.contains("ergo-sdk-rust = { path = "),
        "expected local SDK dependency, got:\n{cargo_toml}"
    );
    assert!(
        cargo_toml.contains(".."),
        "expected relative SDK path segments, got:\n{cargo_toml}"
    );

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn doctor_reports_missing_scaffold_file() -> Result<(), String> {
    let root = make_workspace_temp_dir("doctor_missing");
    let project_root = root.join("sample-app");
    init_command(&[project_root.display().to_string()])?;

    fs::remove_file(project_root.join("graphs/strategy.yaml"))
        .map_err(|err| format!("remove graph: {err}"))?;

    let doctor = run_cargo_project(&project_root, &["run", "--quiet", "--", "doctor"])?;
    assert!(
        !doctor.status.success(),
        "doctor should fail when graph is missing"
    );
    let stderr = String::from_utf8_lossy(&doctor.stderr);
    assert!(
        stderr.contains("doctor failed: expected 'graphs/strategy.yaml' to exist"),
        "unexpected stderr: {stderr}"
    );

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn doctor_reports_python_channel_syntax_error() -> Result<(), String> {
    let root = make_workspace_temp_dir("doctor_python_syntax");
    let project_root = root.join("sample-app");
    init_command(&[project_root.display().to_string()])?;

    fs::write(
        project_root.join("channels/ingress/live_feed.py"),
        "def broken(:\n    pass\n",
    )
    .map_err(|err| format!("write broken ingress script: {err}"))?;

    let doctor = run_cargo_project(&project_root, &["run", "--quiet", "--", "doctor"])?;
    assert!(
        !doctor.status.success(),
        "doctor should fail on invalid python scaffold channel"
    );
    let stderr = String::from_utf8_lossy(&doctor.stderr);
    assert!(
        stderr.contains("doctor failed: python3 could not compile 'channels/ingress/live_feed.py'"),
        "unexpected stderr: {stderr}"
    );

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn init_rejects_non_empty_target_without_force() -> Result<(), String> {
    let root = make_workspace_temp_dir("non_empty");
    let project_root = root.join("sample-app");
    fs::create_dir_all(&project_root).map_err(|err| format!("create project dir: {err}"))?;
    fs::write(project_root.join("keep.txt"), "existing")
        .map_err(|err| format!("write existing file: {err}"))?;

    let err = init_command(&[project_root.display().to_string()])
        .expect_err("non-empty target should fail");
    assert!(err.contains("not empty"));

    let _ = fs::remove_dir_all(root);
    Ok(())
}
