use std::collections::HashMap;

use crate::common::Value;
use crate::compute::{ComputeError, ComputePrimitive, ComputePrimitiveManifest, PrimitiveState};

use super::manifest::divide_manifest;

pub struct Divide {
    manifest: ComputePrimitiveManifest,
}

impl Divide {
    pub fn new() -> Self {
        Self {
            manifest: divide_manifest(),
        }
    }
}

impl Default for Divide {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputePrimitive for Divide {
    fn manifest(&self) -> &ComputePrimitiveManifest {
        &self.manifest
    }

    /// B.2: Strict divide - math-true semantics.
    ///
    /// Returns `Err(DivisionByZero)` when `b == 0.0`.
    /// Returns `Err(NonFiniteResult)` when result overflows to inf.
    ///
    /// For fallback-on-zero behavior, use `safe_divide`.
    ///
    /// See: B.2 in PHASE_INVARIANTS.md
    fn compute(
        &self,
        inputs: &HashMap<String, Value>,
        _parameters: &HashMap<String, Value>,
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

        // B.2: Strict divide - error on zero or non-finite result.
        if b == 0.0 {
            return Err(ComputeError::DivisionByZero);
        }

        let result = a / b;
        if !result.is_finite() {
            return Err(ComputeError::NonFiniteResult);
        }

        Ok(HashMap::from([(
            "result".to_string(),
            Value::Number(result),
        )]))
    }
}
