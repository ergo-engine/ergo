//! in_memory
//!
//! Purpose:
//! - Define the loader-owned public transport carrier for in-memory graph authoring input.
//! - Keep the caller-facing in-memory source shape separate from discovery and resolver logic.
//!
//! Owns:
//! - `InMemorySourceInput`, the public input DTO accepted by the loader's in-memory entrypoints.
//!
//! Does not own:
//! - In-memory logical-path validation, source resolution, or cluster-tree discovery.
//! - Public result bundles, prepared assets, or any kernel semantic validation.
//!
//! Connects to:
//! - `io.rs`, `discovery.rs`, and `resolver.rs`, which all consume the same transport input.
//!
//! Safety notes:
//! - `source_id` is the logical lookup identity; `source_label` is diagnostic-only.
//! - Validation of the fields happens in the resolver-owned in-memory validation seam.

#[derive(Debug, Clone)]
pub struct InMemorySourceInput {
    pub source_id: String,
    pub source_label: String,
    pub content: String,
}
