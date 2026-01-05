use std::collections::HashMap;

use crate::common::Value;
use crate::compute::implementations::{
    Abs, Add, And, ConstBool, ConstNumber, Divide, Eq, Gt, Gte, Lt, Lte, Min, Multiply, Negate,
    Neq, Not, Or, Select, Subtract,
};
use crate::compute::ComputePrimitive;

fn expect_panic<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) {
    assert!(std::panic::catch_unwind(f).is_err());
}

#[test]
fn const_number_requires_parameter_and_emits_value() {
    let const_number = ConstNumber::new();
    let outputs = const_number.compute(
        &HashMap::new(),
        &HashMap::from([("value".to_string(), Value::Number(2.5))]),
        None,
    );
    assert_eq!(outputs.get("value"), Some(&Value::Number(2.5)));

    expect_panic(|| {
        const_number.compute(&HashMap::new(), &HashMap::new(), None);
    });
}

#[test]
fn const_bool_requires_parameter_and_emits_value() {
    let const_bool = ConstBool::new();
    let outputs = const_bool.compute(
        &HashMap::new(),
        &HashMap::from([("value".to_string(), Value::Bool(true))]),
        None,
    );
    assert_eq!(outputs.get("value"), Some(&Value::Bool(true)));

    expect_panic(|| {
        const_bool.compute(&HashMap::new(), &HashMap::new(), None);
    });
}

#[test]
fn add_requires_inputs_and_computes() {
    let add = Add::new();
    let outputs = add.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(1.0)),
            ("b".to_string(), Value::Number(2.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Number(3.0)));

    expect_panic(|| {
        add.compute(
            &HashMap::from([("a".to_string(), Value::Number(1.0))]),
            &HashMap::new(),
            None,
        );
    });
}

#[test]
fn subtract_requires_inputs_and_computes() {
    let subtract = Subtract::new();
    let outputs = subtract.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(5.0)),
            ("b".to_string(), Value::Number(3.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Number(2.0)));

    expect_panic(|| {
        subtract.compute(
            &HashMap::from([("a".to_string(), Value::Number(1.0))]),
            &HashMap::new(),
            None,
        );
    });
}

#[test]
fn multiply_requires_inputs_and_computes() {
    let multiply = Multiply::new();
    let outputs = multiply.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(2.0)),
            ("b".to_string(), Value::Number(4.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Number(8.0)));

    expect_panic(|| {
        multiply.compute(
            &HashMap::from([("a".to_string(), Value::Number(1.0))]),
            &HashMap::new(),
            None,
        );
    });
}

#[test]
fn divide_requires_inputs_and_computes() {
    let divide = Divide::new();
    let outputs = divide.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(8.0)),
            ("b".to_string(), Value::Number(2.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Number(4.0)));

    expect_panic(|| {
        divide.compute(
            &HashMap::from([("a".to_string(), Value::Number(1.0))]),
            &HashMap::new(),
            None,
        );
    });
}

#[test]
fn negate_requires_input_and_computes() {
    let negate = Negate::new();
    let outputs = negate.compute(
        &HashMap::from([("value".to_string(), Value::Number(3.0))]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Number(-3.0)));

    expect_panic(|| {
        negate.compute(&HashMap::new(), &HashMap::new(), None);
    });
}

#[test]
fn abs_positive_is_identity() {
    let abs = Abs::new();
    let outputs = abs.compute(
        &HashMap::from([("value".to_string(), Value::Number(3.5))]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Number(3.5)));
}

#[test]
fn abs_negative_is_flipped() {
    let abs = Abs::new();
    let outputs = abs.compute(
        &HashMap::from([("value".to_string(), Value::Number(-3.5))]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Number(3.5)));
}

