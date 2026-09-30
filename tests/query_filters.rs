// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Filter tests — verify path, error, duration, and combined filter logic.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

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