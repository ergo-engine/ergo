//! process_driver
//!
//! Purpose:
//! - Host-owned process-ingress v0 runtime seam for canonical live runs.
//! - Launches a child process, enforces the stdout JSONL protocol handshake,
//!   streams `HostedEvent` frames into `HostedRunner::step()`, and shapes
//!   process-ingress completion versus interruption outcomes.
//!
//! Owns:
//! - `ergo-driver.v0` hello ordering, protocol-version acceptance, and outer
//!   `hello` / `event` / `end` message framing for the process-ingress path.
//! - Host operational waiting policy for startup grace, event receive wakeups,
//!   and termination grace.
//! - Host-managed child-process lifecycle for process-group setup, abort, and
//!   post-`end` drain behavior.
//!
//! Does not own:
//! - Canonical run orchestration and final summary shaping in `live_run.rs`.
//! - `HostedEvent` serde semantics in `runner.rs`; nested event-frame shape
//!   changes there affect this wire protocol.
//! - Fixture ingress semantics or any future bidirectional ingress protocol.
//!
//! Connects to:
//! - `live_run.rs`, which selects process ingress under canonical run.
//! - `HostedRunner::step(...)` for host-owned step execution.
//! - The ingress channel guide and host-stop doctrine, which describe the v0
//!   process-ingress protocol and its host-owned lifecycle policy.
//!
//! Safety notes:
//! - `PROCESS_DRIVER_PROTOCOL_VERSION` is the host-local authority for the v0
//!   protocol token; broader repo-wide constant extraction is tracked in issue
//!   #70.
//! - Before the first committed step, protocol/process failures are surfaced as
//!   start/protocol/IO errors; after that boundary they become interrupted runs.
//!   That lifecycle split is currently encoded through `committed_event_count`
//!   rather than a typed phase object; broader structural cleanup is tracked in
//!   issue #69.
//! - Process ingress currently synthesizes a single episode count under `"E1"`
//!   because v0 has no episode boundary frame. That is correct for v0 but
//!   remains a latent correctness seam tracked in issue #69.
//! - Driver start/protocol/I/O failures remain host-authored operational
//!   diagnostics, but they now feed the typed `HostDriverError` surface
//!   instead of collapsing back into the old `HostRunError` string variants.
//! - On Unix, abort kills the host-managed process group configured during
//!   spawn, not just the direct child.

use super::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ProcessDriverMessage {
    Hello { protocol: String },
    Event { event: HostedEvent },
    End,
}

pub(super) const PROCESS_DRIVER_PROTOCOL_VERSION: &str = "ergo-driver.v0";

#[derive(Debug)]
enum ProcessDriverStreamObservation {
    Line(String),
    Eof,
    ReadError(ProcessDriverReadFailure),
}

