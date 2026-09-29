// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! CLI integration tests — end-to-end tests that run cargo test with json-test-trace,
//! read the produced JSONL, and query it through the CLI binary.
//!
//! No snapshots. No fabricated fixtures. Real tests, real data, every time.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Path to the json-test-trace binary
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

/// Run the CLI query subcommand and return stdout
fn run_query(input: &str, args: &[&str]) -> (String, String) {
    let mut cmd = Command::new(binary());
    cmd.args(["query", input]);
    cmd.args(args);
    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (stdout, stderr)
}

/// Run the CLI summary subcommand and return stdout
fn run_summary(input: &str) -> String {
    let output = Command::new(binary())
        .args(["summary", input])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli");

    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Run the CLI instructions subcommand and return stdout
fn run_instructions(input: &str) -> String {
    let output = Command::new(binary())
        .args(["instructions", input])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli");

    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn query_pretty_format_outputs_json_array() {
    let (stdout, _stderr) = run_query("fixtures/mixed_results.jsonl", &["--format", "pretty"]);
    assert!(stdout.trim().starts_with('['), "pretty format should start with [");
    assert!(stdout.trim().ends_with(']'), "pretty format should end with ]");
}

#[test]
fn query_json_format_outputs_jsonl() {
    let (stdout, _stderr) = run_query("fixtures/mixed_results.jsonl", &["--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert!(lines.len() >= 3, "should have at least 3 JSONL lines");
    for line in lines {
        assert!(
            line.starts_with('{'),
            "JSONL line should start with {{: {}",
            line
        );
    }
}

#[test]
fn query_status_pass_filters_correctly() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "pass", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        assert!(
            line.contains(r#""status":"PASS""#),
            "pass filter should only include PASS tests: {}",
            line
        );
    }
}

#[test]
fn query_status_fail_filters_correctly() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "fail", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 failed test");
    assert!(lines[0].contains(r#""status":"FAIL""#));
    assert!(lines[0].contains("test_third"));
}

#[test]
fn query_path_substring_match() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--path", "auth", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 3, "all tests should match 'auth' substring");
}

#[test]
fn query_path_no_match() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--path", "nonexistent_module", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 0, "no tests should match 'nonexistent_module'");
}

#[test]
fn query_error_substring_match() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--error", "ValidationError", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 test with ValidationError");
    assert!(lines[0].contains("test_third"));
}

#[test]
fn query_error_no_match() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--error", "NonExistentError", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 0, "no tests should match 'NonExistentError'");
}

#[test]
fn query_combined_filters() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "fail", "--path", "auth", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 test matching both filters");
    assert!(lines[0].contains("test_third"));
}

#[test]
fn query_status_fail_with_error_filter() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "fail", "--error", "ValidationError", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 test matching both filters");
    assert!(lines[0].contains("test_third"));
}

#[test]
fn query_status_fail_with_error_and_duration_filter() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "fail", "--error", "ValidationError", "--min-duration", "10", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 test matching all filters");
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("should be valid JSON");
    let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
    assert!(duration >= 10.0, "duration should be >= 10ms");
}

#[test]
fn query_status_pass_with_path_and_duration_range() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "pass", "--path", "auth", "--min-duration", "10", "--max-duration", "20", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "should have exactly 2 passed tests in range");
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["status"], "PASS");
        let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
        assert!(duration >= 10.0 && duration <= 20.0, "duration should be between 10 and 20ms");
    }
}

#[test]
fn query_status_pass_with_error_no_match() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "pass", "--error", "ValidationError", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 0, "no passed tests should have ValidationError");
}

#[test]
fn query_all_filters_together() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "fail", "--path", "auth", "--error", "ValidationError", "--min-duration", "10", "--max-duration", "20", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 test matching all filters");
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("should be valid JSON");
    assert_eq!(json["status"], "FAIL");
    assert_eq!(json["name"], "test_third");
    let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
    assert!(duration >= 10.0 && duration <= 20.0, "duration should be between 10 and 20ms");
}

#[test]
fn summary_produces_valid_json() {
    let output = run_summary("fixtures/mixed_results.jsonl");
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    assert_eq!(json["tests"]["total"], 3);
    assert_eq!(json["tests"]["passed"], 2);
    assert_eq!(json["tests"]["failed"], 1);
}

#[test]
fn summary_includes_error_categories() {
    let output = run_summary("fixtures/mixed_results.jsonl");
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let errors = json["errors"].as_array().expect("errors should be an array");
    assert!(errors.len() >= 1, "should have at least 1 error category");
    let validation_error = errors
        .iter()
        .find(|e| e["category"] == "ValidationError")
        .expect("should have ValidationError category");
    assert_eq!(validation_error["count"], 1);
}

#[test]
fn instructions_produces_readable_output() {
    let output = run_instructions("fixtures/mixed_results.jsonl");
    assert!(output.contains("=== test_first ==="));
    assert!(output.contains("=== test_second ==="));
    assert!(output.contains("=== test_third ==="));
    assert!(output.contains("Status: PASS"));
    assert!(output.contains("Status: FAIL"));
}

#[test]
fn instructions_shows_errors_for_failed_tests() {
    let output = run_instructions("fixtures/mixed_results.jsonl");
    assert!(
        output.contains("Errors: Test failed with errors in log"),
        "instructions should show error indicator for failed tests"
    );
}

#[test]
fn query_file_not_found() {
    let output = Command::new(binary())
        .args(["query", "/tmp/nonexistent_file_12345.jsonl"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli");

    assert!(!output.status.success(), "should fail for nonexistent file");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error reading file"), "should show error message");
}

#[test]
fn query_default_format_is_pretty() {
    let (stdout, _stderr) = run_query("fixtures/mixed_results.jsonl", &[]);
    assert!(stdout.starts_with('['), "default format should be pretty (JSON array)");
}

#[test]
fn query_min_duration_filter() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--min-duration", "15", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let duration: f64 = serde_json::from_str::<serde_json::Value>(line)
            .expect("should be valid JSON")
            .get("duration_ms")
            .and_then(|v| v.as_f64())
            .expect("should have duration_ms");
        assert!(duration >= 15.0, "all tests should have duration >= 15ms");
    }
}

#[test]
fn query_max_duration_filter() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--max-duration", "15", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let duration: f64 = serde_json::from_str::<serde_json::Value>(line)
            .expect("should be valid JSON")
            .get("duration_ms")
            .and_then(|v| v.as_f64())
            .expect("should have duration_ms");
        assert!(duration <= 15.0, "all tests should have duration <= 15ms");
    }
}

#[test]
fn summary_includes_modules() {
    let output = run_summary("fixtures/mixed_results.jsonl");
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let modules = json["modules"].as_array().expect("modules should be an array");
    assert!(
        modules.iter().any(|m| m == "allograph::auth::claims"),
        "should include module name"
    );
}

#[test]
fn query_status_pass_includes_only_passed() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "pass", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "should have exactly 2 passed tests");
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["status"], "PASS");
        assert_eq!(json["has_errors"], false);
    }
}

#[test]
fn query_status_fail_includes_errors() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "fail", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 failed test");
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("should be valid JSON");
    assert_eq!(json["status"], "FAIL");
    assert_eq!(json["has_errors"], true);
}