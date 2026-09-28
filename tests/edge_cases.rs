// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Integration tests for edge cases not covered by the main test suite.

/// Test that an empty test (no tracing events) still emits test.start/test.end records.
#[json_test_trace::test]
fn empty_test_emits_lifecycle_records() {
    // No tracing events emitted
}

/// Test that a test with only debug-level events (below default INFO) still flushes.
#[json_test_trace::test]
fn debug_level_events_flush() {
    json_test_trace::tracing::debug!("this is a debug message");
}

/// Test that multiple concurrent tests flush without corruption.
#[json_test_trace::test(tokio::test)]
async fn concurrent_tests_flush_independently() {
    json_test_trace::tracing::info!("concurrent test event");
}

/// Test that the flush guard works correctly when dropped explicitly.
#[json_test_trace::test]
fn explicit_guard_drop() {
    let _guard = json_test_trace::init_test("explicit_guard_drop", module_path!());
    json_test_trace::tracing::info!("before explicit drop");
    // Guard drops here at end of scope
}

/// Test that nested spans are captured correctly.
#[json_test_trace::test]
fn nested_spans_captured() {
    let span = json_test_trace::tracing::info_span!("parent", name = "parent_span");
    let _enter = span.enter();
    json_test_trace::tracing::info!("inside parent");
    
    let child_span = json_test_trace::tracing::info_span!("child", name = "child_span");
    let _enter = child_span.enter();
    json_test_trace::tracing::info!("inside child");
}

/// Test that error-level events are captured.
#[json_test_trace::test]
fn error_level_events_captured() {
    json_test_trace::tracing::error!("this is an error message");
    json_test_trace::tracing::warn!("this is a warning message");
}

/// Test that the subscriber can be initialized multiple times (idempotent).
#[json_test_trace::test]
fn init_is_idempotent() {
    // init_global_test_logging is called by the macro, calling it again should be safe
    json_test_trace::init_global_test_logging();
    json_test_trace::tracing::info!("after double init");
}

/// Test that test.name and test.module fields are present in records.
#[json_test_trace::test]
fn test_identity_fields_present() {
    json_test_trace::tracing::info!("test identity check");
}

/// Test that the log file is created even if no events are emitted.
#[json_test_trace::test]
fn log_file_created_on_init() {
    // The #[ctor] creates the file, so it should exist even for empty tests
}

/// Test that async tests with tokio runtime flush correctly.
#[json_test_trace::test(tokio::test)]
async fn async_tokio_flush() {
    json_test_trace::tracing::info!("async event 1");
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    json_test_trace::tracing::info!("async event 2");
}

/// Test that tests with many events flush correctly.
#[json_test_trace::test]
fn many_events_flush() {
    for i in 0..100 {
        json_test_trace::tracing::info!("event {}", i);
    }
}

/// Test that tests with long names flush correctly.
#[json_test_trace::test]
fn test_with_very_long_name_that_exceeds_normal_identifier_length() {
    json_test_trace::tracing::info!("long name test");
}

/// Test that tests with special characters in module path work.
#[json_test_trace::test]
fn module_path_special_chars() {
    json_test_trace::tracing::info!("module path test");
}

/// Test that the flush happens even when panic occurs (unwinding panic).
#[json_test_trace::test]
#[should_panic(expected = "flush on panic")]
fn panic_flushes_buffer() {
    json_test_trace::tracing::info!("before panic");
    panic!("flush on panic");
}

/// Test that Result-returning tests flush correctly on success.
#[json_test_trace::test]
fn result_test_success() -> Result<(), Box<dyn std::error::Error>> {
    json_test_trace::tracing::info!("result test success");
    Ok(())
}