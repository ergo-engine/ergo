use std::collections::HashMap;

use crate::action::{
    AckAction, ActionOutcome, ActionPrimitive, ActionValue, AnnotateAction, ParameterValue,
};

fn expect_panic<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) {
    assert!(std::panic::catch_unwind(f).is_err());
}

#[test]
fn ack_action_respects_accept_parameter() {
    let action = AckAction::new();
    let accepted = action.execute(
        &HashMap::from([(
            "event".to_string(),
            ActionValue::Event(ActionOutcome::Attempted),
        )]),
        &HashMap::from([("accept".to_string(), ParameterValue::Bool(true))]),
    );
    assert_eq!(
        accepted.get("outcome"),
        Some(&ActionValue::Event(ActionOutcome::Completed))
    );

    let rejected = action.execute(
        &HashMap::from([(
            "event".to_string(),
            ActionValue::Event(ActionOutcome::Attempted),
        )]),
        &HashMap::from([("accept".to_string(), ParameterValue::Bool(false))]),
    );
    assert_eq!(
        rejected.get("outcome"),
        Some(&ActionValue::Event(ActionOutcome::Rejected))
    );
}

#[test]
fn annotate_action_emits_attempted() {
    let action = AnnotateAction::new();
    let outputs = action.execute(
        &HashMap::from([(
            "event".to_string(),
            ActionValue::Event(ActionOutcome::Attempted),
        )]),
        &HashMap::from([(
            "note".to_string(),
            ParameterValue::String("hello".to_string()),
        )]),
    );
    assert_eq!(
        outputs.get("outcome"),
        Some(&ActionValue::Event(ActionOutcome::Attempted))
    );
}

#[test]
fn actions_require_event_input() {
    let action = AckAction::new();
    expect_panic(|| {
        action.execute(&HashMap::new(), &HashMap::new());
    });
}
