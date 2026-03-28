//! egress::process::tests
//!
//! Purpose:
//! - Keep protocol and lifecycle contract tests for the host egress process
//!   seam out of the production file while locking the wire format, timing
//!   heuristics, and child-process behavior this module exposes.

use super::*;

use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(prefix: &str) -> Result<PathBuf, String> {
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "ergo-egress-process-{prefix}-{}-{}",
        std::process::id(),
        index
    ));
    fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
    Ok(dir)
}

fn write_script(base: &Path, name: &str, body: &str) -> Result<PathBuf, String> {
    let path = base.join(name);
    fs::write(&path, body).map_err(|err| format!("write script '{}': {err}", path.display()))?;
    let mut perms = fs::metadata(&path)
        .map_err(|err| format!("read script metadata '{}': {err}", path.display()))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms)
        .map_err(|err| format!("chmod script '{}': {err}", path.display()))?;
    Ok(path)
}

fn wait_for_path(path: &Path, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err(format!("timed out waiting for '{}'", path.display()))
}

fn current_process_group_id() -> Result<u32, String> {
    let output = Command::new("ps")
        .args(["-o", "pgid=", "-p", &std::process::id().to_string()])
        .output()
        .map_err(|err| format!("run ps for parent pgid: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "run ps for parent pgid exited with status {}",
            output.status
        ));
    }
    let raw = String::from_utf8(output.stdout)
        .map_err(|err| format!("decode parent pgid output: {err}"))?;
    raw.trim()
        .parse::<u32>()
        .map_err(|err| format!("parse parent pgid '{}': {err}", raw.trim()))
}

fn config_for_script(script: &Path, ack_timeout: Duration) -> EgressConfig {
    EgressConfig {
        default_ack_timeout: ack_timeout,
        channels: BTreeMap::from([(
            "broker".to_string(),
            EgressChannelConfig::Process {
                command: vec!["sh".to_string(), script.display().to_string()],
            },
        )]),
        routes: BTreeMap::from([(
            "place_order".to_string(),
            super::super::EgressRoute {
                channel: "broker".to_string(),
                ack_timeout: None,
            },
        )]),
    }
}

fn sample_intent(intent_id: &str) -> IntentRecord {
    IntentRecord {
        kind: "place_order".to_string(),
        intent_id: intent_id.to_string(),
        fields: vec![
            IntentField {
                name: "symbol".to_string(),
                value: Value::String("EURUSD".to_string()),
            },
            IntentField {
                name: "qty".to_string(),
                value: Value::Number(100.0),
            },
        ],
    }
}

#[test]
fn egress_process_error_display_contract_is_stable() {
    let cases = [
        (
            EgressProcessError::InvalidConfig(
                "egress channel 'broker' has empty process command".to_string(),
            ),
            "egress channel 'broker' has empty process command".to_string(),
        ),
        (
            EgressProcessError::Startup {
                channel: "broker".to_string(),
                detail: "spawn failed".to_string(),
            },
            "egress channel 'broker' startup failed: spawn failed".to_string(),
        ),
        (
            EgressProcessError::Protocol {
                channel: "broker".to_string(),
                detail: "unexpected frame".to_string(),
            },
            "egress channel 'broker' protocol violation: unexpected frame".to_string(),
        ),
        (
            EgressProcessError::Io {
                channel: "broker".to_string(),
                detail: "broken pipe".to_string(),
            },
            "egress channel 'broker' I/O failure: broken pipe".to_string(),
        ),
        (
            EgressProcessError::Timeout {
                channel: "broker".to_string(),
                intent_id: "eid1:sha256:test".to_string(),
                timeout: Duration::from_millis(250),
            },
            "egress channel 'broker' timed out waiting for durable-accept ack for intent 'eid1:sha256:test' after 250ms".to_string(),
        ),
        (
            EgressProcessError::PendingAcks {
                channel: "broker".to_string(),
                detail: "in-flight intent IDs: {\"eid1\"}".to_string(),
            },
            "egress channel 'broker' has unresolved ack state: in-flight intent IDs: {\"eid1\"}"
                .to_string(),
        ),
    ];

    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
    }
}

#[test]
fn outbound_message_json_shapes_are_stable() {
    let fields = intent_fields_to_json_object(&[
        IntentField {
            name: "active".to_string(),
            value: Value::Bool(true),
        },
        IntentField {
            name: "levels".to_string(),
            value: Value::Series(vec![1.0, 2.5]),
        },
        IntentField {
            name: "price".to_string(),
            value: Value::Number(101.5),
        },
        IntentField {
            name: "symbol".to_string(),
            value: Value::String("EURUSD".to_string()),
        },
    ]);

    let intent = OutboundMessage::Intent {
        intent_id: "eid1:sha256:test".to_string(),
        kind: "place_order".to_string(),
        fields,
    };
    assert_eq!(
        serde_json::to_string(&intent).expect("intent should serialize"),
        r#"{"type":"intent","intent_id":"eid1:sha256:test","kind":"place_order","fields":{"active":true,"levels":[1.0,2.5],"price":101.5,"symbol":"EURUSD"}}"#
    );

    assert_eq!(
        serde_json::to_string(&OutboundMessage::End).expect("end should serialize"),
        r#"{"type":"end"}"#
    );
}

