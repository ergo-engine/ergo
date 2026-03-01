use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

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
            .map(|effect| {
                let effect_bytes =
                    serde_json::to_vec(effect).expect("ActionEffect must be serializable");
                let mut hasher = Sha256::new();
                hasher.update(&effect_bytes);
                let effect_hash = hex::encode(hasher.finalize());
                CapturedActionEffect {
                    effect: effect.clone(),
                    effect_hash,
                }
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
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create capture output directory: {err}"))?;
        }
    }

    let mut bytes = match style {
        CaptureJsonStyle::Compact => {
            serde_json::to_vec(bundle).map_err(|err| format!("serialize capture bundle: {err}"))?
        }
        CaptureJsonStyle::Pretty => serde_json::to_vec_pretty(bundle)
            .map_err(|err| format!("serialize capture bundle: {err}"))?,
    };
    bytes.push(b'\n');

    write_bytes_atomic(path, &bytes)
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or_else(|| {
        format!(
            "write capture bundle '{}': destination must include a file name",
            path.display()
        )
    })?;
    let (temp_path, mut file) = create_temp_file(path, parent, file_name)?;

    if let Err(err) = file.write_all(bytes) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "write capture bundle '{}': write temp file '{}': {err}",
            path.display(),
            temp_path.display()
        ));
    }

    if let Err(err) = file.sync_all() {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "write capture bundle '{}': sync temp file '{}': {err}",
            path.display(),
            temp_path.display()
        ));
    }

    drop(file);
    if let Err(err) = replace_destination_with_retry(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "write capture bundle '{}': rename temp file '{}': {err}",
            path.display(),
            temp_path.display()
        ));
    }

    Ok(())
}

