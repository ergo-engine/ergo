//! diagnostics
//!
//! Purpose:
//! - Host-owned warning and diagnostic emission. Provides a writer-based
//!   core so emission logic is testable without capturing stderr.
//!
//! Owns:
//! - The `warning: <message>` output format for egress validation warnings.
//! - The `emit_warnings` writer-based helper and the thin `emit_warnings_to_stderr`
//!   production wrapper.
//!
//! Does not own:
//! - Warning types themselves (owned by `egress::validation`).
//! - Validation logic (owned by `egress::validation` and `runner`).
//!
//! Connects to:
//! - `runner.rs` (`HostedRunner::new()` compatibility path).
//! - `usecases/live_prep.rs` (orchestration path).
//!
//! Safety notes:
//! - The `warning: <format>` output is a public contract consumed by CLI callers
//!   that parse stderr. Changes to the format must be coordinated.

use std::fmt::Display;
use std::io::Write;

/// Emit each warning to `writer` in the standard `warning: <message>` format.
/// Testable: pass any `Write` impl. For production use, prefer
/// `emit_warnings_to_stderr`.
pub(crate) fn emit_warnings<W: Write>(writer: &mut W, warnings: &[impl Display]) {
    for warning in warnings {
        let _ = writeln!(writer, "warning: {warning}");
    }
}

/// Thin production wrapper: emit warnings to stderr.
pub(crate) fn emit_warnings_to_stderr(warnings: &[impl Display]) {
    emit_warnings(&mut std::io::stderr(), warnings);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::egress::EgressValidationWarning;

    #[test]
    fn emit_warnings_formats_each_warning_on_its_own_line() {
        let warnings = vec![
            EgressValidationWarning::RouteForNonEmittableKind {
                intent_kind: "place_order".to_string(),
            },
            EgressValidationWarning::RouteForNonEmittableKind {
                intent_kind: "cancel_order".to_string(),
            },
        ];
        let mut buf = Vec::new();
        emit_warnings(&mut buf, &warnings);
        let output = String::from_utf8(buf).expect("valid utf8");
        assert_eq!(
            output,
            "warning: egress route declared for non-emittable kind 'place_order'\n\
             warning: egress route declared for non-emittable kind 'cancel_order'\n"
        );
    }

    #[test]
    fn emit_warnings_produces_nothing_for_empty_slice() {
        let warnings: Vec<EgressValidationWarning> = vec![];
        let mut buf = Vec::new();
        emit_warnings(&mut buf, &warnings);
        assert!(buf.is_empty());
    }
}