#[test]
fn inbound_message_json_shapes_are_stable() {
    let ready = serde_json::from_str::<InboundMessage>(
        r#"{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["cancel_order","place_order"]}"#,
    )
    .expect("ready frame should deserialize");
    match ready {
        InboundMessage::Ready {
            protocol,
            handled_kinds,
        } => {
            assert_eq!(protocol, "ergo-egress.v1");
            assert_eq!(handled_kinds, vec!["cancel_order", "place_order"]);
        }
        other => panic!("unexpected inbound message: {other:?}"),
    }

    let intent_ack = serde_json::from_str::<InboundMessage>(
        r#"{"type":"intent_ack","intent_id":"eid1:sha256:test","status":"accepted","acceptance":"durable","egress_ref":"broker-123"}"#,
    )
    .expect("intent ack should deserialize");
    match intent_ack {
        InboundMessage::IntentAck {
            intent_id,
            status,
            acceptance,
            egress_ref,
        } => {
            assert_eq!(intent_id, "eid1:sha256:test");
            assert_eq!(status, "accepted");
            assert_eq!(acceptance, "durable");
            assert_eq!(egress_ref.as_deref(), Some("broker-123"));
        }
        other => panic!("unexpected inbound message: {other:?}"),
    }

    let intent_ack_without_ref = serde_json::from_str::<InboundMessage>(
        r#"{"type":"intent_ack","intent_id":"eid1:sha256:no-ref","status":"accepted","acceptance":"durable"}"#,
    )
    .expect("intent ack without ref should deserialize");
    match intent_ack_without_ref {
        InboundMessage::IntentAck { egress_ref, .. } => assert_eq!(egress_ref, None),
        other => panic!("unexpected inbound message: {other:?}"),
    }
}

#[test]
fn common_value_to_json_preserves_current_wire_shapes() {
    assert_eq!(common_value_to_json(&Value::Number(3.5)), json!(3.5));
    assert_eq!(
        common_value_to_json(&Value::Series(vec![1.0, 2.5])),
        json!([1.0, 2.5])
    );
    assert_eq!(common_value_to_json(&Value::Bool(true)), json!(true));
    assert_eq!(
        common_value_to_json(&Value::String("EURUSD".to_string())),
        json!("EURUSD")
    );
}

#[test]
fn pending_ack_probe_timing_constants_are_stable() {
    assert_eq!(
        PENDING_ACK_STRAGGLER_PROBE_WINDOW,
        Duration::from_millis(20)
    );
    assert_eq!(
        PENDING_ACK_STRAGGLER_PROBE_POLL_INTERVAL,
        Duration::from_millis(1)
    );
}

