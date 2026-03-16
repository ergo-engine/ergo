use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ergo_runtime::common::{IntentField, IntentRecord, Value};
use ergo_supervisor::CapturedIntentAck;
use serde::{Deserialize, Serialize};

use super::{EgressChannelConfig, EgressConfig};

const DEFAULT_EGRESS_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_EGRESS_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const CHANNEL_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub enum EgressProcessError {
    InvalidConfig(String),
    Startup {
        channel: String,
        detail: String,
    },
    Protocol {
        channel: String,
        detail: String,
    },
    Io {
        channel: String,
        detail: String,
    },
    Timeout {
        channel: String,
        intent_id: String,
        timeout: Duration,
    },
}

impl std::fmt::Display for EgressProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(detail) => write!(f, "{detail}"),
            Self::Startup { channel, detail } => {
                write!(f, "egress channel '{channel}' startup failed: {detail}")
            }
            Self::Protocol { channel, detail } => {
                write!(f, "egress channel '{channel}' protocol violation: {detail}")
            }
            Self::Io { channel, detail } => {
                write!(f, "egress channel '{channel}' I/O failure: {detail}")
            }
            Self::Timeout {
                channel,
                intent_id,
                timeout,
            } => write!(
                f,
                "egress channel '{channel}' timed out waiting for durable-accept ack for intent '{intent_id}' after {}ms",
                timeout.as_millis()
            ),
        }
    }
}