#[derive(Debug)]
enum ProcessDriverReadFailure {
    InvalidEncoding(String),
    Io(String),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ProcessDriverPolicy {
    pub(super) startup_grace: Duration,
    pub(super) termination_grace: Duration,
    pub(super) poll_interval: Duration,
    pub(super) event_recv_timeout: Duration,
}

pub(super) const DEFAULT_PROCESS_DRIVER_POLICY: ProcessDriverPolicy = ProcessDriverPolicy {
    startup_grace: Duration::from_secs(5),
    termination_grace: Duration::from_secs(5),
    poll_interval: Duration::from_millis(10),
    event_recv_timeout: Duration::from_millis(100),
};

pub(super) fn validate_process_driver_command(
    command: &[String],
) -> Result<(), HostDriverInputError> {
    if command.is_empty() {
        return Err(HostDriverInputError::ProcessCommandEmpty);
    }

    let program = command[0].trim();
    if program.is_empty() {
        return Err(HostDriverInputError::ProcessExecutableBlank);
    }

    if uses_explicit_program_path(program) {
        validate_explicit_process_driver_path(Path::new(program), command)
    } else {
        Ok(())
    }
}

#[derive(Debug)]
enum ProcessDriverReceiveFailure {
    Timeout,
    Disconnected,
}

fn process_driver_host_stop_execution(
    child: &mut Child,
    stderr_handle: &mut Option<JoinHandle<String>>,
    runner: HostedRunner,
    event_count: usize,
    committed_event_count: usize,
    episode_event_counts: Vec<(String, usize)>,
) -> Result<DriverExecution, HostRunError> {
    let _detail = abort_process_child(child, stderr_handle.take());
    host_stop_driver_execution(
        runner,
        event_count,
        committed_event_count,
        episode_event_counts,
    )
}

fn driver_start_error(detail: impl Into<String>) -> HostRunError {
    HostRunError::Driver(HostDriverError::Start(HostDriverStartError::new(detail)))
}

fn driver_start_io_error(detail: impl Into<String>, source: std::io::Error) -> HostRunError {
    HostRunError::Driver(HostDriverError::Start(HostDriverStartError::with_source(
        detail, source,
    )))
}

fn driver_protocol_error(detail: impl Into<String>) -> HostRunError {
    HostRunError::Driver(HostDriverError::Protocol(HostDriverProtocolError::new(
        detail,
    )))
}

fn driver_protocol_json_error(
    detail: impl Into<String>,
    source: serde_json::Error,
) -> HostRunError {
    HostRunError::Driver(HostDriverError::Protocol(
        HostDriverProtocolError::with_json_source(detail, source),
    ))
}

fn driver_io_error(detail: impl Into<String>) -> HostRunError {
    HostRunError::Driver(HostDriverError::Io(HostDriverIoError::new(detail)))
}

fn driver_io_source_error(detail: impl Into<String>, source: std::io::Error) -> HostRunError {
    HostRunError::Driver(HostDriverError::Io(HostDriverIoError::with_source(
        detail, source,
    )))
}

pub(super) fn run_process_driver(
    command: Vec<String>,
    runner: HostedRunner,
    process_policy: ProcessDriverPolicy,
    lifecycle: &RunLifecycleState,
) -> Result<DriverExecution, HostRunError> {
    validate_process_driver_command(&command)
        .map_err(|err| HostRunError::Driver(HostDriverError::Input(err)))?;
    let command_display = format!("{command:?}");
    let mut child = spawn_process_driver(&command)?;
    let mut stderr_handle = child.stderr.take().map(drain_process_stderr);
    let stdout = child.stdout.take().ok_or_else(|| {
        driver_start_error(format!(
            "process driver {command_display} did not expose a stdout protocol stream"
        ))
    })?;
    let stdout_rx = spawn_process_stdout_reader(stdout);
    let mut hello_received = false;
    let mut event_counter = 0usize;
    let mut committed_event_count = 0usize;
    let mut episodes = Vec::new();
    let mut runner = runner;
    let startup_deadline = Instant::now() + process_policy.startup_grace;

    loop {
        if lifecycle.should_stop(committed_event_count) {
            return process_driver_host_stop_execution(
                &mut child,
                &mut stderr_handle,
                runner,
                event_counter,
                committed_event_count,
                episodes,
            );
        }

        if !hello_received && Instant::now() >= startup_deadline {
            let detail = abort_process_child(&mut child, stderr_handle.take());
            let suffix = if detail.is_empty() {
                String::new()
            } else {
                format!(" ({detail})")
            };
            return Err(driver_start_error(format!(
                "process driver {command_display} did not emit a protocol frame within {}ms before startup completed{}",
                process_policy.startup_grace.as_millis(),
                suffix
            )));
        }

        let recv_timeout = if hello_received {
            process_policy.event_recv_timeout
        } else {
            startup_deadline
                .saturating_duration_since(Instant::now())
                .min(process_policy.event_recv_timeout)
        };

        let observation = match recv_process_stream_observation(&stdout_rx, Some(recv_timeout)) {
            Ok(observation) => observation,
            Err(ProcessDriverReceiveFailure::Timeout) => {
                if lifecycle.should_stop(committed_event_count) {
                    return process_driver_host_stop_execution(
                        &mut child,
                        &mut stderr_handle,
                        runner,
                        event_counter,
                        committed_event_count,
                        episodes,
                    );
                }
                continue;
            }
            Err(ProcessDriverReceiveFailure::Disconnected) => {
                if lifecycle.should_stop(committed_event_count) {
                    return process_driver_host_stop_execution(
                        &mut child,
                        &mut stderr_handle,
                        runner,
                        event_counter,
                        committed_event_count,
                        episodes,
                    );
                }
                let detail = abort_process_child(&mut child, stderr_handle.take());
                return Err(if detail.is_empty() {
                    driver_io_error(format!(
                        "receive process driver stdout for {command_display}: stdout reader disconnected unexpectedly"
                    ))
                } else {
                    driver_io_error(format!(
                        "receive process driver stdout for {command_display}: stdout reader disconnected unexpectedly ({detail})"
                    ))
                });
            }
        };

        let message = match observation {
            ProcessDriverStreamObservation::Line(line) => {
                let trimmed = line.trim_end_matches(['\r', '\n']);
                match serde_json::from_str::<ProcessDriverMessage>(trimmed) {
                    Ok(message) => message,
                    Err(err) => {
                        let _detail = abort_process_child(&mut child, stderr_handle.take());
                        if committed_event_count == 0 {
                            return Err(driver_protocol_json_error(
                                format!(
                                    "process driver {command_display} emitted invalid JSONL protocol before first committed step: {err}"
                                ),
                                err,
                            ));
                        }
                        return Ok(DriverExecution {
                            runner,
                            event_count: event_counter,
                            episode_event_counts: episodes,
                            terminal: DriverTerminal::Interrupted(
                                InterruptionReason::ProtocolViolation,
                            ),
                        });
                    }
                }
            }
            ProcessDriverStreamObservation::Eof => {
                if lifecycle.should_stop(committed_event_count) {
                    return process_driver_host_stop_execution(
                        &mut child,
                        &mut stderr_handle,
                        runner,
                        event_counter,
                        committed_event_count,
                        episodes,
                    );
                }
                let exit_status =
                    wait_for_child_exit(&mut child, process_policy).map_err(|err| {
                        let detail = abort_process_child(&mut child, stderr_handle.take());
                        if detail.is_empty() {
                            driver_io_source_error(
                                format!(
                                    "wait on process driver {command_display} after stdout EOF: {err}"
                                ),
                                err,
                            )
                        } else {
                            driver_io_source_error(
                                format!(
                                    "wait on process driver {command_display} after stdout EOF: {err} ({detail})"
                                ),
                                err,
                            )
                        }
                    })?;

                let detail = match exit_status {
                    Some(_) => take_process_stderr(stderr_handle.take()),
                    None => abort_process_child(&mut child, stderr_handle.take()),
                };

                if committed_event_count == 0 {
                    let suffix = if detail.is_empty() {
                        String::new()
                    } else {
                        format!(" ({detail})")
                    };
                    let message = match exit_status {
                        Some(status) => format!(
                            "process driver {command_display} ended before first committed step ({}){}",
                            format_exit_status(status),
                            suffix
                        ),
                        None => format!(
                            "process driver {command_display} closed stdout but did not exit within {}ms before first committed step{}",
                            process_policy.termination_grace.as_millis(),
                            suffix
                        ),
                    };
                    return Err(driver_start_error(message));
                }

                return Ok(DriverExecution {
                    runner,
                    event_count: event_counter,
                    episode_event_counts: episodes,
                    terminal: DriverTerminal::Interrupted(InterruptionReason::DriverTerminated),
                });
            }
            ProcessDriverStreamObservation::ReadError(failure) => {
                if lifecycle.should_stop(committed_event_count) {
                    return process_driver_host_stop_execution(
                        &mut child,
                        &mut stderr_handle,
                        runner,
                        event_counter,
                        committed_event_count,
                        episodes,
                    );
                }
                let extra = abort_process_child(&mut child, stderr_handle.take());
                match failure {
                    ProcessDriverReadFailure::InvalidEncoding(detail) => {
                        let message = if extra.is_empty() {
                            format!(
                                "process driver {command_display} emitted malformed protocol bytes: {detail}"
                            )
                        } else {
                            format!(
                                "process driver {command_display} emitted malformed protocol bytes: {detail} ({extra})"
                            )
                        };
                        if committed_event_count == 0 {
                            return Err(driver_protocol_error(message));
                        }
                        return Ok(DriverExecution {
                            runner,
                            event_count: event_counter,
                            episode_event_counts: episodes,
                            terminal: DriverTerminal::Interrupted(
                                InterruptionReason::ProtocolViolation,
                            ),
                        });
                    }
                    ProcessDriverReadFailure::Io(detail) => {
                        let message = if extra.is_empty() {
                            format!("read process driver stdout for {command_display}: {detail}")
                        } else {
                            format!(
                                "read process driver stdout for {command_display}: {detail} ({extra})"
                            )
                        };
                        if committed_event_count == 0 {
                            return Err(driver_io_error(message));
                        }
                        return Ok(DriverExecution {
                            runner,
                            event_count: event_counter,
                            episode_event_counts: episodes,
                            terminal: DriverTerminal::Interrupted(InterruptionReason::DriverIo),
                        });
                    }
                }
            }
        };

        if !hello_received {
            match message {
                ProcessDriverMessage::Hello { protocol }
                    if protocol == PROCESS_DRIVER_PROTOCOL_VERSION =>
                {
                    hello_received = true;
                    continue;
                }
                ProcessDriverMessage::Hello { protocol } => {
                    let _detail = abort_process_child(&mut child, stderr_handle.take());
                    return Err(driver_protocol_error(format!(
                        "process driver {command_display} declared unsupported protocol '{protocol}'"
                    )));
                }
                other => {
                    let _detail = abort_process_child(&mut child, stderr_handle.take());
                    return Err(driver_protocol_error(format!(
                        "process driver {command_display} must send hello first, got {}",
                        process_message_name(&other)
                    )));
                }
            }
        }

        match message {
            ProcessDriverMessage::Hello { .. } => {
                let _detail = abort_process_child(&mut child, stderr_handle.take());
                if committed_event_count == 0 {
                    return Err(driver_protocol_error(format!(
                        "process driver {command_display} sent duplicate hello before first committed step"
                    )));
                }
                return Ok(DriverExecution {
                    runner,
                    event_count: event_counter,
                    episode_event_counts: episodes,
                    terminal: DriverTerminal::Interrupted(InterruptionReason::ProtocolViolation),
                });
            }
            ProcessDriverMessage::Event { event } => match runner.step(event) {
                Ok(_) => {
                    event_counter += 1;
                    committed_event_count += 1;
                    if episodes.is_empty() {
                        episodes.push(("E1".to_string(), 0));
                    }
                    episodes[0].1 += 1;
                    if lifecycle.should_stop(committed_event_count) {
                        return process_driver_host_stop_execution(
                            &mut child,
                            &mut stderr_handle,
                            runner,
                            event_counter,
                            committed_event_count,
                            episodes,
                        );
                    }
                }
                Err(crate::HostedStepError::EgressDispatchFailure(failure)) => {
                    event_counter += 1;
                    if episodes.is_empty() {
                        episodes.push(("E1".to_string(), 0));
                    }
                    episodes[0].1 += 1;
                    let _detail = abort_process_child(&mut child, stderr_handle.take());
                    return Ok(DriverExecution {
                        runner,
                        event_count: event_counter,
                        episode_event_counts: episodes,
                        terminal: DriverTerminal::Interrupted(
                            interruption_from_egress_dispatch_failure(failure),
                        ),
                    });
                }
                Err(err) => {
                    let _detail = abort_process_child(&mut child, stderr_handle.take());
                    return Err(HostRunError::Step(err));
                }
            },
            ProcessDriverMessage::End => {
                if committed_event_count == 0 {
                    let _detail = abort_process_child(&mut child, stderr_handle.take());
                    return Err(driver_protocol_error(format!(
                        "process driver {command_display} ended before first committed step"
                    )));
                }

                let terminal = drain_process_after_end(
                    &command_display,
                    &mut child,
                    &stdout_rx,
                    &mut stderr_handle,
                    process_policy,
                )
                .map_err(|err| HostRunError::Driver(HostDriverError::Io(err)))?;

                return Ok(DriverExecution {
                    runner,
                    event_count: event_counter,
                    episode_event_counts: episodes,
                    terminal,
                });
            }
        }
    }
}

fn uses_explicit_program_path(program: &str) -> bool {
    let path = Path::new(program);
    path.is_absolute()
        || program.contains(std::path::MAIN_SEPARATOR)
        || (cfg!(windows) && program.contains('/'))
}

fn validate_explicit_process_driver_path(
    path: &Path,
    _command: &[String],
) -> Result<(), HostDriverInputError> {
    let metadata =
        fs::metadata(path).map_err(|source| HostDriverInputError::ProcessPathMetadata {
            path: path.to_path_buf(),
            source,
        })?;
    if !metadata.is_file() {
        return Err(HostDriverInputError::ProcessPathNotFile {
            path: path.to_path_buf(),
        });
    }
    if !metadata_is_executable(&metadata) {
        return Err(HostDriverInputError::ProcessPathNotExecutable {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn metadata_is_executable(metadata: &fs::Metadata) -> bool {
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn metadata_is_executable(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
}

pub(super) fn spawn_process_driver(command: &[String]) -> Result<Child, HostRunError> {
    let mut child = Command::new(&command[0]);
    child.args(&command[1..]);
    child.stdin(Stdio::null());
    child.stdout(Stdio::piped());
    child.stderr(Stdio::piped());
    configure_host_managed_child(&mut child);
    child.spawn().map_err(|err| {
        driver_start_io_error(format!("spawn process driver {:?}: {err}", command), err)
    })
}

fn spawn_process_stdout_reader(stdout: ChildStdout) -> Receiver<ProcessDriverStreamObservation> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = tx.send(ProcessDriverStreamObservation::Eof);
                    break;
                }
                Ok(_) => {
                    if tx.send(ProcessDriverStreamObservation::Line(line)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let failure = if err.kind() == std::io::ErrorKind::InvalidData {
                        ProcessDriverReadFailure::InvalidEncoding(err.to_string())
                    } else {
                        ProcessDriverReadFailure::Io(err.to_string())
                    };
                    let _ = tx.send(ProcessDriverStreamObservation::ReadError(failure));
                    break;
                }
            }
        }
    });
    rx
}

fn drain_process_stderr(stderr: impl Read + Send + 'static) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output);
        output
    })
}

