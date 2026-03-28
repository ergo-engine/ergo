//! egress::process
//!
//! Purpose:
//! - Run host-managed egress child processes for the `ergo-egress.v1` wire
//!   protocol, translating intent records into outbound JSON frames and
//!   validating durable-accept acknowledgments.
//!
//! Owns:
//! - Process lifecycle for egress channels: spawn, ready handshake, dispatch,
//!   pending-ack checks, bounded shutdown, and force-kill cleanup.
//! - The wire contract for `OutboundMessage`, `InboundMessage`, and the host's
//!   `Value` -> JSON projection sent to egress processes.
//! - Host operational timing policy for this seam: 5s startup/shutdown bounds
//!   plus a 20ms straggler probe with 1ms polling when finalization asserts
//!   that no ack frames remain buffered.
//!
//! Does not own:
//! - Egress route/provenance parsing; `config.rs` owns the authored config
//!   surface.
//! - Live-path validation; `validation.rs` owns pre-run egress checks.
//! - Host interruption taxonomy or `HostedStepError` mapping; `runner.rs` and
//!   `error.rs` decide how these failures surface at higher layers.
//! - Replay semantics; replay never launches egress channels.
//!
//! Connects to:
//! - `runner.rs`, which starts `EgressRuntime`, dispatches intents, and
//!   finalizes hosted runs through pending-ack and shutdown checks.
//! - Project-owned egress programs implementing the `ergo-egress.v1` child
//!   process protocol.
//! - `error.rs`, which currently stringifies `EgressProcessError` into
//!   `HostedStepError`.
//!
//! Safety notes:
//! - The ready handshake must attest the exact protocol version and all routed
//!   kinds assigned to a channel before live execution begins.
//! - Dispatch waits only for `status="accepted"` plus
//!   `acceptance="durable"`; timeout/protocol/I/O failures quiesce the failing
//!   channel and then all channels for consistency.
//! - The straggler probe in `assert_no_pending_acks` is host operational
//!   policy, not protocol truth: a short 20ms / 1ms heuristic to catch late
//!   stdout frames before capture finalization.
//! - `InboundMessage` currently spans both startup and dispatch phases; that
//!   lack of phase-typed protocol messages is deferred second-pass debt.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ergo_runtime::common::{IntentField, IntentRecord, Value};
use ergo_supervisor::CapturedIntentAck;
use serde::{Deserialize, Serialize};

use super::{EgressChannelConfig, EgressConfig};

const DEFAULT_EGRESS_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_EGRESS_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const CHANNEL_POLL_INTERVAL: Duration = Duration::from_millis(10);
const EGRESS_PROTOCOL: &str = "ergo-egress.v1";
const PENDING_ACK_STRAGGLER_PROBE_WINDOW: Duration = Duration::from_millis(20);
const PENDING_ACK_STRAGGLER_PROBE_POLL_INTERVAL: Duration = Duration::from_millis(1);

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
    PendingAcks {
        channel: String,
        detail: String,
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
            Self::PendingAcks { channel, detail } => write!(
                f,
                "egress channel '{channel}' has unresolved ack state: {detail}"
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
    Ready {
        protocol: String,
        handled_kinds: Vec<String>,
    },
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

fn configure_host_managed_child(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        command.process_group(0);
    }
}

fn kill_host_managed_child(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = unsafe { libc::killpg(child.id() as libc::pid_t, libc::SIGKILL) };
    }

    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
}