impl std::error::Error for EgressProcessError {}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutboundMessage {
    Intent {
        intent_id: String,
        kind: String,
        fields: BTreeMap<String, serde_json::Value>,
    },
    End,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InboundMessage {
    Ready,
    IntentAck {
        intent_id: String,
        status: String,
        acceptance: String,
        #[serde(default)]
        egress_ref: Option<String>,
    },
}

#[derive(Debug)]
enum ChannelObservation {
    Line(String),
    Eof,
    ReadError(String),
}

struct EgressChannelHandle {
    channel_id: String,
    child: Child,
    stdin: ChildStdin,
    stdout_rx: Receiver<ChannelObservation>,
    stderr_handle: Option<JoinHandle<String>>,
}

impl EgressChannelHandle {
    fn spawn(channel_id: &str, config: &EgressChannelConfig) -> Result<Self, EgressProcessError> {
        let command = match config {
            EgressChannelConfig::Process { command } => command,
        };
        if command.is_empty() {
            return Err(EgressProcessError::InvalidConfig(format!(
                "egress channel '{channel_id}' has empty process command"
            )));
        }

        let mut child = Command::new(&command[0]);
        child.args(&command[1..]);
        child.stdin(Stdio::piped());
        child.stdout(Stdio::piped());
        child.stderr(Stdio::piped());
        let mut child = child.spawn().map_err(|err| EgressProcessError::Startup {
            channel: channel_id.to_string(),
            detail: format!("spawn {:?}: {err}", command),
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| EgressProcessError::Startup {
                channel: channel_id.to_string(),
                detail: "child process did not provide stdin".to_string(),
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| EgressProcessError::Startup {
                channel: channel_id.to_string(),
                detail: "child process did not provide stdout".to_string(),
            })?;
        let stderr_handle = child.stderr.take().map(drain_stderr);
        let stdout_rx = spawn_stdout_reader(stdout);

        Ok(Self {
            channel_id: channel_id.to_string(),
            child,
            stdin,
            stdout_rx,
            stderr_handle,
        })
    }

    fn wait_ready(&mut self, timeout: Duration) -> Result<(), EgressProcessError> {
        let observation = recv_observation(&self.stdout_rx, Some(timeout)).map_err(|failure| {
            self.force_terminate();
            match failure {
                RecvTimeoutError::Timeout => EgressProcessError::Startup {
                    channel: self.channel_id.clone(),
                    detail: format!(
                        "channel did not emit ready frame within {}ms",
                        timeout.as_millis()
                    ),
                },
                RecvTimeoutError::Disconnected => EgressProcessError::Startup {
                    channel: self.channel_id.clone(),
                    detail: "stdout reader disconnected before ready handshake".to_string(),
                },
            }
        })?;

        match observation {
            ChannelObservation::Line(line) => {
                let message = serde_json::from_str::<InboundMessage>(line.trim_end_matches('\n'))
                    .map_err(|err| {
                    self.force_terminate();
                    EgressProcessError::Protocol {
                        channel: self.channel_id.clone(),
                        detail: format!("invalid startup frame: {err}"),
                    }
                })?;

                match message {
                    InboundMessage::Ready => Ok(()),
                    InboundMessage::IntentAck { .. } => {
                        self.force_terminate();
                        Err(EgressProcessError::Protocol {
                            channel: self.channel_id.clone(),
                            detail: "expected ready frame, got intent_ack".to_string(),
                        })
                    }
                }
            }
            ChannelObservation::Eof => {
                self.force_terminate();
                Err(EgressProcessError::Startup {
                    channel: self.channel_id.clone(),
                    detail: "channel closed stdout before ready handshake".to_string(),
                })
            }
            ChannelObservation::ReadError(detail) => {
                self.force_terminate();
                Err(EgressProcessError::Io {
                    channel: self.channel_id.clone(),
                    detail,
                })
            }
        }
    }

    fn dispatch_intent(
        &mut self,
        intent: &IntentRecord,
        timeout: Duration,
    ) -> Result<CapturedIntentAck, EgressProcessError> {
        let outbound = OutboundMessage::Intent {
            intent_id: intent.intent_id.clone(),
            kind: intent.kind.clone(),
            fields: intent_fields_to_json_object(&intent.fields),
        };
        let payload = serde_json::to_string(&outbound).map_err(|err| EgressProcessError::Io {
            channel: self.channel_id.clone(),
            detail: format!("serialize intent payload: {err}"),
        })?;

        writeln!(self.stdin, "{payload}").map_err(|err| EgressProcessError::Io {
            channel: self.channel_id.clone(),
            detail: format!("write intent payload: {err}"),
        })?;
        self.stdin.flush().map_err(|err| EgressProcessError::Io {
            channel: self.channel_id.clone(),
            detail: format!("flush intent payload: {err}"),
        })?;

        let observation =
            recv_observation(&self.stdout_rx, Some(timeout)).map_err(|failure| match failure {
                RecvTimeoutError::Timeout => EgressProcessError::Timeout {
                    channel: self.channel_id.clone(),
                    intent_id: intent.intent_id.clone(),
                    timeout,
                },
                RecvTimeoutError::Disconnected => EgressProcessError::Io {
                    channel: self.channel_id.clone(),
                    detail: "stdout reader disconnected while waiting for ack".to_string(),
                },
            })?;

        match observation {
            ChannelObservation::Line(line) => {
                let message = serde_json::from_str::<InboundMessage>(line.trim_end_matches('\n'))
                    .map_err(|err| EgressProcessError::Protocol {
                    channel: self.channel_id.clone(),
                    detail: format!("invalid ack frame: {err}"),
                })?;

                match message {
                    InboundMessage::Ready => Err(EgressProcessError::Protocol {
                        channel: self.channel_id.clone(),
                        detail: "unexpected ready frame after startup".to_string(),
                    }),
                    InboundMessage::IntentAck {
                        intent_id,
                        status,
                        acceptance,
                        egress_ref,
                    } => {
                        if intent_id != intent.intent_id {
                            return Err(EgressProcessError::Protocol {
                                channel: self.channel_id.clone(),
                                detail: format!(
                                    "ack intent_id mismatch: expected '{}', got '{}'",
                                    intent.intent_id, intent_id
                                ),
                            });
                        }

                        if status != "accepted" || acceptance != "durable" {
                            return Err(EgressProcessError::Protocol {
                                channel: self.channel_id.clone(),
                                detail: format!(
                                    "ack must be accepted+durable, got status='{status}' acceptance='{acceptance}'"
                                ),
                            });
                        }

                        Ok(CapturedIntentAck {
                            intent_id,
                            channel: self.channel_id.clone(),
                            status,
                            acceptance,
                            egress_ref,
                        })
                    }
                }
            }
            ChannelObservation::Eof => Err(EgressProcessError::Io {
                channel: self.channel_id.clone(),
                detail: "channel closed stdout while waiting for ack".to_string(),
            }),
            ChannelObservation::ReadError(detail) => Err(EgressProcessError::Io {
                channel: self.channel_id.clone(),
                detail,
            }),
        }
    }

    fn shutdown(&mut self, timeout: Duration) -> Result<(), EgressProcessError> {
        let end = OutboundMessage::End;
        if let Ok(payload) = serde_json::to_string(&end) {
            let _ = writeln!(self.stdin, "{payload}");
            let _ = self.stdin.flush();
        }

        wait_for_exit(&mut self.child, timeout).map_err(|err| EgressProcessError::Io {
            channel: self.channel_id.clone(),
            detail: format!("wait for graceful shutdown: {err}"),
        })?;
        Ok(())
    }

    fn force_terminate(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = take_stderr(self.stderr_handle.take());
    }
}

impl Drop for EgressChannelHandle {
    fn drop(&mut self) {
        self.force_terminate();
    }
}

pub struct EgressRuntime {
    config: EgressConfig,
    channels: BTreeMap<String, EgressChannelHandle>,
    started: bool,
}

impl EgressRuntime {
    pub fn new(config: EgressConfig) -> Self {
        Self {
            config,
            channels: BTreeMap::new(),
            started: false,
        }
    }

    pub fn config(&self) -> &EgressConfig {
        &self.config
    }

    pub fn route_kind_set(&self) -> std::collections::HashSet<String> {
        self.config.routes.keys().cloned().collect()
    }

    pub fn start_channels(&mut self) -> Result<(), EgressProcessError> {
        if self.started {
            return Ok(());
        }

        let mut started = BTreeMap::new();
        for (channel_id, channel_config) in &self.config.channels {
            let mut handle = EgressChannelHandle::spawn(channel_id, channel_config)?;
            handle.wait_ready(DEFAULT_EGRESS_STARTUP_TIMEOUT)?;
            started.insert(channel_id.clone(), handle);
        }
        self.channels = started;
        self.started = true;
        Ok(())
    }

    pub fn dispatch_intent(
        &mut self,
        intent: &IntentRecord,
    ) -> Result<CapturedIntentAck, EgressProcessError> {
        if !self.started {
            return Err(EgressProcessError::Startup {
                channel: "all".to_string(),
                detail: "egress channels are not started".to_string(),
            });
        }

        let route = self
            .config
            .routes
            .get(&intent.kind)
            .ok_or_else(|| {
                EgressProcessError::InvalidConfig(format!(
                    "no egress route configured for intent kind '{}'",
                    intent.kind
                ))
            })?
            .clone();

        let timeout = route.ack_timeout.unwrap_or(self.config.default_ack_timeout);
        let handle = self.channels.get_mut(&route.channel).ok_or_else(|| {
            EgressProcessError::InvalidConfig(format!(
                "route for intent kind '{}' references unknown channel '{}'",
                intent.kind, route.channel
            ))
        })?;

        handle.dispatch_intent(intent, timeout)
    }

    pub fn assert_no_pending_acks(&self) -> Result<(), EgressProcessError> {
        if !self.started {
            return Ok(());
        }
        Ok(())
    }

    pub fn shutdown_channels(&mut self) -> Result<(), EgressProcessError> {
        if !self.started {
            return Ok(());
        }

        let mut first_error = None;
        for handle in self.channels.values_mut() {
            if let Err(err) = handle.shutdown(DEFAULT_EGRESS_SHUTDOWN_TIMEOUT) {
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
        }
        self.channels.clear();
        self.started = false;

        if let Some(err) = first_error {
            return Err(err);
        }
        Ok(())
    }
}

fn intent_fields_to_json_object(fields: &[IntentField]) -> BTreeMap<String, serde_json::Value> {
    fields
        .iter()
        .map(|field| (field.name.clone(), common_value_to_json(&field.value)))
        .collect()
}

fn common_value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Number(number) => serde_json::json!(number),
        Value::Series(series) => serde_json::json!(series),
        Value::Bool(boolean) => serde_json::json!(boolean),
        Value::String(string) => serde_json::json!(string),
    }
}

fn spawn_stdout_reader(stdout: ChildStdout) -> Receiver<ChannelObservation> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = tx.send(ChannelObservation::Eof);
                    break;
                }
                Ok(_) => {
                    if tx.send(ChannelObservation::Line(line)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ = tx.send(ChannelObservation::ReadError(err.to_string()));
                    break;
                }
            }
        }
    });
    rx
}

