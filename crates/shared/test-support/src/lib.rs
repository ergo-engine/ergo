//! ergo-test-support — Shared test utilities
//!
//! Purpose:
//! - Provides small helpers used across workspace test suites
//!   (e.g., deterministic temp directory naming).

pub fn temp_name(seed: &str) -> String {
    format!("ergo-test-{seed}")
}
