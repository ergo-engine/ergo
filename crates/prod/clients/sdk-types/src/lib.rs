//! ergo-sdk-types — Shared type definitions for Ergo SDK clients
//!
//! Purpose:
//! - Defines lightweight serializable types shared between the Rust
//!   SDK and other language bindings (e.g., `SdkVersion`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkVersion {
    pub value: String,
}
