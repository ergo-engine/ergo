use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp_file(
    base: &std::path::Path,
    name: &str,
    contents: &str,
) -> Result<std::path::PathBuf, String> {
    let path = base.join(name);
    fs::write(&path, contents).map_err(|err| format!("write {}: {err}", path.display()))?;
    Ok(path)
}

fn cli_sdk_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("workspace root")
        .join("crates/prod/clients/sdk-rust")
}

#[test]
fn run_dispatch_returns_text_summary() -> Result<(), String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-cli-dispatch-run-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        r#"
kind: cluster
id: dispatch_run
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
    )?;
    let fixture = write_temp_file(
            &temp_dir,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        )?;
    let capture = temp_dir.join("capture.json");

    let args = vec![
        "run".to_string(),
        graph.to_string_lossy().to_string(),
        "--fixture".to_string(),
        fixture.to_string_lossy().to_string(),
        "--capture-output".to_string(),
        capture.to_string_lossy().to_string(),
    ];
    let result = dispatch_with_args(&args)?;
    let text = match result {
        DispatchOutput::Text(text) => text,
        DispatchOutput::Json(_) => return Err("expected text output".to_string()),
    };
    assert!(
        text.contains("episodes=1 events=1"),
        "unexpected text: {text}"
    );
    assert!(
        text.contains(&format!("capture artifact: {}", capture.display())),
        "unexpected text: {text}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn replay_dispatch_returns_text_summary() -> Result<(), String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-cli-dispatch-replay-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&temp_dir).map_err(|err| format!("create temp dir: {err}"))?;

    let graph = write_temp_file(
        &temp_dir,
        "graph.yaml",
        r#"
kind: cluster
id: dispatch_replay
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
    )?;
    let fixture = write_temp_file(
            &temp_dir,
            "fixture.jsonl",
            "{\"kind\":\"episode_start\",\"id\":\"E1\"}\n{\"kind\":\"event\",\"event\":{\"type\":\"Command\"}}\n",
        )?;
    let capture = temp_dir.join("capture.json");

    let run_args = vec![
        "run".to_string(),
        graph.to_string_lossy().to_string(),
        "--fixture".to_string(),
        fixture.to_string_lossy().to_string(),
        "--capture-output".to_string(),
        capture.to_string_lossy().to_string(),
    ];
    let _ = dispatch_with_args(&run_args)?;

    let replay_args = vec![
        "replay".to_string(),
        capture.to_string_lossy().to_string(),
        "--graph".to_string(),
        graph.to_string_lossy().to_string(),
    ];
    let result = dispatch_with_args(&replay_args)?;
    let text = match result {
        DispatchOutput::Text(text) => text,
        DispatchOutput::Json(_) => return Err("expected text output".to_string()),
    };
    assert!(
        text.contains("replay graph_id=dispatch_replay events=1"),
        "unexpected text: {text}"
    );
    assert!(
        text.contains("replay identity: match"),
        "unexpected text: {text}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn init_dispatch_routes_to_scaffold_command() -> Result<(), String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = std::env::temp_dir().join(format!(
        "ergo-cli-dispatch-init-{}-{}",
        std::process::id(),
        index
    ));
    let project_root = temp_dir.join("sample-app");
    let sdk_path = cli_sdk_path();

    let args = vec![
        "init".to_string(),
        project_root.to_string_lossy().to_string(),
        "--sdk-path".to_string(),
        sdk_path.to_string_lossy().to_string(),
    ];
    let result = dispatch_with_args(&args)?;
    let text = match result {
        DispatchOutput::Text(text) => text,
        DispatchOutput::Json(_) => return Err("expected text output".to_string()),
    };

    assert!(text.contains("initialized Ergo SDK project"));
    assert!(project_root.join("Cargo.toml").exists());

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn help_init_dispatch_returns_init_notes() -> Result<(), String> {
    let result = dispatch_with_args(&["help".to_string(), "init".to_string()])?;
    let text = match result {
        DispatchOutput::Text(text) => text,
        DispatchOutput::Json(_) => return Err("expected text output".to_string()),
    };

    assert!(text.contains("ergo init <project-dir>"));
    assert!(text.contains("use --sdk-path outside the checkout"));
    assert!(text.contains("POSIX 'sh'"));
    Ok(())
}
