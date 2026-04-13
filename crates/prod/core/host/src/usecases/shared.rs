//! usecases shared
//!
//! Purpose:
//! - Hold the private import/prelude authority for the host `usecases`
//!   submodules and shared internal support types.
//!
//! Owns:
//! - The internal `use` surface consumed by `live_prep`, `live_run`,
//!   `process_driver`, and the `usecases.rs` facade implementation.
//!
//! Does not own:
//! - Public host API types or canonical orchestration entrypoints; those remain
//!   in `usecases.rs`.
//!
//! Connects to:
//! - `usecases.rs` as the parent facade implementation.
//! - `live_prep.rs`, `live_run.rs`, and `process_driver.rs` as the private
//!   execution submodules.
//!
//! Safety notes:
//! - This module exists specifically so the public facade is no longer the
//!   de facto import authority for every child module.
//! - Keep imports here narrowly aligned with real submodule needs; do not turn
//!   this into a second semantic authority.

pub(super) use ergo_adapter::{
    adapter_fingerprint,
    fixture::{self, FixtureParseError},
    validate_action_adapter_composition, validate_capture_format,
    validate_source_adapter_composition, AdapterManifest, AdapterProvides, CompositionError,
    DemoSourceContextError, EventBindingError, EventTime, GraphId, InvalidAdapter, RuntimeHandle,
};
pub(super) use ergo_loader::LoaderError;
pub(super) use ergo_runtime::catalog::{
    build_core_catalog, core_registries, CorePrimitiveCatalog, CoreRegistrationError,
    CoreRegistries,
};
pub(super) use ergo_runtime::cluster::{
    expand, ClusterDefinition, ClusterLoader, ClusterVersionIndex, ExpandError, ExpandedGraph,
    PrimitiveCatalog, PrimitiveKind, Version,
};
pub(super) use ergo_runtime::common::ErrorInfo;
pub(super) use ergo_runtime::provenance::{
    compute_runtime_provenance, RuntimeProvenanceError, RuntimeProvenanceScheme,
};
pub(super) use ergo_supervisor::replay::StrictReplayExpectations;
#[cfg(test)]
pub(super) use ergo_supervisor::Decision;
pub(super) use ergo_supervisor::{
    write_capture_bundle, CaptureBundle, CaptureJsonStyle, CaptureWriteError, Constraints,
    NO_ADAPTER_PROVENANCE,
};
pub(super) use serde::{Deserialize, Serialize};
pub(super) use std::collections::{BTreeSet, HashMap, HashSet};
pub(super) use std::fs;
pub(super) use std::io::{BufRead, BufReader, Read};
pub(super) use std::path::{Path, PathBuf};
pub(super) use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
pub(super) use std::sync::atomic::{AtomicBool, Ordering};
pub(super) use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
pub(super) use std::sync::Arc;
pub(super) use std::thread::{self, JoinHandle};
pub(super) use std::time::{Duration, Instant};

pub(super) use crate::egress::compute_egress_provenance;
pub(super) use crate::{
    decision_counts, replay_bundle_strict, runner::validate_hosted_runner_configuration,
    EgressConfig, EgressDispatchFailure, HostedAdapterConfig, HostedEvent, HostedReplayError,
    HostedRunner, HostedStepError, PROCESS_DRIVER_PROTOCOL_VERSION,
};