#[test]
fn dispatch_single_intent_ack_succeeds() -> Result<(), String> {
    let dir = temp_dir("ack-ok")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
  id=$(printf '%s\n' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
  printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable","egress_ref":"broker-1"}\n' "$id"
done
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    runtime
        .start_channels()
        .map_err(|err| format!("start channels: {err}"))?;
    let ack = runtime
        .dispatch_intent(&sample_intent("eid1:sha256:one"))
        .map_err(|err| format!("dispatch: {err}"))?;
    assert_eq!(ack.intent_id, "eid1:sha256:one");
    assert_eq!(ack.channel, "broker");
    assert_eq!(ack.status, "accepted");
    assert_eq!(ack.acceptance, "durable");
    assert_eq!(ack.egress_ref.as_deref(), Some("broker-1"));
    runtime
        .shutdown_channels()
        .map_err(|err| format!("shutdown channels: {err}"))?;
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn dispatch_timeout_returns_timeout_error() -> Result<(), String> {
    let dir = temp_dir("ack-timeout")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
  sleep 1
done
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_millis(50)));
    runtime
        .start_channels()
        .map_err(|err| format!("start channels: {err}"))?;
    let err = runtime
        .dispatch_intent(&sample_intent("eid1:sha256:timeout"))
        .expect_err("timeout should fail dispatch");
    assert!(matches!(err, EgressProcessError::Timeout { .. }));
    runtime
        .shutdown_channels()
        .map_err(|err| format!("shutdown channels: {err}"))?;
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn invalid_ack_returns_protocol_error_and_quiesces_channels() -> Result<(), String> {
    let dir = temp_dir("ack-invalid")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
  printf '%s\n' '{"type":"intent_ack","intent_id":"wrong","status":"accepted","acceptance":"durable"}'
done
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    runtime
        .start_channels()
        .map_err(|err| format!("start channels: {err}"))?;
    let err = runtime
        .dispatch_intent(&sample_intent("eid1:sha256:expected"))
        .expect_err("mismatched intent id should fail");
    assert!(matches!(err, EgressProcessError::Protocol { .. }));

    let second = runtime
        .dispatch_intent(&sample_intent("eid1:sha256:after-protocol"))
        .expect_err("protocol failure should quiesce channels");
    assert!(matches!(second, EgressProcessError::Startup { .. }));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn ack_must_be_accepted_and_durable() -> Result<(), String> {
    let dir = temp_dir("ack-not-durable")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
  id=$(printf '%s\n' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
  printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"volatile"}\n' "$id"
done
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    runtime
        .start_channels()
        .map_err(|err| format!("start channels: {err}"))?;
    let err = runtime
        .dispatch_intent(&sample_intent("eid1:sha256:not-durable"))
        .expect_err("non-durable ack must fail");
    assert!(matches!(err, EgressProcessError::Protocol { .. }));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn startup_without_ready_fails() -> Result<(), String> {
    let dir = temp_dir("startup-fail")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
exit 0
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    let err = runtime
        .start_channels()
        .expect_err("missing ready frame must fail startup");
    assert!(matches!(err, EgressProcessError::Startup { .. }));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn startup_fails_when_ready_protocol_mismatches() -> Result<(), String> {
    let dir = temp_dir("startup-proto-mismatch")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v0","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
done
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    let err = runtime
        .start_channels()
        .expect_err("protocol mismatch must fail startup");
    assert!(matches!(err, EgressProcessError::Protocol { .. }));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn startup_fails_when_ready_missing_routed_kind() -> Result<(), String> {
    let dir = temp_dir("startup-missing-kind")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["cancel_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
done
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    let err = runtime
        .start_channels()
        .expect_err("missing routed kind must fail startup");
    assert!(matches!(err, EgressProcessError::Protocol { .. }));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn startup_fails_when_ready_contains_duplicate_handled_kinds() -> Result<(), String> {
    let dir = temp_dir("startup-duplicate-kind")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order","place_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
done
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    let err = runtime
        .start_channels()
        .expect_err("duplicate handled kinds must fail startup");
    assert!(matches!(err, EgressProcessError::Protocol { .. }));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn multiple_intents_dispatch_and_ack() -> Result<(), String> {
    let dir = temp_dir("ack-multi")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
  id=$(printf '%s\n' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
  printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}\n' "$id"
done
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    runtime
        .start_channels()
        .map_err(|err| format!("start channels: {err}"))?;
    let first = runtime
        .dispatch_intent(&sample_intent("eid1:sha256:first"))
        .map_err(|err| format!("dispatch first: {err}"))?;
    let second = runtime
        .dispatch_intent(&sample_intent("eid1:sha256:second"))
        .map_err(|err| format!("dispatch second: {err}"))?;
    assert_eq!(first.intent_id, "eid1:sha256:first");
    assert_eq!(second.intent_id, "eid1:sha256:second");
    runtime
        .shutdown_channels()
        .map_err(|err| format!("shutdown channels: {err}"))?;
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn timeout_quiesces_channels_for_consistency() -> Result<(), String> {
    let dir = temp_dir("quiesce-timeout")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
  sleep 1
done
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_millis(50)));
    runtime
        .start_channels()
        .map_err(|err| format!("start channels: {err}"))?;
    let err = runtime
        .dispatch_intent(&sample_intent("eid1:sha256:timeout-quiesce"))
        .expect_err("timeout should fail dispatch");
    assert!(matches!(err, EgressProcessError::Timeout { .. }));

    let second = runtime
        .dispatch_intent(&sample_intent("eid1:sha256:after-timeout"))
        .expect_err("quiesced runtime should reject further dispatch");
    assert!(matches!(second, EgressProcessError::Startup { .. }));
    assert!(
        runtime.assert_no_pending_acks(false).is_ok(),
        "quiesced runtime should not expose pending ack state"
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn pending_ack_assertion_fails_when_extra_stdout_frame_is_buffered() -> Result<(), String> {
    let dir = temp_dir("pending-buffered-frame")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
  id=$(printf '%s\n' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
  printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}\n' "$id"
  printf '%s\n' '{"type":"intent_ack","intent_id":"unexpected","status":"accepted","acceptance":"durable"}'
done
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    runtime
        .start_channels()
        .map_err(|err| format!("start channels: {err}"))?;
    runtime
        .dispatch_intent(&sample_intent("eid1:sha256:buffered"))
        .map_err(|err| format!("dispatch: {err}"))?;

    let err = runtime
        .assert_no_pending_acks(false)
        .expect_err("buffered stdout frame must fail pending-ack invariant");
    assert!(matches!(err, EgressProcessError::PendingAcks { .. }));

    runtime
        .shutdown_channels()
        .map_err(|err| format!("shutdown channels: {err}"))?;
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn pending_ack_probe_catches_late_straggler_within_current_window() -> Result<(), String> {
    let dir = temp_dir("pending-late-straggler")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
  id=$(printf '%s\n' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
  printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}\n' "$id"
  sleep 0.005
  printf '%s\n' '{"type":"intent_ack","intent_id":"late","status":"accepted","acceptance":"durable"}'
done
"#,
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    runtime
        .start_channels()
        .map_err(|err| format!("start channels: {err}"))?;
    runtime
        .dispatch_intent(&sample_intent("eid1:sha256:late"))
        .map_err(|err| format!("dispatch: {err}"))?;

    let err = runtime
        .assert_no_pending_acks(false)
        .expect_err("late straggler inside probe window must fail invariant");
    assert!(matches!(err, EgressProcessError::PendingAcks { .. }));

    runtime
        .shutdown_channels()
        .map_err(|err| format!("shutdown channels: {err}"))?;
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn host_managed_egress_channel_uses_separate_process_group() -> Result<(), String> {
    let dir = temp_dir("pgid-isolation")?;
    let pgid_path = dir.join("egress.pgid");
    let script = write_script(
        &dir,
        "egress.sh",
        &format!(
            r#"#!/bin/sh
pgid_path='{pgid_path}'
ps -o pgid= -p $$ | tr -d ' ' > "$pgid_path"
printf '%s\n' '{{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
done
"#,
            pgid_path = pgid_path.display()
        ),
    )?;

    let mut runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    runtime
        .start_channels()
        .map_err(|err| format!("start channels: {err}"))?;
    wait_for_path(&pgid_path, Duration::from_secs(1))?;

    let parent_pgid = current_process_group_id()?;
    let child_pgid = fs::read_to_string(&pgid_path)
        .map_err(|err| format!("read child pgid '{}': {err}", pgid_path.display()))?
        .trim()
        .parse::<u32>()
        .map_err(|err| format!("parse child pgid '{}': {err}", pgid_path.display()))?;
    assert_ne!(
        child_pgid, parent_pgid,
        "egress child should run in its own process group"
    );

    runtime
        .shutdown_channels()
        .map_err(|err| format!("shutdown channels: {err}"))?;
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn host_stop_pending_ack_check_tolerates_eof_after_child_exit() -> Result<(), String> {
    let dir = temp_dir("host-stop-eof")?;
    let script = write_script(
        &dir,
        "egress.sh",
        r#"#!/bin/sh
printf '%s\n' '{"type":"ready","protocol":"ergo-egress.v1","handled_kinds":["place_order"]}'
while IFS= read -r line; do
  echo "$line" | grep -q '"type":"end"' && exit 0
  id=$(printf '%s\n' "$line" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
  printf '{"type":"intent_ack","intent_id":"%s","status":"accepted","acceptance":"durable"}\n' "$id"
  exit 0
done
"#,
    )?;

    let mut strict_runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    strict_runtime
        .start_channels()
        .map_err(|err| format!("start strict runtime: {err}"))?;
    strict_runtime
        .dispatch_intent(&sample_intent("eid1:sha256:strict"))
        .map_err(|err| format!("dispatch strict intent: {err}"))?;
    thread::sleep(PENDING_ACK_STRAGGLER_PROBE_WINDOW);
    let err = strict_runtime
        .assert_no_pending_acks(false)
        .expect_err("normal completion should still reject child EOF");
    assert!(matches!(err, EgressProcessError::PendingAcks { .. }));

    let mut stop_runtime = EgressRuntime::new(config_for_script(&script, Duration::from_secs(1)));
    stop_runtime
        .start_channels()
        .map_err(|err| format!("start stop runtime: {err}"))?;
    stop_runtime
        .dispatch_intent(&sample_intent("eid1:sha256:stop"))
        .map_err(|err| format!("dispatch stop intent: {err}"))?;
    thread::sleep(PENDING_ACK_STRAGGLER_PROBE_WINDOW);
    stop_runtime
        .assert_no_pending_acks(true)
        .map_err(|err| format!("host-stop pending-ack check: {err}"))?;
    stop_runtime
        .shutdown_channels()
        .map_err(|err| format!("shutdown stop runtime: {err}"))?;

    let _ = fs::remove_dir_all(dir);
    Ok(())
}
