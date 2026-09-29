//! Tests that validate the parser against fixture files.
//! These fixtures represent the exact format produced by json-test-trace.

use json_test_trace::{TestParser, build_summary, TestResult};
use std::path::PathBuf;
use std::fs;

/// Get the path to the fixtures directory.
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// Load a fixture file and return its contents.
fn load_fixture(name: &str) -> String {
    let path = fixtures_dir().join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read fixture {}: {}", name, e))
}

/// Parse a fixture and return the test results.
fn parse_fixture(name: &str) -> Vec<TestResult> {
    let content = load_fixture(name);
    let mut parser = TestParser::new();
    
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        parser.parse_line(line).unwrap_or_else(|e| panic!("failed to parse line in {}: {}", name, e));
    }
    
    parser.finalize()
}

/// Test that a passing test fixture parses correctly.
#[test]
fn fixture_passing_test_parses() {
    let results = parse_fixture("passing_test.jsonl");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "test_first");
    assert!(results[0].is_passed());
    assert_eq!(results[0].duration_ms, Some(10.0));
}

/// Test that a failing test fixture parses correctly.
#[test]
fn fixture_failing_test_parses() {
    let results = parse_fixture("failing_test.jsonl");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "test_parse_invalid_sdl");
    assert!(!results[0].is_passed());
    assert!(results[0].has_errors);
}

/// Test that a mixed results fixture parses correctly.
#[test]
fn fixture_mixed_results_parses() {
    let results = parse_fixture("mixed_results.jsonl");
    assert_eq!(results.len(), 3);
    
    let first = results.iter().find(|t| t.name == "test_first").unwrap();
    assert!(first.is_passed());
    
    let second = results.iter().find(|t| t.name == "test_second").unwrap();
    assert!(second.is_passed());
    
    let third = results.iter().find(|t| t.name == "test_third").unwrap();
    assert!(!third.is_passed());
    assert!(third.has_errors);
}

/// Test that a concurrent tests fixture parses correctly.
#[test]
fn fixture_concurrent_tests_parses() {
    let results = parse_fixture("concurrent_tests.jsonl");
    assert_eq!(results.len(), 3);
    
    let alpha = results.iter().find(|t| t.name == "test_alpha").unwrap();
    assert!(alpha.is_passed());
    
    let beta = results.iter().find(|t| t.name == "test_beta").unwrap();
    assert!(!beta.is_passed());
    assert!(beta.has_errors);
    
    let gamma = results.iter().find(|t| t.name == "test_gamma").unwrap();
    assert!(gamma.is_passed());
}

/// Test that a panic test fixture parses correctly.
#[test]
fn fixture_panic_test_parses() {
    let results = parse_fixture("panic_test.jsonl");
    assert_eq!(results.len(), 1);
    
    let test = &results[0];
    assert_eq!(test.name, "test_will_panic");
    assert!(test.panicked);
    assert!(test.panic_message.is_some());
    assert_eq!(test.panic_message.as_deref(), Some("assertion failed: result.is_ok()"));
}

/// Test that an ignored test fixture parses correctly.
#[test]
fn fixture_ignored_test_parses() {
    let results = parse_fixture("ignored_test.jsonl");
    assert_eq!(results.len(), 2);
    
    let normal = results.iter().find(|t| t.name == "test_normal").unwrap();
    assert!(normal.is_passed());
    assert!(!normal.ignored);
    
    let skipped = results.iter().find(|t| t.name == "test_skipped").unwrap();
    assert!(skipped.ignored);
}

/// Test that the summary builder works with fixture data.
#[test]
fn fixture_summary_mixed_results() {
    let results = parse_fixture("mixed_results.jsonl");
    let summary = build_summary(&results);
    
    assert_eq!(summary.tests.total, 3);
    assert_eq!(summary.tests.passed, 2);
    assert_eq!(summary.tests.failed, 1);
}

/// Test that the summary builder works with panic data.
#[test]
fn fixture_summary_panic() {
    let results = parse_fixture("panic_test.jsonl");
    let summary = build_summary(&results);
    
    assert_eq!(summary.tests.total, 1);
    assert_eq!(summary.tests.failed, 1);
    assert_eq!(summary.errors.len(), 1);
    assert_eq!(summary.errors[0].category, "Panic");
}

/// Test that the summary builder works with ignored tests.
#[test]
fn fixture_summary_ignored() {
    let results = parse_fixture("ignored_test.jsonl");
    let summary = build_summary(&results);
    
    assert_eq!(summary.tests.total, 2);
    assert_eq!(summary.tests.passed, 1);
    assert_eq!(summary.tests.ignored, 1);
}

/// Test that error extraction works with fixture data.
#[test]
fn fixture_error_extraction() {
    let results = parse_fixture("failing_test.jsonl");
    let summary = build_summary(&results);
    
    assert_eq!(summary.errors.len(), 1);
    assert_eq!(summary.errors[0].category, "AdminSdl");
}

/// Test that nested spans fixture parses correctly.
#[test]
fn fixture_nested_spans_parses() {
    let results = parse_fixture("nested_spans.jsonl");
    assert_eq!(results.len(), 1);
    
    let test = &results[0];
    assert_eq!(test.name, "test_nested");
    assert!(test.is_passed());
    assert_eq!(test.log_events.len(), 2);
}

/// Test that slow test fixture parses correctly.
#[test]
fn fixture_slow_test_parses() {
    let results = parse_fixture("slow_test.jsonl");
    assert_eq!(results.len(), 1);
    
    let test = &results[0];
    assert_eq!(test.name, "test_slow_query");
    assert!(test.is_passed());
    assert!(test.duration_ms.unwrap() > 2000.0);
}

/// Test that test with warnings fixture parses correctly.
#[test]
fn fixture_test_with_warnings_parses() {
    let results = parse_fixture("test_with_warnings.jsonl");
    assert_eq!(results.len(), 1);
    
    let test = &results[0];
    assert_eq!(test.name, "test_with_warnings");
    assert!(test.is_passed());
    assert_eq!(test.log_events.len(), 2);
}
