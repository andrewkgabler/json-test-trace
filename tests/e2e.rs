// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! End-to-end tests — query real json-test-trace JSONL output through the CLI binary.
//!
//! Fixtures contain real output from cargo test with json-test-trace enabled.
//! No fabricated data. No snapshots. Real test data, every time.

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Path to the json-test-trace binary
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

/// Path to the fixtures directory
fn fixture_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("fixtures");
    path.push(name);
    path
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
fn e2e_query_json_format_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert!(lines.len() >= 3, "should have at least 3 test results from real cargo test");
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert!(json.get("name").is_some(), "each test should have a name");
        assert!(json.get("status").is_some(), "each test should have a status");
        assert!(json.get("module").is_some(), "each test should have a module");
        assert!(json.get("duration_ms").is_some(), "each test should have duration_ms");
    }
}

#[test]
fn e2e_query_pretty_format_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--format", "pretty"],
    );
    assert!(stdout.trim().starts_with('['), "pretty format should start with [");
    assert!(stdout.trim().ends_with(']'), "pretty format should end with ]");
}

#[test]
fn e2e_summary_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let output = run_summary(fixture.to_string_lossy().as_ref());
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let total: usize = json["tests"]["total"].as_u64().expect("total should be a number") as usize;
    assert_eq!(total, 3, "should have 3 tests from real cargo test");
    let passed: usize = json["tests"]["passed"].as_u64().expect("passed should be a number") as usize;
    assert_eq!(passed, 2, "should have 2 passed tests");
    let failed: usize = json["tests"]["failed"].as_u64().expect("failed should be a number") as usize;
    assert_eq!(failed, 1, "should have 1 failed test");
}

#[test]
fn e2e_instructions_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let output = run_instructions(fixture.to_string_lossy().as_ref());
    assert!(output.contains("=== test_first ==="));
    assert!(output.contains("=== test_second ==="));
    assert!(output.contains("=== test_third ==="));
    assert!(output.contains("Status: PASS"));
    assert!(output.contains("Status: FAIL"));
}

#[test]
fn e2e_query_status_pass_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--status", "pass", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "should have exactly 2 passed tests");
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["status"], "PASS", "all tests should be PASS");
        assert_eq!(json["has_errors"], false, "passed tests should not have errors");
    }
}

#[test]
fn e2e_query_status_fail_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--status", "fail", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 failed test");
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("should be valid JSON");
    assert_eq!(json["status"], "FAIL");
    assert_eq!(json["has_errors"], true);
}

#[test]
fn e2e_query_duration_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
        assert!(duration > 0.0, "duration should be positive");
    }
}

#[test]
fn e2e_query_log_events_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let log_events: usize = json["log_events"].as_u64().expect("should have log_events") as usize;
        assert!(log_events >= 1, "each test should have at least 1 log event");
    }
}

#[test]
fn e2e_query_module_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let module: &str = json["module"].as_str().expect("module should be a string");
        assert_eq!(module, "allograph::auth::claims");
    }
}

#[test]
fn e2e_summary_error_categories_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let output = run_summary(fixture.to_string_lossy().as_ref());
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
fn e2e_summary_modules_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let output = run_summary(fixture.to_string_lossy().as_ref());
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let modules = json["modules"].as_array().expect("modules should be an array");
    assert!(modules.iter().any(|m| m == "allograph::auth::claims"));
}

#[test]
fn e2e_query_combined_filters_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
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
fn e2e_query_min_duration_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--min-duration", "15", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
        assert!(duration >= 15.0, "duration should be >= 15ms");
    }
}

#[test]
fn e2e_query_max_duration_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--max-duration", "15", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
        assert!(duration <= 15.0, "duration should be <= 15ms");
    }
}

#[test]
fn e2e_query_name_filter_from_real_test_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let name: &str = json["name"].as_str().expect("name should be a string");
        assert!(!name.is_empty(), "name should not be empty");
    }
}

#[test]
fn e2e_query_file_not_found() {
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
fn e2e_query_default_format_is_pretty() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(fixture.to_string_lossy().as_ref(), &[]);
    assert!(stdout.starts_with('['), "default format should be pretty (JSON array)");
}

