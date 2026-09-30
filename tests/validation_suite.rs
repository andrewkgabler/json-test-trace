// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Known pass/fail/panic test suite for summary validation.

#[json_test_trace::test]
fn pass_one() {
    json_test_trace::tracing::info!("passing test one");
    assert_eq!(1 + 1, 2);
}

#[json_test_trace::test]
fn pass_two() {
    json_test_trace::tracing::info!("passing test two");
    assert_eq!(2 + 2, 4);
}

#[json_test_trace::test]
fn pass_three() {
    json_test_trace::tracing::info!("passing test three");
    assert_eq!(3 * 3, 9);
}

#[json_test_trace::test]
fn fail_one() {
    json_test_trace::tracing::info!("failing test");
    assert_eq!(1 + 1, 3);
}

#[json_test_trace::test]
fn panic_one() {
    json_test_trace::tracing::info!("about to panic");
    panic!("boom");
}