fn recv_observation(
    receiver: &Receiver<ChannelObservation>,
    timeout: Option<Duration>,
) -> Result<ChannelObservation, RecvTimeoutError> {
    match timeout {
        Some(timeout) => receiver.recv_timeout(timeout),
        None => receiver.recv().map_err(|_| RecvTimeoutError::Disconnected),
    }
}

fn drain_stderr(stderr: impl Read + Send + 'static) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output);
        output
    })
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Result<Option<ExitStatus>, String> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(Some(status)),
            Ok(None) => {}
            Err(err) => return Err(err.to_string()),
        }

        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        thread::sleep(CHANNEL_POLL_INTERVAL);
    }
}

fn take_stderr(handle: Option<JoinHandle<String>>) -> String {
    handle
        .map(|stderr_handle| stderr_handle.join().unwrap_or_default())
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
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
        fs::write(&path, body)
            .map_err(|err| format!("write script '{}': {err}", path.display()))?;
        let mut perms = fs::metadata(&path)
            .map_err(|err| format!("read script metadata '{}': {err}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)
            .map_err(|err| format!("chmod script '{}': {err}", path.display()))?;
        Ok(path)
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
    fn dispatch_single_intent_ack_succeeds() -> Result<(), String> {
        let dir = temp_dir("ack-ok")?;
        let script = write_script(
            &dir,
            "egress.sh",
            r#"#!/bin/sh
printf '%s\n' '{"type":"ready"}'
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
printf '%s\n' '{"type":"ready"}'
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
    fn invalid_ack_returns_protocol_error() -> Result<(), String> {
        let dir = temp_dir("ack-invalid")?;
        let script = write_script(
            &dir,
            "egress.sh",
            r#"#!/bin/sh
printf '%s\n' '{"type":"ready"}'
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
        runtime
            .shutdown_channels()
            .map_err(|err| format!("shutdown channels: {err}"))?;
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
    fn multiple_intents_dispatch_and_ack() -> Result<(), String> {
        let dir = temp_dir("ack-multi")?;
        let script = write_script(
            &dir,
            "egress.sh",
            r#"#!/bin/sh
printf '%s\n' '{"type":"ready"}'
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
}