#[test]
fn comparisons_require_inputs_and_compute() {
    let gt = Gt::new();
    let gt_out = gt.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(3.0)),
            ("b".to_string(), Value::Number(2.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(gt_out.get("result"), Some(&Value::Bool(true)));

    expect_panic(|| {
        gt.compute(
            &HashMap::from([("a".to_string(), Value::Number(3.0))]),
            &HashMap::new(),
            None,
        );
    });

    let lt = Lt::new();
    let lt_out = lt.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(1.0)),
            ("b".to_string(), Value::Number(2.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(lt_out.get("result"), Some(&Value::Bool(true)));

    expect_panic(|| {
        lt.compute(
            &HashMap::from([("a".to_string(), Value::Number(1.0))]),
            &HashMap::new(),
            None,
        );
    });

    let eq = Eq::new();
    let eq_out = eq.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(2.0)),
            ("b".to_string(), Value::Number(2.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(eq_out.get("result"), Some(&Value::Bool(true)));

    expect_panic(|| {
        eq.compute(
            &HashMap::from([("a".to_string(), Value::Number(2.0))]),
            &HashMap::new(),
            None,
        );
    });

    let neq = Neq::new();
    let neq_out = neq.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(2.0)),
            ("b".to_string(), Value::Number(3.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(neq_out.get("result"), Some(&Value::Bool(true)));

    expect_panic(|| {
        neq.compute(
            &HashMap::from([("a".to_string(), Value::Number(2.0))]),
            &HashMap::new(),
            None,
        );
    });
}

#[test]
fn gte_basic_true_when_equal() {
    let gte = Gte::new();
    let outputs = gte.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(2.0)),
            ("b".to_string(), Value::Number(2.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Bool(true)));
}

#[test]
fn gte_basic_false_when_less() {
    let gte = Gte::new();
    let outputs = gte.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(1.0)),
            ("b".to_string(), Value::Number(2.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Bool(false)));
}

#[test]
fn lte_basic_true_when_equal() {
    let lte = Lte::new();
    let outputs = lte.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(2.0)),
            ("b".to_string(), Value::Number(2.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Bool(true)));
}

#[test]
fn lte_basic_false_when_greater() {
    let lte = Lte::new();
    let outputs = lte.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(3.0)),
            ("b".to_string(), Value::Number(2.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Bool(false)));
}

#[test]
fn min_selects_lower_value() {
    let min = Min::new();
    let outputs = min.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(2.0)),
            ("b".to_string(), Value::Number(5.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Number(2.0)));
}

#[test]
fn min_selects_lower_when_swapped() {
    let min = Min::new();
    let outputs = min.compute(
        &HashMap::from([
            ("a".to_string(), Value::Number(5.0)),
            ("b".to_string(), Value::Number(2.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(outputs.get("result"), Some(&Value::Number(2.0)));
}

#[test]
fn boolean_ops_require_inputs_and_compute() {
    let and = And::new();
    let and_out = and.compute(
        &HashMap::from([
            ("a".to_string(), Value::Bool(true)),
            ("b".to_string(), Value::Bool(false)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(and_out.get("result"), Some(&Value::Bool(false)));

    let or = Or::new();
    let or_out = or.compute(
        &HashMap::from([
            ("a".to_string(), Value::Bool(true)),
            ("b".to_string(), Value::Bool(false)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(or_out.get("result"), Some(&Value::Bool(true)));

    let not = Not::new();
    let not_out = not.compute(
        &HashMap::from([("value".to_string(), Value::Bool(true))]),
        &HashMap::new(),
        None,
    );
    assert_eq!(not_out.get("result"), Some(&Value::Bool(false)));

    expect_panic(|| {
        and.compute(
            &HashMap::from([("a".to_string(), Value::Bool(true))]),
            &HashMap::new(),
            None,
        );
    });

    expect_panic(|| {
        or.compute(
            &HashMap::from([("a".to_string(), Value::Bool(true))]),
            &HashMap::new(),
            None,
        );
    });

    expect_panic(|| {
        not.compute(&HashMap::new(), &HashMap::new(), None);
    });
}

#[test]
fn select_requires_all_inputs_and_routes_without_casts() {
    let select = Select::new();
    let true_out = select.compute(
        &HashMap::from([
            ("cond".to_string(), Value::Bool(true)),
            ("when_true".to_string(), Value::Number(10.0)),
            ("when_false".to_string(), Value::Number(5.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(true_out.get("result"), Some(&Value::Number(10.0)));

    let false_out = select.compute(
        &HashMap::from([
            ("cond".to_string(), Value::Bool(false)),
            ("when_true".to_string(), Value::Number(10.0)),
            ("when_false".to_string(), Value::Number(5.0)),
        ]),
        &HashMap::new(),
        None,
    );
    assert_eq!(false_out.get("result"), Some(&Value::Number(5.0)));

    expect_panic(|| {
        select.compute(
            &HashMap::from([
                ("when_true".to_string(), Value::Number(10.0)),
                ("when_false".to_string(), Value::Number(5.0)),
            ]),
            &HashMap::new(),
            None,
        );
    });
}
