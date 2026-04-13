//! capture
//!
//! Purpose:
//! - Define the kernel-owned capture bundle/session types plus the typed capture
//!   artifact write boundary.
//!
//! Owns:
//! - `CapturingSession` and `CapturingDecisionLog` for bundle accumulation.
//! - `CaptureWriteError` and atomic capture-artifact write policy.
//!
//! Does not own:
//! - Host capture orchestration or product-facing write-error rendering.
//! - Replay validation semantics over completed capture bundles.
//!
//! Connects to:
//! - Host and SDK capture write paths through `write_capture_bundle`.
//! - Supervisor demo/fixture helpers that persist capture artifacts.
//!
//! Safety notes:
//! - Artifact writes remain atomic through temp-file write + sync + rename.
//! - `CaptureWriteError` preserves the exact write stage and chained source so
//!   higher layers can stop flattening capture write failures into strings.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ergo_adapter::capture::ExternalEventRecord;
use ergo_adapter::{ExternalEvent, GraphId, RuntimeInvoker};

use crate::{
    CaptureBundle, CapturedActionEffect, Constraints, DecisionLog, DecisionLogEntry,
    EpisodeInvocationRecord, Supervisor,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureJsonStyle {
    Compact,
    Pretty,
}

#[derive(Debug)]
pub enum CaptureWriteError {
    CreateOutputDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    Serialize {
        path: PathBuf,
        style: CaptureJsonStyle,
        source: serde_json::Error,
    },
    InvalidDestination {
        path: PathBuf,
    },
    CreateTempFile {
        destination: PathBuf,
        temp_path: PathBuf,
        source: std::io::Error,
    },
    ExhaustedTempFileAttempts {
        destination: PathBuf,
    },
    WriteTempFile {
        destination: PathBuf,
        temp_path: PathBuf,
        source: std::io::Error,
    },
    SyncTempFile {
        destination: PathBuf,
        temp_path: PathBuf,
        source: std::io::Error,
    },
    RenameTempFile {
        destination: PathBuf,
        temp_path: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for CaptureWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateOutputDirectory { source, .. } => {
                write!(f, "create capture output directory: {source}")
            }
            Self::Serialize {
                path,
                style,
                source,
            } => write!(
                f,
                "serialize capture bundle '{}' ({}): {source}",
                path.display(),
                match style {
                    CaptureJsonStyle::Compact => "compact",
                    CaptureJsonStyle::Pretty => "pretty",
                }
            ),
            Self::InvalidDestination { path } => write!(
                f,
                "write capture bundle '{}': destination must include a file name",
                path.display()
            ),
            Self::CreateTempFile {
                destination,
                temp_path,
                source,
            } => write!(
                f,
                "write capture bundle '{}': create temp file '{}': {source}",
                destination.display(),
                temp_path.display()
            ),
            Self::ExhaustedTempFileAttempts { destination } => write!(
                f,
                "write capture bundle '{}': exhausted temp file creation attempts",
                destination.display()
            ),
            Self::WriteTempFile {
                destination,
                temp_path,
                source,
            } => write!(
                f,
                "write capture bundle '{}': write temp file '{}': {source}",
                destination.display(),
                temp_path.display()
            ),
            Self::SyncTempFile {
                destination,
                temp_path,
                source,
            } => write!(
                f,
                "write capture bundle '{}': sync temp file '{}': {source}",
                destination.display(),
                temp_path.display()
            ),
            Self::RenameTempFile {
                destination,
                temp_path,
                source,
            } => write!(
                f,
                "write capture bundle '{}': rename temp file '{}': {source}",
                destination.display(),
                temp_path.display()
            ),
        }
    }
}

