//! protocol
//!
//! Purpose:
//! - Define narrow host-owned protocol constants that downstream Rust callers
//!   need to share with canonical host behavior.
//!
//! Owns:
//! - The canonical process-ingress protocol version token accepted by the host
//!   `DriverConfig::Process` runtime seam.
//!
//! Does not own:
//! - Process-driver loop behavior or lifecycle enforcement in
//!   `usecases/process_driver.rs`.
//! - Egress-channel protocol constants, which remain owned by the egress
//!   runtime seam.
//!
//! Connects to:
//! - `ergo_host` public re-exports consumed by CLI, SDK, and host tests.
//! - Authoring/docs examples that describe the current live process-ingress
//!   protocol token.
//!
//! Safety notes:
//! - Changing `PROCESS_DRIVER_PROTOCOL_VERSION` is a wire-compatibility
//!   decision for the public process-ingress protocol, not a local refactor.

pub const PROCESS_DRIVER_PROTOCOL_VERSION: &str = "ergo-driver.v0";
