//! egress
//!
//! Purpose:
//! - Re-export hub for the host egress subsystem.
//!
//! Owns:
//! - The public surface gating for the `config`, `process`, and `validation`
//!   submodules.
//!
//! Does not own:
//! - Egress behavior, protocol semantics, or config validation rules; those
//!   remain owned by the re-exported submodules.
//!
//! Connects to:
//! - `runner.rs`, which consumes `EgressRuntime` and `EgressConfig`.
//! - `usecases.rs`, which consumes `compute_egress_provenance`.
//! - `lib.rs`, which re-exports this full public egress surface.

mod config;
mod process;
mod validation;

pub use config::{
    compute_egress_provenance, parse_egress_config_toml, EgressChannelConfig, EgressConfig,
    EgressRoute,
};
pub use process::{EgressProcessError, EgressRuntime};
pub use validation::{validate_egress_config, EgressValidationError, EgressValidationWarning};
