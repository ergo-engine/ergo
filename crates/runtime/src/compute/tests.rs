use std::collections::HashMap;

use crate::common::Value;
use crate::compute::implementations::{
    Abs, Add, And, ConstBool, ConstNumber, Divide, Eq, Gt, Gte, Lt, Lte, Max, Min, Multiply,
    Negate, Neq, Not, Or, SafeDivide, Select, SelectBool, Subtract,
};
use crate::compute::{ComputeError, ComputePrimitive};

fn expect_panic<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) {
    assert!(std::panic::catch_unwind(f).is_err());
}

#[test]
fn const_number_requires_parameter_and_emits_value() {
    let const_number = ConstNumber::new();
    let outputs = const_number
        .compute(
            &HashMap::new(),
            &HashMap::from([("value".to_string(), Value::Number(2.5))]),
            None,
        )
        .expect("const_number should succeed");
    assert_eq!(outputs.get("value"), Some(&Value::Number(2.5)));

    expect_panic(|| {
        let _ = const_number.compute(&HashMap::new(), &HashMap::new(), None);
    });
}

#[test]
fn const_bool_requires_parameter_and_emits_value() {
    let const_bool = ConstBool::new();
    let outputs = const_bool
        .compute(
            &HashMap::new(),
            &HashMap::from([("value".to_string(), Value::Bool(true))]),
            None,
        )
        .expect("const_bool should succeed");
    assert_eq!(outputs.get("value"), Some(&Value::Bool(true)));

    expect_panic(|| {
        let _ = const_bool.compute(&HashMap::new(), &HashMap::new(), None);
    });
}