impl std::error::Error for CaptureWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateOutputDirectory { source, .. } => Some(source),
            Self::Serialize { source, .. } => Some(source),
            Self::CreateTempFile { source, .. } => Some(source),
            Self::WriteTempFile { source, .. } => Some(source),
            Self::SyncTempFile { source, .. } => Some(source),
            Self::RenameTempFile { source, .. } => Some(source),
            Self::InvalidDestination { .. } | Self::ExhaustedTempFileAttempts { .. } => None,
        }
    }
}

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);
const MAX_TEMP_FILE_ATTEMPTS: u32 = 64;
#[cfg(windows)]
const MAX_REPLACE_RETRY_ATTEMPTS: u32 = 64;
#[cfg(windows)]
const REPLACE_RETRY_DELAY_MS: u64 = 10;

pub struct CapturingDecisionLog<L: DecisionLog> {
    inner: L,
    bundle: Arc<Mutex<CaptureBundle>>,
}

impl<L: DecisionLog> CapturingDecisionLog<L> {
    pub fn new(inner: L, bundle: Arc<Mutex<CaptureBundle>>) -> Self {
        Self { inner, bundle }
    }
}

impl<L: DecisionLog> DecisionLog for CapturingDecisionLog<L> {
    fn log(&self, entry: DecisionLogEntry) {
        self.inner.log(entry.clone());

        let captured_effects: Vec<CapturedActionEffect> = entry
            .effects
            .iter()
            .map(|effect| CapturedActionEffect {
                effect_hash: crate::compute_effect_hash(effect),
                effect: effect.clone(),
            })
            .collect();

        let mut record = EpisodeInvocationRecord::from(&entry);
        record.effects = captured_effects;

        let mut guard = self.bundle.lock().expect("capture bundle poisoned");
        guard.decisions.push(record);
    }
}

pub struct CapturingSession<L: DecisionLog, R: RuntimeInvoker> {
    supervisor: Supervisor<CapturingDecisionLog<L>, R>,
    bundle: Arc<Mutex<CaptureBundle>>,
}

impl<L: DecisionLog, R: RuntimeInvoker> CapturingSession<L, R> {
    pub fn new(
        graph_id: GraphId,
        constraints: Constraints,
        inner_log: L,
        runtime: R,
        runtime_provenance: String,
    ) -> Self {
        Self::new_with_provenance(
            graph_id,
            constraints,
            inner_log,
            runtime,
            crate::NO_ADAPTER_PROVENANCE.to_string(),
            runtime_provenance,
        )
    }

    pub fn new_with_provenance(
        graph_id: GraphId,
        constraints: Constraints,
        inner_log: L,
        runtime: R,
        adapter_provenance: String,
        runtime_provenance: String,
    ) -> Self {
        let bundle = Arc::new(Mutex::new(CaptureBundle {
            capture_version: crate::CAPTURE_FORMAT_VERSION.to_string(),
            graph_id: graph_id.clone(),
            config: constraints.clone(),
            events: Vec::new(),
            decisions: Vec::new(),
            adapter_provenance,
            runtime_provenance,
            egress_provenance: None,
        }));

        let capturing_log = CapturingDecisionLog::new(inner_log, Arc::clone(&bundle));
        let supervisor = Supervisor::with_runtime(graph_id, constraints, capturing_log, runtime);

        Self { supervisor, bundle }
    }

    pub fn on_event(&mut self, event: ExternalEvent) {
        {
            let mut guard = self.bundle.lock().expect("capture bundle poisoned");
            guard.events.push(ExternalEventRecord::from_event(&event));
        }

        self.supervisor.on_event(event);
    }

    pub fn into_bundle(self) -> CaptureBundle {
        let CapturingSession { supervisor, bundle } = self;
        drop(supervisor);

        match Arc::try_unwrap(bundle) {
            Ok(mutex) => mutex.into_inner().expect("capture bundle poisoned"),
            Err(shared) => shared.lock().expect("capture bundle poisoned").clone(),
        }
    }
}

