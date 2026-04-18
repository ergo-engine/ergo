//! usecases shared
//!
//! Purpose:
//! - Shared external-crate and standard-library import prelude for the host
//!   `usecases` submodules (`live_prep`, `live_run`, and the `usecases.rs`
//!   facade).
//!
//! Owns:
//! - The `pub(super)` re-export surface consumed via `use super::shared::*`
//!   in sibling modules. Items here are external-crate types, std types, and
//!   crate-internal re-exports that multiple siblings need.
//!
//! Does not own:
//! - Public host API types (owned by `usecases.rs`).
//! - Sibling-module types (imported explicitly between siblings).
//! - Process-driver-only types (`process_driver.rs` imports directly).
//!
//! Safety notes:
//! - `process_driver.rs` imports all its dependencies explicitly and does NOT
//!   use this prelude. Only `live_prep.rs` and `live_run.rs` consume it.
//! - Keep imports here narrowly aligned with real multi-consumer needs; do not
//!   add items used by only one sibling.

// --- External crate types ---
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

// --- Standard library ---
pub(super) use std::collections::{BTreeSet, HashMap, HashSet};
pub(super) use std::fs;
pub(super) use std::path::{Path, PathBuf};
pub(super) use std::sync::atomic::{AtomicBool, Ordering};
pub(super) use std::sync::Arc;
pub(super) use std::time::{Duration, Instant};

// --- Crate-internal re-exports ---
pub(super) use crate::egress::compute_egress_provenance;
pub(super) use crate::{
    decision_counts, replay_bundle_strict, runner::validate_hosted_runner_configuration,
    EgressConfig, EgressDispatchFailure, HostedAdapterConfig, HostedEvent, HostedReplayError,
    HostedRunner, HostedStepError,
};