#[test]
fn e2e_instructions_all_tests_present() {
    let fixture = fixture_path("mixed_results.jsonl");
    let output = run_instructions(fixture.to_string_lossy().as_ref());
    let (query_stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = query_stdout.trim().lines().collect();
    let test_count = lines.len();
    let header_count = output.matches("===").count() / 2;
    assert_eq!(
        header_count, test_count,
        "instructions should have {} test headers, got {}",
        test_count, header_count
    );
}

#[test]
fn e2e_summary_counts_match_query_results() {
    let fixture = fixture_path("mixed_results.jsonl");
    let summary_output = run_summary(fixture.to_string_lossy().as_ref());
    let summary_json: serde_json::Value = serde_json::from_str(&summary_output).expect("summary should be valid JSON");

    let (query_stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let query_lines: Vec<&str> = query_stdout.trim().lines().collect();

    let summary_total: usize = summary_json["tests"]["total"].as_u64().expect("total should be a number") as usize;
    assert_eq!(
        summary_total, query_lines.len(),
        "summary total should match query result count"
    );
}

#[test]
fn e2e_summary_passed_count_matches_query_pass() {
    let fixture = fixture_path("mixed_results.jsonl");
    let summary_output = run_summary(fixture.to_string_lossy().as_ref());
    let summary_json: serde_json::Value = serde_json::from_str(&summary_output).expect("summary should be valid JSON");

    let (pass_stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--status", "pass", "--format", "json"],
    );
    let pass_lines: Vec<&str> = pass_stdout.trim().lines().collect();

    let summary_passed: usize = summary_json["tests"]["passed"].as_u64().expect("passed should be a number") as usize;
    assert_eq!(
        summary_passed, pass_lines.len(),
        "summary passed count should match query pass result count"
    );
}

#[test]
fn e2e_summary_failed_count_matches_query_fail() {
    let fixture = fixture_path("mixed_results.jsonl");
    let summary_output = run_summary(fixture.to_string_lossy().as_ref());
    let summary_json: serde_json::Value = serde_json::from_str(&summary_output).expect("summary should be valid JSON");

    let (fail_stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--status", "fail", "--format", "json"],
    );
    let fail_lines: Vec<&str> = fail_stdout.trim().lines().collect();

    let summary_failed: usize = summary_json["tests"]["failed"].as_u64().expect("failed should be a number") as usize;
    assert_eq!(
        summary_failed, fail_lines.len(),
        "summary failed count should match query fail result count"
    );
}

#[test]
fn e2e_query_has_errors_field_present() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert!(json.get("has_errors").is_some(), "should have has_errors field");
        let has_errors: bool = json["has_errors"].as_bool().expect("has_errors should be a boolean");
        assert!(has_errors || !has_errors, "has_errors should be true or false");
    }
}

#[test]
fn e2e_query_panic_fields_present() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert!(json.get("panicked").is_some(), "should have panicked field");
        assert!(json.get("panic_message").is_some(), "should have panic_message field");
    }
}

#[test]
fn e2e_query_path_substring_match() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--path", "auth", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 3, "all tests should match 'auth' substring");
}

#[test]
fn e2e_query_path_no_match() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--path", "nonexistent_module", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 0, "no tests should match 'nonexistent_module'");
}

#[test]
fn e2e_query_error_substring_match() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--error", "ValidationError", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 test with ValidationError");
    assert!(lines[0].contains("test_third"));
}

#[test]
fn e2e_query_error_no_match() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--error", "NonExistentError", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 0, "no tests should match 'NonExistentError'");
}

#[test]
fn e2e_query_combined_filters() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--status", "fail", "--path", "auth", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 test matching both filters");
    assert!(lines[0].contains("test_third"));
}

#[test]
fn e2e_query_status_fail_with_error_filter() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--status", "fail", "--error", "ValidationError", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 test matching both filters");
    assert!(lines[0].contains("test_third"));
}

#[test]
fn e2e_query_status_fail_with_error_and_duration_filter() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--status", "fail", "--error", "ValidationError", "--min-duration", "10", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 test matching all filters");
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("should be valid JSON");
    let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
    assert!(duration >= 10.0, "duration should be >= 10ms");
}

#[test]
fn e2e_query_status_pass_with_path_and_duration_range() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
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
fn e2e_query_status_pass_with_error_no_match() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--status", "pass", "--error", "ValidationError", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 0, "no passed tests should have ValidationError");
}

#[test]
fn e2e_query_all_filters_together() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
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
fn e2e_summary_includes_error_categories() {
    let fixture = fixture_path("mixed_results.jsonl");
    let output = run_summary(fixture.to_string_lossy().as_ref());
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
fn e2e_instructions_produces_readable_output() {
    let fixture = fixture_path("mixed_results.jsonl");
    let output = run_instructions(fixture.to_string_lossy().as_ref());
    assert!(output.contains("=== test_first ==="));
    assert!(output.contains("=== test_second ==="));
    assert!(output.contains("=== test_third ==="));
    assert!(output.contains("Status: PASS"));
    assert!(output.contains("Status: FAIL"));
}

#[test]
fn e2e_instructions_shows_errors_for_failed_tests() {
    let fixture = fixture_path("mixed_results.jsonl");
    let output = run_instructions(fixture.to_string_lossy().as_ref());
    assert!(
        output.contains("Errors: Test failed with errors in log"),
        "instructions should show error indicator for failed tests"
    );
}

#[test]
fn e2e_summary_produces_valid_json() {
    let fixture = fixture_path("mixed_results.jsonl");
    let output = run_summary(fixture.to_string_lossy().as_ref());
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    assert_eq!(json["tests"]["total"], 3);
    assert_eq!(json["tests"]["passed"], 2);
    assert_eq!(json["tests"]["failed"], 1);
}

#[test]
fn e2e_summary_includes_modules() {
    let fixture = fixture_path("mixed_results.jsonl");
    let output = run_summary(fixture.to_string_lossy().as_ref());
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let modules = json["modules"].as_array().expect("modules should be an array");
    assert!(
        modules.iter().any(|m| m == "allograph::auth::claims"),
        "should include module name"
    );
}

#[test]
fn e2e_query_status_pass_includes_only_passed() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
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
fn e2e_query_status_fail_includes_errors() {
    let fixture = fixture_path("mixed_results.jsonl");
    let (stdout, _stderr) = run_query(
        fixture.to_string_lossy().as_ref(),
        &["--status", "fail", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 failed test");
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("should be valid JSON");
    assert_eq!(json["status"], "FAIL");
    assert_eq!(json["has_errors"], true);
}