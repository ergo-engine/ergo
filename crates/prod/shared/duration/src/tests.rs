//! ergo_prod_duration::tests
//!
//! Purpose:
//! - Lock the shared prod duration-literal grammar and error wording so host
//!   and loader stay aligned.

use super::parse_duration_literal;
use std::time::Duration;

#[test]
fn parses_supported_suffixes() {
    assert_eq!(
        parse_duration_literal("1500ms").expect("milliseconds should parse"),
        Duration::from_millis(1500)
    );
    assert_eq!(
        parse_duration_literal("5s").expect("seconds should parse"),
        Duration::from_secs(5)
    );
    assert_eq!(
        parse_duration_literal("3m").expect("minutes should parse"),
        Duration::from_secs(180)
    );
    assert_eq!(
        parse_duration_literal("2h").expect("hours should parse"),
        Duration::from_secs(7200)
    );
}

#[test]
fn invalid_numbers_preserve_error_wording() {
    let err = parse_duration_literal("xs").expect_err("invalid literal must fail");
    assert_eq!(err, "invalid duration 'xs': invalid digit found in string");
}

#[test]
fn unsupported_suffixes_preserve_error_wording() {
    let err = parse_duration_literal("5d").expect_err("unsupported suffix must fail");
    assert_eq!(err, "unsupported duration '5d' (expected suffix ms|s|m|h)");
}

#[test]
fn minute_and_hour_expansion_saturates() {
    assert_eq!(
        parse_duration_literal("18446744073709551615m").expect("minutes should saturate"),
        Duration::from_secs(u64::MAX)
    );
    assert_eq!(
        parse_duration_literal("18446744073709551615h").expect("hours should saturate"),
        Duration::from_secs(u64::MAX)
    );
}