pub fn write_capture_bundle(
    path: &Path,
    bundle: &CaptureBundle,
    style: CaptureJsonStyle,
) -> Result<(), CaptureWriteError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| {
                CaptureWriteError::CreateOutputDirectory {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }
    }

    let mut bytes = match style {
        CaptureJsonStyle::Compact => {
            serde_json::to_vec(bundle).map_err(|source| CaptureWriteError::Serialize {
                path: path.to_path_buf(),
                style,
                source,
            })?
        }
        CaptureJsonStyle::Pretty => {
            serde_json::to_vec_pretty(bundle).map_err(|source| CaptureWriteError::Serialize {
                path: path.to_path_buf(),
                style,
                source,
            })?
        }
    };
    bytes.push(b'\n');

    write_bytes_atomic(path, &bytes)
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), CaptureWriteError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| CaptureWriteError::InvalidDestination {
            path: path.to_path_buf(),
        })?;
    let (temp_path, mut file) = create_temp_file(path, parent, file_name)?;

    if let Err(source) = file.write_all(bytes) {
        let _ = fs::remove_file(&temp_path);
        return Err(CaptureWriteError::WriteTempFile {
            destination: path.to_path_buf(),
            temp_path,
            source,
        });
    }

    if let Err(source) = file.sync_all() {
        let _ = fs::remove_file(&temp_path);
        return Err(CaptureWriteError::SyncTempFile {
            destination: path.to_path_buf(),
            temp_path,
            source,
        });
    }

    drop(file);
    if let Err(source) = replace_destination_with_retry(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(CaptureWriteError::RenameTempFile {
            destination: path.to_path_buf(),
            temp_path,
            source,
        });
    }

    Ok(())
}

fn create_temp_file(
    destination: &Path,
    parent: &Path,
    file_name: &std::ffi::OsStr,
) -> Result<(std::path::PathBuf, std::fs::File), CaptureWriteError> {
    for _ in 0..MAX_TEMP_FILE_ATTEMPTS {
        let suffix = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_name = format!(
            "{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            suffix
        );
        let temp_path = parent.join(temp_name);
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(CaptureWriteError::CreateTempFile {
                    destination: destination.to_path_buf(),
                    temp_path,
                    source,
                });
            }
        }
    }

    Err(CaptureWriteError::ExhaustedTempFileAttempts {
        destination: destination.to_path_buf(),
    })
}

#[cfg(not(windows))]
fn replace_destination_with_retry(temp_path: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(temp_path, destination)
}

#[cfg(windows)]
fn replace_destination_with_retry(temp_path: &Path, destination: &Path) -> std::io::Result<()> {
    use std::time::Duration;

    let mut last_permission_error = None;
    for attempt in 0..MAX_REPLACE_RETRY_ATTEMPTS {
        match replace_destination_once(temp_path, destination) {
            Ok(()) => return Ok(()),
            Err(err)
                if err.kind() == ErrorKind::PermissionDenied
                    && attempt + 1 < MAX_REPLACE_RETRY_ATTEMPTS =>
            {
                last_permission_error = Some(err);
                // Windows can transiently deny atomic replace when the destination is contended.
                std::thread::sleep(Duration::from_millis(REPLACE_RETRY_DELAY_MS));
            }
            Err(err) => return Err(err),
        }
    }

    Err(last_permission_error.unwrap_or_else(|| {
        std::io::Error::new(
            ErrorKind::PermissionDenied,
            "atomic replace failed after retry attempts",
        )
    }))
}

#[cfg(windows)]
fn replace_destination_once(temp_path: &Path, destination: &Path) -> std::io::Result<()> {
    use std::iter;
    use std::os::windows::ffi::OsStrExt;
    type Dword = u32;
    type WinBool = i32;

    const MOVEFILE_REPLACE_EXISTING: Dword = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: Dword = 0x0000_0008;

    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: Dword,
        ) -> WinBool;
    }

    let temp_wide: Vec<u16> = temp_path
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();

    // SAFETY: pointers are valid for the duration of the call and NUL terminated.
    let ok = unsafe {
        MoveFileExW(
            temp_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