fn recv_process_stream_observation(
    stdout_rx: &Receiver<ProcessDriverStreamObservation>,
    timeout: Option<Duration>,
) -> Result<ProcessDriverStreamObservation, ProcessDriverReceiveFailure> {
    match timeout {
        Some(timeout) => stdout_rx.recv_timeout(timeout).map_err(|err| match err {
            RecvTimeoutError::Timeout => ProcessDriverReceiveFailure::Timeout,
            RecvTimeoutError::Disconnected => ProcessDriverReceiveFailure::Disconnected,
        }),
        None => stdout_rx
            .recv()
            .map_err(|_| ProcessDriverReceiveFailure::Disconnected),
    }
}

fn configure_host_managed_child(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        command.process_group(0);
    }
}

pub(super) fn kill_host_managed_child(child: &mut Child) {
    #[cfg(unix)]
    {
        // SAFETY: `configure_host_managed_child` places the spawned ingress
        // process into its own process group. `Child::id()` returns the OS pid
        // for that still-owned child handle, and `killpg(pid, SIGKILL)` targets
        // that host-managed process group so abort tears down the full ingress
        // subtree instead of only the direct child.
        let _ = unsafe { libc::killpg(child.id() as libc::pid_t, libc::SIGKILL) };
    }

    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
}