struct EgressChannelHandle {
    channel_id: String,
    child: Child,
    stdin: ChildStdin,
    stdout_rx: Receiver<ChannelObservation>,
    stderr_handle: Option<JoinHandle<String>>,
    handled_kinds: HashSet<String>,
    in_flight_intent_ids: BTreeSet<String>,
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
        configure_host_managed_child(&mut child);
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
            handled_kinds: HashSet::new(),
            in_flight_intent_ids: BTreeSet::new(),
        })
    }

    fn wait_ready(
        &mut self,
        timeout: Duration,
        required_kinds: &BTreeSet<String>,
    ) -> Result<(), EgressProcessError> {
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
                    InboundMessage::Ready {
                        protocol,
                        handled_kinds,
                    } => {
                        if protocol != EGRESS_PROTOCOL {
                            self.force_terminate();
                            return Err(EgressProcessError::Protocol {
                                channel: self.channel_id.clone(),
                                detail: format!(
                                    "ready frame protocol mismatch: expected '{}', got '{}'",
                                    EGRESS_PROTOCOL, protocol
                                ),
                            });
                        }

                        let mut seen = HashSet::new();
                        for kind in &handled_kinds {
                            if !seen.insert(kind.clone()) {
                                self.force_terminate();
                                return Err(EgressProcessError::Protocol {
                                    channel: self.channel_id.clone(),
                                    detail: format!(
                                        "ready frame contains duplicate handled_kinds entry '{}'",
                                        kind
                                    ),
                                });
                            }
                        }

                        let handled_kind_set: HashSet<String> = handled_kinds.into_iter().collect();
                        for required_kind in required_kinds {
                            if !handled_kind_set.contains(required_kind) {
                                self.force_terminate();
                                return Err(EgressProcessError::Protocol {
                                    channel: self.channel_id.clone(),
                                    detail: format!(
                                        "ready frame missing required handled kind '{}'",
                                        required_kind
                                    ),
                                });
                            }
                        }
                        self.handled_kinds = handled_kind_set;
                        Ok(())
                    }
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
        if !self.in_flight_intent_ids.insert(intent.intent_id.clone()) {
            return Err(EgressProcessError::Protocol {
                channel: self.channel_id.clone(),
                detail: format!(
                    "intent '{}' is already in-flight on channel",
                    intent.intent_id
                ),
            });
        }

        let outbound = OutboundMessage::Intent {
            intent_id: intent.intent_id.clone(),
            kind: intent.kind.clone(),
            fields: intent_fields_to_json_object(&intent.fields),
        };
        let payload = match serde_json::to_string(&outbound) {
            Ok(payload) => payload,
            Err(err) => {
                self.in_flight_intent_ids.remove(&intent.intent_id);
                return Err(EgressProcessError::Io {
                    channel: self.channel_id.clone(),
                    detail: format!("serialize intent payload: {err}"),
                });
            }
        };

        if let Err(err) = writeln!(self.stdin, "{payload}") {
            self.in_flight_intent_ids.remove(&intent.intent_id);
            return Err(EgressProcessError::Io {
                channel: self.channel_id.clone(),
                detail: format!("write intent payload: {err}"),
            });
        }
        if let Err(err) = self.stdin.flush() {
            self.in_flight_intent_ids.remove(&intent.intent_id);
            return Err(EgressProcessError::Io {
                channel: self.channel_id.clone(),
                detail: format!("flush intent payload: {err}"),
            });
        }

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

        let result = match observation {
            ChannelObservation::Line(line) => {
                let message = serde_json::from_str::<InboundMessage>(line.trim_end_matches('\n'))
                    .map_err(|err| EgressProcessError::Protocol {
                    channel: self.channel_id.clone(),
                    detail: format!("invalid ack frame: {err}"),
                })?;

                match message {
                    InboundMessage::Ready { .. } => Err(EgressProcessError::Protocol {
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
        };

        if result.is_ok() {
            self.in_flight_intent_ids.remove(&intent.intent_id);
        }
        result
    }

    fn shutdown(&mut self, timeout: Duration) -> Result<(), EgressProcessError> {
        let end = OutboundMessage::End;
        if let Ok(payload) = serde_json::to_string(&end) {
            let _ = writeln!(self.stdin, "{payload}");
            let _ = self.stdin.flush();
        }

        let exited =
            wait_for_exit(&mut self.child, timeout).map_err(|err| EgressProcessError::Io {
                channel: self.channel_id.clone(),
                detail: format!("wait for graceful shutdown: {err}"),
            })?;
        if exited.is_none() {
            return Err(EgressProcessError::Io {
                channel: self.channel_id.clone(),
                detail: format!(
                    "channel did not terminate within shutdown timeout ({}ms)",
                    timeout.as_millis()
                ),
            });
        }
        Ok(())
    }

    fn assert_no_pending_acks(
        &mut self,
        host_stop_requested: bool,
    ) -> Result<(), EgressProcessError> {
        if !self.in_flight_intent_ids.is_empty() {
            return Err(EgressProcessError::PendingAcks {
                channel: self.channel_id.clone(),
                detail: format!("in-flight intent IDs: {:?}", self.in_flight_intent_ids),
            });
        }

        // Probe briefly for straggler frames so the invariant catches late acks
        // that arrive right after the last dispatch completes.
        let deadline = std::time::Instant::now() + PENDING_ACK_STRAGGLER_PROBE_WINDOW;
        loop {
            match self.stdout_rx.try_recv() {
                Ok(ChannelObservation::Line(line)) => {
                    return Err(EgressProcessError::PendingAcks {
                        channel: self.channel_id.clone(),
                        detail: format!(
                            "unexpected buffered stdout frame after final dispatch: {}",
                            line.trim_end_matches('\n')
                        ),
                    });
                }
                Ok(ChannelObservation::Eof) => {
                    if host_stop_requested {
                        return Ok(());
                    }
                    return Err(EgressProcessError::PendingAcks {
                        channel: self.channel_id.clone(),
                        detail: "stdout reached EOF with no explicit shutdown".to_string(),
                    });
                }
                Ok(ChannelObservation::ReadError(detail)) => {
                    return Err(EgressProcessError::PendingAcks {
                        channel: self.channel_id.clone(),
                        detail: format!(
                            "stdout reader observed post-dispatch read error: {detail}"
                        ),
                    });
                }
                Err(TryRecvError::Disconnected) => {
                    if host_stop_requested && self.child_has_exited() {
                        return Ok(());
                    }
                    return Err(EgressProcessError::PendingAcks {
                        channel: self.channel_id.clone(),
                        detail: "stdout reader disconnected unexpectedly".to_string(),
                    });
                }
                Err(TryRecvError::Empty) => {
                    if std::time::Instant::now() >= deadline {
                        return Ok(());
                    }
                    std::thread::sleep(PENDING_ACK_STRAGGLER_PROBE_POLL_INTERVAL);
                }
            }
        }
    }

    fn quiesce(&mut self) {
        self.in_flight_intent_ids.clear();
        self.force_terminate();
    }

    fn child_has_exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    fn force_terminate(&mut self) {
        kill_host_managed_child(&mut self.child);
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
            let required_kinds = self.required_route_kinds_for_channel(channel_id);
            handle.wait_ready(DEFAULT_EGRESS_STARTUP_TIMEOUT, &required_kinds)?;
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

        match handle.dispatch_intent(intent, timeout) {
            Ok(ack) => Ok(ack),
            Err(err @ EgressProcessError::Timeout { .. })
            | Err(err @ EgressProcessError::Protocol { .. })
            | Err(err @ EgressProcessError::Io { .. }) => {
                self.quiesce_channel(&route.channel);
                self.quiesce_all_channels();
                Err(err)
            }
            Err(err) => Err(err),
        }
    }

    pub fn assert_no_pending_acks(
        &mut self,
        host_stop_requested: bool,
    ) -> Result<(), EgressProcessError> {
        if !self.started {
            return Ok(());
        }
        for handle in self.channels.values_mut() {
            handle.assert_no_pending_acks(host_stop_requested)?;
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

    pub fn quiesce_all_channels(&mut self) {
        for handle in self.channels.values_mut() {
            handle.quiesce();
        }
        self.channels.clear();
        self.started = false;
    }

    fn quiesce_channel(&mut self, channel_id: &str) {
        if let Some(mut handle) = self.channels.remove(channel_id) {
            handle.quiesce();
        }
        if self.channels.is_empty() {
            self.started = false;
        }
    }

    fn required_route_kinds_for_channel(&self, channel_id: &str) -> BTreeSet<String> {
        self.config
            .routes
            .iter()
            .filter_map(|(intent_kind, route)| {
                (route.channel == channel_id).then_some(intent_kind.clone())
            })
            .collect()
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
            kill_host_managed_child(child);
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
mod tests;