fn create_temp_file(
    destination: &Path,
    parent: &Path,
    file_name: &std::ffi::OsStr,
) -> Result<(std::path::PathBuf, std::fs::File), String> {
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
            Err(err) => {
                return Err(format!(
                    "write capture bundle '{}': create temp file '{}': {err}",
                    destination.display(),
                    temp_path.display()
                ))
            }
        }
    }

    Err(format!(
        "write capture bundle '{}': exhausted temp file creation attempts",
        destination.display()
    ))
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
mod tests {
    use super::*;
    use crate::Constraints;
    use ergo_adapter::GraphId;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(label: &str) -> PathBuf {
        let index = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "ergo-supervisor-capture-{label}-{}-{}",
            std::process::id(),
            index
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn sample_bundle() -> CaptureBundle {
        CaptureBundle {
            capture_version: "v2".to_string(),
            graph_id: GraphId::new("capture_test"),
            config: Constraints::default(),
            events: Vec::new(),
            decisions: Vec::new(),
            adapter_provenance: crate::NO_ADAPTER_PROVENANCE.to_string(),
            runtime_provenance: "rpv1:sha256:test".to_string(),
        }
    }

    #[test]
    fn writes_compact_json_with_trailing_newline() {
        let dir = temp_dir("compact");
        let path = dir.join("capture.json");
        write_capture_bundle(&path, &sample_bundle(), CaptureJsonStyle::Compact)
            .expect("compact write should succeed");

        let raw = fs::read_to_string(&path).expect("read capture");
        assert!(raw.ends_with('\n'), "expected trailing newline");
        assert_eq!(
            raw.matches('\n').count(),
            1,
            "compact output should be single-line"
        );
        serde_json::from_str::<CaptureBundle>(&raw).expect("capture json should parse");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_pretty_json_with_trailing_newline() {
        let dir = temp_dir("pretty");
        let path = dir.join("capture.json");
        write_capture_bundle(&path, &sample_bundle(), CaptureJsonStyle::Pretty)
            .expect("pretty write should succeed");

        let raw = fs::read_to_string(&path).expect("read capture");
        assert!(raw.ends_with('\n'), "expected trailing newline");
        assert!(
            raw.matches('\n').count() > 1,
            "pretty output should contain multiple lines"
        );
        serde_json::from_str::<CaptureBundle>(&raw).expect("capture json should parse");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_replace_overwrites_existing_file_cleanly() {
        let dir = temp_dir("replace");
        let path = dir.join("capture.json");
        fs::write(&path, "old-content\n").expect("write original file");

        write_capture_bundle(&path, &sample_bundle(), CaptureJsonStyle::Compact)
            .expect("atomic overwrite should succeed");

        let raw = fs::read_to_string(&path).expect("read capture");
        assert_ne!(raw, "old-content\n", "expected replacement");
        assert!(raw.ends_with('\n'), "expected trailing newline");
        serde_json::from_str::<CaptureBundle>(&raw).expect("capture json should parse");

        let temp_glob = format!("capture.json.{}.*.tmp", std::process::id());
        let leftovers = std::fs::read_dir(&dir)
            .expect("read temp dir")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| {
                let prefix = format!("capture.json.{}.", std::process::id());
                name.starts_with(&prefix) && name.ends_with(".tmp")
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "temp files should not remain after success (pattern: {temp_glob})"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn concurrent_writes_to_same_destination_succeed() {
        let dir = temp_dir("concurrent");
        let path = dir.join("capture.json");
        let mut handles = Vec::new();

        for idx in 0..8 {
            let path = path.clone();
            handles.push(std::thread::spawn(move || {
                let mut bundle = sample_bundle();
                bundle.graph_id = GraphId::new(format!("capture_test_{idx}"));
                write_capture_bundle(&path, &bundle, CaptureJsonStyle::Compact)
            }));
        }

        for handle in handles {
            handle
                .join()
                .expect("thread panicked")
                .expect("writer should succeed");
        }

        let raw = fs::read_to_string(&path).expect("read capture");
        assert!(raw.ends_with('\n'), "expected trailing newline");
        serde_json::from_str::<CaptureBundle>(&raw).expect("capture json should parse");

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn failed_write_leaves_existing_destination_untouched() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_dir("failure");
        let path = dir.join("capture.json");
        fs::write(&path, "old-content\n").expect("write original file");

        let mut perms = fs::metadata(&dir).expect("dir metadata").permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&dir, perms.clone()).expect("set dir readonly");

        let result = write_capture_bundle(&path, &sample_bundle(), CaptureJsonStyle::Compact);
        assert!(result.is_err(), "write should fail in readonly directory");

        let current = fs::read_to_string(&path).expect("read original file");
        assert_eq!(
            current, "old-content\n",
            "destination should remain unchanged"
        );

        perms.set_mode(0o755);
        fs::set_permissions(&dir, perms).expect("restore dir permissions");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn capturing_log_hashes_non_empty_effects_correctly() {
        use ergo_runtime::common::{ActionEffect, EffectWrite, Value};
        use sha2::{Digest, Sha256};

        let effect = ActionEffect {
            kind: "set_context".to_string(),
            writes: vec![EffectWrite {
                key: "price".to_string(),
                value: Value::Number(42.0),
            }],
        };

        let bundle = Arc::new(Mutex::new(CaptureBundle {
            capture_version: "v2".to_string(),
            graph_id: GraphId::new("hash_test"),
            config: Constraints::default(),
            events: Vec::new(),
            decisions: Vec::new(),
            adapter_provenance: crate::NO_ADAPTER_PROVENANCE.to_string(),
            runtime_provenance: "rpv1:sha256:test".to_string(),
        }));

        let inner = crate::replay::MemoryDecisionLog::default();
        let capturing_log = CapturingDecisionLog::new(inner, Arc::clone(&bundle));

        // Construct a DecisionLogEntry with a real effect
        let entry = crate::DecisionLogEntry {
            graph_id: GraphId::new("hash_test"),
            event_id: ergo_adapter::EventId::new("e1"),
            event: ergo_adapter::ExternalEvent::mechanical(
                ergo_adapter::EventId::new("e1"),
                ergo_adapter::ExternalEventKind::Command,
            ),
            decision: crate::Decision::Invoke,
            schedule_at: None,
            episode_id: crate::EpisodeId::new(0),
            deadline: None,
            termination: Some(ergo_adapter::RunTermination::Completed),
            retry_count: 0,
            effects: vec![effect.clone()],
        };

        capturing_log.log(entry);

        let guard = bundle.lock().expect("bundle poisoned");
        assert_eq!(guard.decisions.len(), 1);
        let record = &guard.decisions[0];
        let captured_effects = &record.effects;
        assert_eq!(captured_effects.len(), 1, "one effect expected");
        assert_eq!(captured_effects[0].effect, effect);

        // Verify hash matches serde_json::to_vec -> SHA-256 path
        let expected_bytes = serde_json::to_vec(&effect).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&expected_bytes);
        let expected_hash = hex::encode(hasher.finalize());
        assert_eq!(
            captured_effects[0].effect_hash, expected_hash,
            "effect_hash must equal SHA-256 of serde_json::to_vec(&effect)"
        );
    }
}
