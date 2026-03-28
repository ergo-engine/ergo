//! ergo_loader
//!
//! Purpose:
//! - Expose the loader crate's public transport/config surface for project discovery, graph decode, source loading, and cluster discovery.
//! - Collect the stable top-level re-exports that higher prod layers use without reaching into loader internals.
//!
//! Owns:
//! - The public crate façade and re-export policy for decode, discovery, io, and project surfaces.
//! - The boundary between the smaller top-level convenience API and the module-public advanced
//!   helper surface that remains intentionally supported.
//!
//! Does not own:
//! - Decode, discovery, project, or io behavior themselves; those remain implemented in their respective modules.
//! - Kernel semantic validation, host orchestration, or runtime execution policy.
//!
//! Connects to:
//! - `decode`, `discovery`, `io`, and `project` as the loader's public submodules.
//! - `sdk-rust`, `host`, and loader tests as downstream consumers of the top-level façade.
//!
//! Safety notes:
//! - Loader stays a transport/config boundary: this façade re-exports loader-owned surfaces but does not widen them into semantic authority.
//! - `resolver` remains internal so source-identity and candidate-selection mechanics do not leak into the public contract.

pub mod decode;
pub mod discovery;
mod in_memory;
pub mod io;
pub mod project;
mod resolver;

// Keep advanced helpers under their owning modules (`discovery::...`) instead of flattening every
// supported loader surface into the crate root.
pub use decode::{
    decode_graph_json, decode_graph_yaml, decode_graph_yaml_labeled, parse_graph_file,
    DecodedAuthoringGraph,
};
pub use discovery::{load_cluster_tree, resolve_cluster_candidates};
pub use in_memory::InMemorySourceInput;
pub use io::{
    load_graph_assets_from_memory, load_graph_assets_from_paths, load_graph_sources,
    load_in_memory_graph_sources, FilesystemGraphBundle, InMemoryGraphBundle, LoaderDecodeError,
    LoaderDiscoveryError, LoaderError, LoaderIoError, PreparedGraphAssets,
};
pub use project::{
    discover_project_root, load_project, ProjectError, ProjectIngress, ProjectManifest,
    ProjectProfile, ResolvedProject, ResolvedProjectIngress, ResolvedProjectProfile,
};