#[test]
fn add_requires_inputs_and_computes() {
    let add = Add::new();
    let outputs = add
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(1.0)),
                ("b".to_string(), Value::Number(2.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("add should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Number(3.0)));

    expect_panic(|| {
        let _ = add.compute(
            &HashMap::from([("a".to_string(), Value::Number(1.0))]),
            &HashMap::new(),
            None,
        );
    });
}

#[test]
fn subtract_requires_inputs_and_computes() {
    let subtract = Subtract::new();
    let outputs = subtract
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(5.0)),
                ("b".to_string(), Value::Number(3.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("subtract should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Number(2.0)));

    expect_panic(|| {
        let _ = subtract.compute(
            &HashMap::from([("a".to_string(), Value::Number(1.0))]),
            &HashMap::new(),
            None,
        );
    });
}

#[test]
fn multiply_requires_inputs_and_computes() {
    let multiply = Multiply::new();
    let outputs = multiply
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(2.0)),
                ("b".to_string(), Value::Number(4.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("multiply should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Number(8.0)));

    expect_panic(|| {
        let _ = multiply.compute(
            &HashMap::from([("a".to_string(), Value::Number(1.0))]),
            &HashMap::new(),
            None,
        );
    });
}

#[test]
fn divide_requires_inputs_and_computes() {
    let divide = Divide::new();
    let outputs = divide
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(8.0)),
                ("b".to_string(), Value::Number(2.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("divide should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Number(4.0)));

    expect_panic(|| {
        let _ = divide.compute(
            &HashMap::from([("a".to_string(), Value::Number(1.0))]),
            &HashMap::new(),
            None,
        );
    });
}

#[test]
fn divide_by_zero_errors() {
    let divide = Divide::new();
    let result = divide.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(8.0)),
            ("b".to_string(), Value::Number(0.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert!(matches!(result, Err(ComputeError::DivisionByZero)));
}

#[test]
fn divide_by_negative_zero_errors() {
    let divide = Divide::new();
    let result = divide.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(8.0)),
            ("b".to_string(), Value::Number(-0.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert!(matches!(result, Err(ComputeError::DivisionByZero)));
}

#[test]
fn divide_zero_by_zero_errors() {
    let divide = Divide::new();
    let result = divide.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(0.0)),
            ("b".to_string(), Value::Number(0.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert!(matches!(result, Err(ComputeError::DivisionByZero)));
}

#[test]
fn divide_overflow_errors() {
    let divide = Divide::new();
    let result = divide.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(1e308)),
            ("b".to_string(), Value::Number(1e-308)),
        ]),
        &HashMap::new(),
        None,
    );
    assert!(matches!(result, Err(ComputeError::NonFiniteResult)));
}

#[test]
fn negate_requires_input_and_computes() {
    let negate = Negate::new();
    let outputs = negate
        .compute(
            &HashMap::from([("value".to_string(), Value::Number(3.0))]),
            &HashMap::new(),
            None,
        )
        .expect("negate should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Number(-3.0)));

    expect_panic(|| {
        let _ = negate.compute(&HashMap::new(), &HashMap::new(), None);
    });
}

#[test]
fn abs_positive_is_identity() {
    let abs = Abs::new();
    let outputs = abs
        .compute(
            &HashMap::from([("value".to_string(), Value::Number(3.5))]),
            &HashMap::new(),
            None,
        )
        .expect("abs should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Number(3.5)));
}

#[test]
fn abs_negative_is_flipped() {
    let abs = Abs::new();
    let outputs = abs
        .compute(
            &HashMap::from([("value".to_string(), Value::Number(-3.5))]),
            &HashMap::new(),
            None,
        )
        .expect("abs should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Number(3.5)));
}

#[test]
fn comparisons_require_inputs_and_compute() {
    let gt = Gt::new();
    let gt_out = gt
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(3.0)),
                ("b".to_string(), Value::Number(2.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("gt should succeed");
    assert_eq!(gt_out.get("result"), Some(&Value::Bool(true)));

    expect_panic(|| {
        let _ = gt.compute(
            &HashMap::from([("a".to_string(), Value::Number(3.0))]),
            &HashMap::new(),
            None,
        );
    });

    let lt = Lt::new();
    let lt_out = lt
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(1.0)),
                ("b".to_string(), Value::Number(2.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("lt should succeed");
    assert_eq!(lt_out.get("result"), Some(&Value::Bool(true)));

    expect_panic(|| {
        let _ = lt.compute(
            &HashMap::from([("a".to_string(), Value::Number(1.0))]),
            &HashMap::new(),
            None,
        );
    });

    let eq = Eq::new();
    let eq_out = eq
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(2.0)),
                ("b".to_string(), Value::Number(2.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("eq should succeed");
    assert_eq!(eq_out.get("result"), Some(&Value::Bool(true)));

    expect_panic(|| {
        let _ = eq.compute(
            &HashMap::from([("a".to_string(), Value::Number(2.0))]),
            &HashMap::new(),
            None,
        );
    });

    let neq = Neq::new();
    let neq_out = neq
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(2.0)),
                ("b".to_string(), Value::Number(3.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("neq should succeed");
    assert_eq!(neq_out.get("result"), Some(&Value::Bool(true)));

    expect_panic(|| {
        let _ = neq.compute(
            &HashMap::from([("a".to_string(), Value::Number(2.0))]),
            &HashMap::new(),
            None,
        );
    });
}

#[test]
fn gte_basic_true_when_equal() {
    let gte = Gte::new();
    let outputs = gte
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(2.0)),
                ("b".to_string(), Value::Number(2.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("gte should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Bool(true)));
}

#[test]
fn gte_basic_false_when_less() {
    let gte = Gte::new();
    let outputs = gte
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(1.0)),
                ("b".to_string(), Value::Number(2.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("gte should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Bool(false)));
}

#[test]
fn lte_basic_true_when_equal() {
    let lte = Lte::new();
    let outputs = lte
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(2.0)),
                ("b".to_string(), Value::Number(2.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("lte should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Bool(true)));
}

#[test]
fn lte_basic_false_when_greater() {
    let lte = Lte::new();
    let outputs = lte
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(3.0)),
                ("b".to_string(), Value::Number(2.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("lte should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Bool(false)));
}

#[test]
fn min_selects_lower_value() {
    let min = Min::new();
    let outputs = min
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(2.0)),
                ("b".to_string(), Value::Number(5.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("min should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Number(2.0)));
}

#[test]
fn min_selects_lower_when_swapped() {
    let min = Min::new();
    let outputs = min
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(5.0)),
                ("b".to_string(), Value::Number(2.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("min should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Number(2.0)));
}

#[test]
fn max_selects_higher_value() {
    let max = Max::new();
    let outputs = max
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(2.0)),
                ("b".to_string(), Value::Number(5.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("max should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Number(5.0)));
}

#[test]
fn max_selects_higher_when_swapped() {
    let max = Max::new();
    let outputs = max
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Number(5.0)),
                ("b".to_string(), Value::Number(2.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("max should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Number(5.0)));
}

#[test]
fn boolean_ops_require_inputs_and_compute() {
    let and = And::new();
    let and_out = and
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Bool(true)),
                ("b".to_string(), Value::Bool(false)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("and should succeed");
    assert_eq!(and_out.get("result"), Some(&Value::Bool(false)));

    let or = Or::new();
    let or_out = or
        .compute(
            &HashMap::from([
                ("a".to_string(), Value::Bool(true)),
                ("b".to_string(), Value::Bool(false)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("or should succeed");
    assert_eq!(or_out.get("result"), Some(&Value::Bool(true)));

    let not = Not::new();
    let not_out = not
        .compute(
            &HashMap::from([("value".to_string(), Value::Bool(true))]),
            &HashMap::new(),
            None,
        )
        .expect("not should succeed");
    assert_eq!(not_out.get("result"), Some(&Value::Bool(false)));

    expect_panic(|| {
        let _ = and.compute(
            &HashMap::from([("a".to_string(), Value::Bool(true))]),
            &HashMap::new(),
            None,
        );
    });

    expect_panic(|| {
        let _ = or.compute(
            &HashMap::from([("a".to_string(), Value::Bool(true))]),
            &HashMap::new(),
            None,
        );
    });

    expect_panic(|| {
        let _ = not.compute(&HashMap::new(), &HashMap::new(), None);
    });
}

#[test]
fn select_requires_all_inputs_and_routes_without_casts() {
    let select = Select::new();
    let true_out = select
        .compute(
            &HashMap::from([
                ("cond".to_string(), Value::Bool(true)),
                ("when_true".to_string(), Value::Number(10.0)),
                ("when_false".to_string(), Value::Number(5.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("select should succeed");
    assert_eq!(true_out.get("result"), Some(&Value::Number(10.0)));

    let false_out = select
        .compute(
            &HashMap::from([
                ("cond".to_string(), Value::Bool(false)),
                ("when_true".to_string(), Value::Number(10.0)),
                ("when_false".to_string(), Value::Number(5.0)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("select should succeed");
    assert_eq!(false_out.get("result"), Some(&Value::Number(5.0)));

    expect_panic(|| {
        let _ = select.compute(
            &HashMap::from([
                ("when_true".to_string(), Value::Number(10.0)),
                ("when_false".to_string(), Value::Number(5.0)),
            ]),
            &HashMap::new(),
            None,
        );
    });
}

#[test]
fn select_bool_true_branch_selected() {
    let select_bool = SelectBool::new();
    let outputs = select_bool
        .compute(
            &HashMap::from([
                ("cond".to_string(), Value::Bool(true)),
                ("when_true".to_string(), Value::Bool(true)),
                ("when_false".to_string(), Value::Bool(false)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("select_bool should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Bool(true)));
}

#[test]
fn select_bool_false_branch_selected() {
    let select_bool = SelectBool::new();
    let outputs = select_bool
        .compute(
            &HashMap::from([
                ("cond".to_string(), Value::Bool(false)),
                ("when_true".to_string(), Value::Bool(true)),
                ("when_false".to_string(), Value::Bool(false)),
            ]),
            &HashMap::new(),
            None,
        )
        .expect("select_bool should succeed");
    assert_eq!(outputs.get("result"), Some(&Value::Bool(false)));
}

#[test]
fn safe_divide_by_zero_uses_fallback() {
    let safe_divide = SafeDivide::new();
    let result = safe_divide.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(8.0)),
            ("b".to_string(), Value::Number(0.0)),
        ]),
        &HashMap::from([("fallback".to_string(), Value::Number(42.0))]),
        None,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get("result"), Some(&Value::Number(42.0)));
}

#[test]
fn safe_divide_overflow_uses_fallback() {
    let safe_divide = SafeDivide::new();
    let result = safe_divide.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(1e308)),
            ("b".to_string(), Value::Number(1e-308)),
        ]),
        &HashMap::from([("fallback".to_string(), Value::Number(-1.0))]),
        None,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get("result"), Some(&Value::Number(-1.0)));
}

#[test]
fn safe_divide_normal_succeeds() {
    let safe_divide = SafeDivide::new();
    let result = safe_divide.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(8.0)),
            ("b".to_string(), Value::Number(2.0)),
        ]),
        &HashMap::from([("fallback".to_string(), Value::Number(0.0))]),
        None,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get("result"), Some(&Value::Number(4.0)));
}
