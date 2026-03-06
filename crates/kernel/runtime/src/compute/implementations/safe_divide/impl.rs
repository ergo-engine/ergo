use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::safe_divide_manifest;

pub struct SafeDivide {
    manifest: ComputePrimitiveManifest,
}

impl SafeDivide {
    pub fn new() -> Self {
        Self {
            manifest: safe_divide_manifest(),
        }
    }
}

impl Default for SafeDivide {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for SafeDivide {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    /// Safe divide with explicit fallback.
    ///
    /// Returns `fallback` when:
    /// - `b == 0.0` (would be division by zero)
    /// - Result is non-finite (overflow)
    ///
    /// This implementation never errors for zero/non-finite conditions.
    /// The author explicitly chooses the fallback value, encoding domain policy.
    ///
    /// **Note:** `fallback` must be a finite number. If `fallback` is NaN or
    /// infinity, safe_divide will return it, and the NUM-FINITE-1 runtime guard
    /// will raise `ExecError::NonFiniteOutput`.
    ///
    /// See: B.2 in PHASE_INVARIANTS.md
    fn compute(
        &self,
        inputs: &HashMap<String, Value>,
        parameters: &HashMap<String, Value>,
        _state: Option<&mut PrimitiveState>,
    ) -> Result<HashMap<String, Value>, ComputeError> {
        let a = inputs
            .get("a")
            .and_then(|v| v.as_number())
            .expect("missing required numeric input 'a'");
        let b = inputs
            .get("b")
            .and_then(|v| v.as_number())
            .expect("missing required numeric input 'b'");
        let fallback = parameters
            .get("fallback")
            .and_then(|v| v.as_number())
            .expect("missing required parameter 'fallback'");

        let result = if b == 0.0 {
            fallback
        } else {
            let r = a / b;
            if r.is_finite() {
                r
            } else {
                fallback
            }
        };

        Ok(HashMap::from([(
            "result".to_string(),
            Value::Number(result),
        )]))
    }
}