fn abort_process_child(child: &mut Child, stderr_handle: Option<JoinHandle<String>>) -> String {
    kill_host_managed_child(child);
    let _ = child.wait();
    take_process_stderr(stderr_handle)
}

fn take_process_stderr(stderr_handle: Option<JoinHandle<String>>) -> String {
    stderr_handle
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn wait_for_child_exit(
    child: &mut Child,
    process_policy: ProcessDriverPolicy,
) -> std::io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + process_policy.termination_grace;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(process_policy.poll_interval);
    }
}

fn drain_process_after_end(
    command_display: &str,
    child: &mut Child,
    stdout_rx: &Receiver<ProcessDriverStreamObservation>,
    stderr_handle: &mut Option<JoinHandle<String>>,
    process_policy: ProcessDriverPolicy,
) -> Result<DriverTerminal, HostDriverIoError> {
    let deadline = Instant::now() + process_policy.termination_grace;
    let mut stdout_eof = false;
    let mut exit_status: Option<ExitStatus> = None;

    loop {
        if exit_status.is_none() {
            match child.try_wait() {
                Ok(status) => exit_status = status,
                Err(err) => {
                    let detail = abort_process_child(child, stderr_handle.take());
                    return if detail.is_empty() {
                        Err(HostDriverIoError::with_source(
                            format!("wait on process driver {command_display}: {err}"),
                            err,
                        ))
                    } else {
                        Err(HostDriverIoError::with_source(
                            format!("wait on process driver {command_display}: {err} ({detail})"),
                            err,
                        ))
                    };
                }
            }
        }

        if stdout_eof {
            if let Some(status) = exit_status {
                let _detail = take_process_stderr(stderr_handle.take());
                return Ok(if status.success() {
                    DriverTerminal::Completed
                } else {
                    DriverTerminal::Interrupted(InterruptionReason::DriverTerminated)
                });
            }

            if Instant::now() >= deadline {
                let _detail = abort_process_child(child, stderr_handle.take());
                return Ok(DriverTerminal::Interrupted(
                    InterruptionReason::DriverTerminated,
                ));
            }

            thread::sleep(process_policy.poll_interval);
            continue;
        }

        let now = Instant::now();
        if now >= deadline {
            let _detail = abort_process_child(child, stderr_handle.take());
            return Ok(DriverTerminal::Interrupted(
                InterruptionReason::DriverTerminated,
            ));
        }

        let timeout = (deadline - now).min(process_policy.poll_interval);
        match stdout_rx.recv_timeout(timeout) {
            Ok(ProcessDriverStreamObservation::Line(_)) => {
                let _detail = abort_process_child(child, stderr_handle.take());
                return Ok(DriverTerminal::Interrupted(
                    InterruptionReason::ProtocolViolation,
                ));
            }
            Ok(ProcessDriverStreamObservation::Eof) => {
                stdout_eof = true;
            }
            Ok(ProcessDriverStreamObservation::ReadError(failure)) => match failure {
                ProcessDriverReadFailure::InvalidEncoding(_detail) => {
                    let _extra = abort_process_child(child, stderr_handle.take());
                    return Ok(DriverTerminal::Interrupted(
                        InterruptionReason::ProtocolViolation,
                    ));
                }
                ProcessDriverReadFailure::Io(detail) => {
                    let extra = abort_process_child(child, stderr_handle.take());
                    return if extra.is_empty() {
                        Err(HostDriverIoError::new(format!(
                            "read process driver stdout for {command_display}: {detail}"
                        )))
                    } else {
                        Err(HostDriverIoError::new(format!(
                            "read process driver stdout for {command_display}: {detail} ({extra})"
                        )))
                    };
                }
            },
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                let extra = abort_process_child(child, stderr_handle.take());
                return if extra.is_empty() {
                    Err(HostDriverIoError::new(format!(
                        "stdout reader disconnected unexpectedly for process driver {command_display}"
                    )))
                } else {
                    Err(HostDriverIoError::new(format!(
                        "stdout reader disconnected unexpectedly for process driver {command_display} ({extra})"
                    )))
                };
            }
        }
    }
}

fn format_exit_status(status: ExitStatus) -> String {
    status.to_string()
}

fn process_message_name(message: &ProcessDriverMessage) -> &'static str {
    match message {
        ProcessDriverMessage::Hello { .. } => "hello",
        ProcessDriverMessage::Event { .. } => "event",
        ProcessDriverMessage::End => "end",
    }
}

#[cfg(test)]
mod tests;
