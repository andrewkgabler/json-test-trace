// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Parser edge case e2e tests — RFC 3339 timestamps, ignored tests, panic detection, is_passed with ignored.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

fn fixture_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("fixtures");
    path.push(name);
    path
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
fn query_rfc3339_timestamps_parsed() {
    let (stdout, _stderr) = run_query(
        fixture_path("rfc3339_timestamps.jsonl").to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have 1 test");
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("should be valid JSON");
    assert_eq!(json["name"], "test_rfc3339");
    assert_eq!(json["status"], "PASS");
}

#[test]
fn query_rfc3339_timestamps_in_summary() {
    let output = Command::new(binary())
        .args(["summary", fixture_path("rfc3339_timestamps.jsonl").to_string_lossy().as_ref()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("summary should be valid JSON");
    assert_eq!(json["tests"]["total"], 1);
    assert_eq!(json["tests"]["passed"], 1);
}

#[test]
fn query_ignored_test_has_ignored_field() {
    let (stdout, _stderr) = run_query(
        fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    let skipped: &str = lines.iter()
        .find(|line| {
            let json: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            json["name"] == "test_skipped"
        })
        .expect("should find test_skipped");
    
    let json: serde_json::Value = serde_json::from_str(skipped).expect("should be valid JSON");
    assert_eq!(json["ignored"], true, "ignored test should have ignored=true");
}

#[test]
fn query_panic_test_has_panicked_field() {
    let (stdout, _stderr) = run_query(
        fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    let panic_test: &str = lines.iter()
        .find(|line| {
            let json: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            json["name"] == "test_panic"
        })
        .expect("should find test_panic");
    
    let json: serde_json::Value = serde_json::from_str(panic_test).expect("should be valid JSON");
    assert_eq!(json["panicked"], true, "panicked test should have panicked=true");
}

#[test]
fn query_panic_test_status_is_fail() {
    let (stdout, _stderr) = run_query(
        fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    let panic_test: &str = lines.iter()
        .find(|line| {
            let json: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            json["name"] == "test_panic"
        })
        .expect("should find test_panic");
    
    let json: serde_json::Value = serde_json::from_str(panic_test).expect("should be valid JSON");
    assert!(json["status"].as_str().unwrap_or("").contains("FAIL") || json["status"].as_str().unwrap_or("") == "FAIL (panic)",
        "panicked test should have FAIL status, got: {}", json["status"]);
}

#[test]
fn query_is_passed_excludes_ignored() {
    let (stdout, _stderr) = run_query(
        fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref(),
        &["--status", "pass", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    let names: Vec<String> = lines.iter()
        .map(|line| {
            let json: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            json["name"].as_str().expect("name should be string").to_string()
        })
        .collect();
    assert!(!names.contains(&"test_skipped".to_string()), "ignored test should not be considered passed");
    assert!(names.contains(&"test_normal".to_string()), "normal test should be considered passed");
}

#[test]
fn query_is_passed_excludes_panicked() {
    let (stdout, _stderr) = run_query(
        fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref(),
        &["--status", "pass", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    let names: Vec<String> = lines.iter()
        .map(|line| {
            let json: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            json["name"].as_str().expect("name should be string").to_string()
        })
        .collect();
    assert!(!names.contains(&"test_panic".to_string()), "panicked test should not be considered passed");
}

#[test]
fn query_error_filter_matches_error_type() {
    let (stdout, _stderr) = run_query(
        fixture_path("various_error_formats.jsonl").to_string_lossy().as_ref(),
        &["--error", "IoError", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 test with IoError");
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("should be valid JSON");
    assert_eq!(json["name"], "test_io_error");
}

#[test]
fn query_error_filter_case_sensitive() {
    let (stdout, _stderr) = run_query(
        fixture_path("various_error_formats.jsonl").to_string_lossy().as_ref(),
        &["--error", "ioerror", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 0, "error filter should be case-sensitive");
}